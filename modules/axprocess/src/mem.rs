use axhal::paging::MappingFlags;
use axsyscall::mem::SyscallMem;
use bitflags::bitflags;
use bitflags::BitFlags;

use crate::scheduler::CurrentTask;

struct SyscallMemImpl;

bitflags! {
    pub struct MMapProtFlag: u32 {
        const PROT_READ = 1 << 0;
        const PROT_WRITE = 1 << 1;
        const PROT_EXEC = 1 << 2;
    }
}

bitflags! {
    pub struct MMapFlags: u32 {
        const MAP_SHARED = 1 << 0;
        const MAP_PRIVATE = 1 << 1;

        const MAP_FIXED = 1 << 4;
        const MAP_ANONYMOUS = 1 << 5;
        const MAP_NORESERVE = 1 << 14;
    }
}

impl From<MMapProtFlag> for MappingFlags {
    fn from(value: MMapProtFlag) -> Self {
        let mut flags = Self::default();

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

        // no file
        if flags.contains(MMapFlags::MAP_ANONYMOUS) {
            let current = CurrentTask::try_get().expect("No current task");
            current.memory_set.mmap(start, len, prot, fixed);
        } else {
            unimplemented!()
        }

        0
    }
}
