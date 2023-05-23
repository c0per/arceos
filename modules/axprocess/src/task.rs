use crate::{
    fd::FdList,
    scheduler::CurrentTask,
    stack::TaskStack,
    stdio::{Stderr, Stdin, Stdout},
};
use alloc::{string::String, sync::Arc, vec, vec::Vec};
use axconfig::TASK_STACK_SIZE;
use axhal::{
    arch::{enter_user, write_page_table_root, TaskContext, TrapFrame},
    mem::{PhysAddr, VirtAddr, PAGE_SIZE_4K},
    paging::MappingFlags,
};
use axmem::MemorySet;
use core::{
    alloc::Layout,
    arch::asm,
    cell::{RefCell, UnsafeCell},
    mem::{align_of, size_of},
    ptr::{copy_nonoverlapping, NonNull},
    sync::atomic::{AtomicU8, AtomicUsize, Ordering},
};
use riscv::register::sstatus::Sstatus;
use spinlock::SpinNoIrq;

#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TaskState {
    Running = 1,
    Ready = 2,
    Blocked = 3,
    Exited = 4,
}

pub struct Task {
    pub pid: usize,
    pub tid: usize,
    pub(crate) state: AtomicU8,
    pub(crate) ctx: UnsafeCell<TaskContext>,

    pub(crate) memory_set: Arc<MemorySet>,

    /// TaskStack is simply a pointer to memory in memory_set.
    /// Kernel stack is mapped in "free memory" region.
    pub(crate) kstack: TaskStack,
    /// User stack is mapped in user space (highest address)
    pub(crate) ustack: TaskStack,

    #[cfg(feature = "fs")]
    pub(crate) fd_table: SpinNoIrq<FdList>,
}

unsafe impl Send for Task {}
unsafe impl Sync for Task {}

impl Task {
    pub fn set_state(&self, state: TaskState) {
        self.state.store(state as u8, Ordering::Release);
    }

    pub fn is_running(&self) -> bool {
        self.state.load(Ordering::Acquire) == TaskState::Running as u8
    }

    pub fn is_ready(&self) -> bool {
        self.state.load(Ordering::Acquire) == TaskState::Ready as u8
    }

    pub fn is_blocked(&self) -> bool {
        self.state.load(Ordering::Acquire) == TaskState::Blocked as u8
    }

    pub fn is_init(&self) -> bool {
        self.pid == 1
    }

    pub fn page_table_ppn(&self) -> PhysAddr {
        self.memory_set.page_table_root_ppn()
    }

    pub fn trap_frame_ptr(&self) -> *const axhal::arch::TrapFrame {
        (usize::from(self.kstack.top()) - core::mem::size_of::<TrapFrame>()) as *const TrapFrame
    }

    pub fn trap_frame_ptr_mut(&self) -> *mut axhal::arch::TrapFrame {
        (usize::from(self.kstack.top()) - core::mem::size_of::<TrapFrame>()) as *mut TrapFrame
    }

    pub fn ctx_mut_ptr(&self) -> *mut TaskContext {
        self.ctx.get()
    }

    /* /// Push in argv to stack in `TrapFrame` in kernel stack and update sp, a0, a1 in `TrapFrame`.
    /// This function need to be when with SUM set.
    pub unsafe fn push_argv(&self, argv: &[&str]) {
        let trap_frame = &mut *self.trap_frame_ptr_mut();

        let mut sp = trap_frame.regs.sp;
        let argc = argv.len();
        let mut argv_addr = Vec::with_capacity(argc);

        argv.iter().for_each(|arg| {
            sp -= (arg.len() + 1) * size_of::<u8>();
            sp -= sp % align_of::<u8>(); // alignment

            copy_nonoverlapping(arg.as_ptr(), sp as *mut u8, arg.len());
            *((sp + arg.len()) as *mut u8) = b'\0';

            argv_addr.push(sp as *const u8);
        });

        sp -= (argc + 1) * size_of::<*const u8>();
        sp -= sp % align_of::<*const u8>(); // alignment
        copy_nonoverlapping(argv_addr.as_ptr(), sp as *mut *const u8, argc);

        sp -= size_of::<isize>();
        sp -= sp % align_of::<isize>();
        *(sp as *mut isize) = argc as isize;

        trap_frame.regs.sp = sp;
        trap_frame.regs.a0 = sp;
    } */

    pub unsafe fn push_slice<T: Copy>(&mut self, vs: &[T]) -> usize {
        let trap_frame = &mut *self.trap_frame_ptr_mut();
        let mut sp = trap_frame.regs.sp;

        sp -= vs.len() * size_of::<T>();
        sp -= sp % align_of::<T>();

        core::slice::from_raw_parts_mut(sp as *mut T, vs.len()).copy_from_slice(vs);

        trap_frame.regs.sp = sp;

        sp
    }

    pub unsafe fn push_str(&mut self, s: &str) -> usize {
        self.push_slice(&[b'\0']);
        self.push_slice(s.as_bytes())
    }

    /// For Init (pid = 1) task only. Must be run after switching to user page table.
    pub unsafe fn enter_as_init(&self) -> ! {
        if self.pid != 1 || self.tid != 1 {
            panic!("Calling enter_user() for task other than (1, 1)");
        }

        self.set_state(TaskState::Running);

        enter_user(self.kstack.top().into());
    }

    pub fn set_in_wait_queue(&self, _: bool) {}

    pub fn id_name(&self) -> String {
        format!("Task({}, {})", self.pid, self.tid)
    }

    // fork
    pub fn clone(&self) -> Self {
        extern "C" {
            fn task_entry();
        }

        let pid = TASK_ID_ALLOCATOR.fetch_add(1, Ordering::Release);

        let mut memory_set = self.memory_set.clone_mapped();

        // Create new U stack
        // let max_va = memory_set.max_va();
        // let ustack_bottom = max_va + PAGE_SIZE_4K;
        // memory_set.alloc_region(
        //     ustack_bottom,
        //     TASK_STACK_SIZE,
        //     MappingFlags::USER | MappingFlags::READ | MappingFlags::WRITE,
        //     None,
        // );
        // let ustack = TaskStack::new(
        //     NonNull::new(usize::from(ustack_bottom) as *mut u8)
        //         .expect("Error creating NonNull<u8> for ustack bottom"),
        //     Layout::from_size_align(axconfig::TASK_STACK_SIZE, 16)
        //         .expect("Error creating layout for ustack"),
        // );

        // Create new S stack
        let kstack = TaskStack::alloc(axconfig::TASK_STACK_SIZE);
        let trap_frame =
            (usize::from(kstack.top()) - core::mem::size_of::<TrapFrame>()) as *mut TrapFrame;
        unsafe {
            core::ptr::copy_nonoverlapping(self.trap_frame_ptr(), trap_frame, 1);
        }

        let trap_frame = unsafe { &mut *trap_frame };
        trap_frame.regs.a0 = 0;
        // trap_frame.regs.sp = ustack.top().into();

        let mut ctx = TaskContext::new();
        ctx.init(
            task_entry as usize,
            kstack.top() - core::mem::size_of::<TrapFrame>(),
        );

        Self {
            pid,
            tid: pid,
            state: AtomicU8::new(TaskState::Ready as u8),
            ctx: UnsafeCell::new(ctx),

            memory_set: Arc::new(memory_set),

            kstack,
            ustack: self.ustack.clone(),

            #[cfg(feature = "fs")]
            fd_table: SpinNoIrq::new(self.fd_table.lock().clone()),
        }
    }

    #[cfg(feature = "fs")]
    pub fn fd_table(&self) -> &SpinNoIrq<FdList> {
        &self.fd_table
    }
}

pub(crate) static TASK_ID_ALLOCATOR: AtomicUsize = AtomicUsize::new(1);
