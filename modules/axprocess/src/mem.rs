use alloc::boxed::Box;
use axhal::mem::VirtAddr;
use axhal::paging::MappingFlags;
use axmem::MemBackend;
use axsyscall::mem::SyscallMem;

use crate::scheduler::CurrentTask;

struct SyscallMemImpl;

bitflags::bitflags! {
    pub struct MMapProtFlag: u32 {
        const PROT_READ = 1 << 0;
        const PROT_WRITE = 1 << 1;
        const PROT_EXEC = 1 << 2;
    }
}

bitflags::bitflags! {
    pub struct MMapFlags: u32 {
        const MAP_FILE = 0; // ignored
        const MAP_SHARED = 1 << 0;
        const MAP_PRIVATE = 1 << 1;

        const MAP_FIXED = 1 << 4;
        const MAP_ANONYMOUS = 1 << 5;
        const MAP_NORESERVE = 1 << 14;
    }
}

impl From<MMapProtFlag> for MappingFlags {
    fn from(value: MMapProtFlag) -> Self {
        let mut flags = MappingFlags::USER;

        if value.contains(MMapProtFlag::PROT_READ) {
            flags |= MappingFlags::READ;
        }
        if value.contains(MMapProtFlag::PROT_WRITE) {
            flags |= MappingFlags::WRITE;
        }
        if value.contains(MMapProtFlag::PROT_EXEC) {
            flags |= MappingFlags::EXECUTE;
        }

        flags
    }
}

#[crate_interface::impl_interface]
impl SyscallMem for SyscallMemImpl {
    fn mmap(start: usize, len: usize, prot: u32, flags: u32, fd: usize, offset: usize) -> isize {
        let prot: MappingFlags = MMapProtFlag::from_bits(prot)
            .expect("MMap prot not supported.")
            .into();

        let flags = MMapFlags::from_bits(flags).expect("MMap prot not supported.");

        let fixed = flags.contains(MMapFlags::MAP_FIXED);
        // try to map to NULL
        if fixed && start == 0 {
            return -1;
        }

        let current = CurrentTask::try_get().expect("No current task");
        let addr = if flags.contains(MMapFlags::MAP_ANONYMOUS) {
            // no file
            current
                .memory_set
                .lock()
                .mmap(start.into(), len, prot, fixed, None)
        } else {
            // file backend
            info!("[mmap] fd: {}, offset: 0x{:x}", fd, offset);
            let file = match current.fd_table.lock().query_fd(fd) {
                Some(file) => Box::new(file.lock().clone_file()),
                // fd not found
                None => return -1,
            };

            let backend = MemBackend::new(file, offset as u64);

            current
                .memory_set
                .lock()
                .mmap(start.into(), len, prot, fixed, Some(backend))
        };

        unsafe { riscv::asm::sfence_vma_all() };

        addr
    }

    fn munmap(start: usize, len: usize) -> isize {
        let current = CurrentTask::try_get().expect("No current task");
        current.memory_set.lock().munmap(start.into(), len);

        unsafe { riscv::asm::sfence_vma_all() };

        0
    }

    fn mprotect(start: usize, len: usize, prot: u32) -> isize {
        let prot: MappingFlags = MMapProtFlag::from_bits(prot)
            .expect("MMap prot not supported.")
            .into();

        info!(
            "[mprotect] addr: [{:?}, {:?}), flags: {:?}",
            start,
            start + len,
            prot
        );

        let current = CurrentTask::try_get().expect("No current task");
        current.memory_set.lock().mprotect(start.into(), len, prot);

        unsafe { riscv::asm::sfence_vma_all() };

        0
    }
}

pub fn handle_page_fault(addr: VirtAddr, flags: MappingFlags) {
    info!("'page fault' addr: {:?}, flags: {:?}", addr, flags);
    let current = CurrentTask::try_get().expect("No current task");

    current.memory_set.lock().handle_page_fault(addr, flags);

    unsafe { riscv::asm::sfence_vma_all() };
}
