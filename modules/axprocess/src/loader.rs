use crate::{fd::FdList, stack::TaskStack, task::TASK_ID_ALLOCATOR, Task, TaskState};
use alloc::{sync::Arc, vec::Vec};
use axconfig::TASK_STACK_SIZE;
use axhal::{
    arch::{write_page_table_root, TaskContext, TrapFrame},
    mem::{VirtAddr, PAGE_SIZE_4K},
    paging::MappingFlags,
};
use axmem::MemorySet;
use core::{
    alloc::Layout,
    cell::UnsafeCell,
    ptr::{null, NonNull},
    str::from_utf8,
    sync::atomic::{AtomicU8, Ordering},
};
use riscv::register::sstatus::Sstatus;
use spinlock::SpinNoIrq;
use xmas_elf::{program::SegmentData, ElfFile};

/// A elf file wrapper.
pub struct Loader<'a> {
    elf: ElfFile<'a>,
}

impl<'a> Loader<'a> {
    /// Create a new Loader from data: &[u8].
    ///
    /// # Panics
    ///
    /// Panics if data is not valid elf.
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            elf: ElfFile::new(data).expect("Error parsing app ELF file."),
        }
    }

    /// Create a Task from Loader, only used for init process. Other processes are spawned by
    /// clone (fork) + execve.
    /// This function will allocate kernel stack and put `TrapFrame` (including `argv`) into place.
    pub fn load(self, args: &[&str]) -> Task {
        if let Some(interp) = self
            .elf
            .program_iter()
            .find(|ph| ph.get_type() == Ok(xmas_elf::program::Type::Interp))
        {
            let interp = match interp.get_data(&self.elf) {
                Ok(SegmentData::Undefined(data)) => data,
                _ => panic!("Invalid data in Interp Elf Program Header"),
            };

            let interp_path = from_utf8(interp).expect("Interpreter path isn't valid UTF-8");
            // remove trailing '\0'
            let interp_path = interp_path.trim_matches(char::from(0));
            info!("Interpreter path: {}", interp_path);

            let mut new_argv = vec![interp_path];
            new_argv.extend_from_slice(args);
            info!("Interpreter args: {:?}", new_argv);

            #[cfg(not(feature = "fs"))]
            {
                panic!("ELF Interpreter is not supported without fs feature");
            }

            let interp = axfs::api::read(interp_path).expect("Error reading Interpreter from fs");
            let loader = Loader::new(&interp);
            return loader.load(&new_argv);
        }

        let mut memory_set = MemorySet::new_with_kernel_mapped();
        unsafe {
            write_page_table_root(memory_set.page_table_root_ppn());
        }

        unsafe {
            riscv::register::sstatus::set_sum();
        }
        let auxv = memory_set.map_elf(&self.elf);

        let kstack = TaskStack::alloc(TASK_STACK_SIZE);

        // Allocate memory for user stack and hold it in memory_set
        let ustack_bottom = VirtAddr::from(0x3fe5_0000);
        memory_set.alloc_region(
            ustack_bottom,
            TASK_STACK_SIZE,
            MappingFlags::USER | MappingFlags::READ | MappingFlags::WRITE,
            None,
        );
        let ustack = TaskStack::new(
            NonNull::new(usize::from(ustack_bottom) as *mut u8)
                .expect("Error creating NonNull<u8> for ustack bottom"),
            Layout::from_size_align(TASK_STACK_SIZE, 16).expect("Error creating layout for ustack"),
        );

        let pid = TASK_ID_ALLOCATOR.fetch_add(1, Ordering::Relaxed);

        // handle trap frame
        let trap_frame = gen_trap_frame(ustack.top(), memory_set.entry);

        unsafe {
            core::ptr::write(
                (usize::from(kstack.top()) - core::mem::size_of::<TrapFrame>()) as *mut TrapFrame,
                trap_frame.clone(),
            );
        }

        let mut task = Task {
            pid,
            tid: pid,
            state: AtomicU8::new(TaskState::Ready as u8),
            ctx: UnsafeCell::new(TaskContext::new()),

            memory_set: Arc::new(memory_set),
            kstack,
            ustack,

            #[cfg(feature = "fs")]
            fd_table: SpinNoIrq::new(FdList::default()),
        };

        unsafe {
            // args[0]
            task.push_str(args[0]);

            // random
            let random_str = &[3703830112808742751usize, 7081108068768079778usize];
            let random_pos = task.push_slice(random_str.as_slice());

            // env
            let env = vec![
                    "SHLVL=1",
                    "PATH=/usr/sbin:/usr/bin:/sbin:/bin",
                    "PWD=/",
                    "GCC_EXEC_PREFIX=/riscv64-linux-musl-native/bin/../lib/gcc/",
                    "COLLECT_GCC=./riscv64-linux-musl-native/bin/riscv64-linux-musl-gcc",
                    "COLLECT_LTO_WRAPPER=/riscv64-linux-musl-native/bin/../libexec/gcc/riscv64-linux-musl/11.2.1/lto-wrapper",
                    "COLLECT_GCC_OPTIONS='-march=rv64gc' '-mabi=lp64d' '-march=rv64imafdc' '-dumpdir' 'a.'",
                    "COMPILER_PATH=/riscv64-linux-musl-native/bin/../libexec/gcc/riscv64-linux-musl/11.2.1/:/riscv64-linux-musl-native/bin/../libexec/gcc/:/riscv64-linux-musl-native/bin/../lib/gcc/riscv64-linux-musl/11.2.1/../../../../riscv64-linux-musl/bin/",
                    "LIBRARY_PATH=/riscv64-linux-musl-native/bin/../lib/gcc/riscv64-linux-musl/11.2.1/:/riscv64-linux-musl-native/bin/../lib/gcc/:/riscv64-linux-musl-native/bin/../lib/gcc/riscv64-linux-musl/11.2.1/../../../../riscv64-linux-musl/lib/:/riscv64-linux-musl-native/bin/../lib/:/riscv64-linux-musl-native/bin/../usr/lib/",
                ];
            let envs: Vec<_> = env.iter().map(|item| task.push_str(item)).collect();

            // args
            let argv: Vec<_> = args.iter().map(|item| task.push_str(item)).collect();

            // auxv
            task.push_slice(&[null::<u8>(), null::<u8>()]);
            for (&type_, &value) in auxv.iter() {
                //info!("auxv {} {:x}", type_ ,value);
                match type_ {
                    25u8 => task.push_slice(&[type_ as usize, random_pos]),
                    _ => task.push_slice(&[type_ as usize, value]),
                };
            }

            task.push_slice(&[null::<u8>()]);
            task.push_slice(envs.as_slice());

            task.push_slice(&[null::<u8>()]);
            task.push_slice(argv.as_slice());

            task.push_slice(&[argv.len()]);
        };

        task
    }
}

/// Create a `TrapFrame` for a new process with ustack_top and entry_point set.
fn gen_trap_frame(ustack_top: VirtAddr, entry_point: usize) -> TrapFrame {
    let mut trap_frame = TrapFrame::default();

    trap_frame.regs.sp = ustack_top.into();
    trap_frame.sepc = entry_point;

    // restore kernel gp, tp
    let tp: usize;
    let gp: usize;
    unsafe {
        core::arch::asm!(
            "mv {}, tp",
            "mv {}, gp",
            out(reg) tp,
            out(reg) gp,
        );
    }
    trap_frame.regs.tp = tp;
    trap_frame.regs.gp = gp;

    // set SPP to User, SUM to 1
    let sstatus_reg = riscv::register::sstatus::read();
    unsafe {
        trap_frame.sstatus =
            *(&sstatus_reg as *const Sstatus as *const usize) & !(1 << 8) | (1 << 18);
    }

    trap_frame
}
