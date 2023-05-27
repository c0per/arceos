use crate_interface::{call_interface, def_interface};
use memory_addr::VirtAddr;
use page_table::MappingFlags;

#[def_interface]
pub trait TrapHandler {
    fn handle_irq(irq_num: usize);

    #[cfg(feature = "syscall")]
    fn handle_user_ecall(syscall_id: usize, args: [usize; 6]) -> isize;

    fn handle_page_fault(addr: VirtAddr, flags: MappingFlags);
}

/// Call the external IRQ handler.
#[allow(dead_code)]
pub(crate) fn handle_irq_extern(irq_num: usize) {
    call_interface!(TrapHandler::handle_irq, irq_num);
}

/// Call the external syscall handler.
#[cfg(feature = "syscall")]
pub(crate) fn handle_user_ecall(syscall_id: usize, args: [usize; 6]) -> isize {
    call_interface!(TrapHandler::handle_user_ecall, syscall_id, args)
}

pub(crate) fn handle_page_fault(addr: VirtAddr, flags: MappingFlags) {
    call_interface!(TrapHandler::handle_page_fault, addr, flags);
}
