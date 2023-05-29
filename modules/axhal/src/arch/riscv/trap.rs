use page_table::MappingFlags;
use riscv::register::{
    scause::{self, Exception as E, Trap},
    stval,
};

use crate::{mem::virt_to_phys, trap::handle_page_fault};

use super::TrapFrame;

include_asm_marcos!();

core::arch::global_asm!(
    include_str!("trap.S"),
    trapframe_size = const core::mem::size_of::<TrapFrame>(),
);

fn handle_breakpoint(sepc: &mut usize) {
    debug!("Exception(Breakpoint) @ {:#x} ", sepc);
    *sepc += 2
}

#[no_mangle]
fn riscv_trap_handler(tf: &mut TrapFrame, from_user: bool) {
    let scause = scause::read();
    match scause.cause() {
        Trap::Exception(E::Breakpoint) => handle_breakpoint(&mut tf.sepc),

        #[cfg(feature = "syscall")]
        Trap::Exception(E::UserEnvCall) => {
            trace!("Handling user syscall.");
            // set sret to the next instruction
            tf.sepc += 4;

            tf.regs.a0 = crate::trap::handle_user_ecall(
                tf.regs.a7,
                [
                    tf.regs.a0, tf.regs.a1, tf.regs.a2, tf.regs.a3, tf.regs.a4, tf.regs.a5,
                ],
            ) as usize;

            info!("Syscall handled. Returning to U mode.");
        }

        Trap::Exception(E::InstructionPageFault) => {
            if !from_user {
                unimplemented!("I page fault from kernel");
            }

            let addr = stval::read();

            handle_page_fault(addr.into(), MappingFlags::USER | MappingFlags::EXECUTE);
        }

        Trap::Exception(E::LoadPageFault) => {
            let addr = stval::read();

            handle_page_fault(addr.into(), MappingFlags::USER | MappingFlags::READ);
        }

        Trap::Exception(E::StorePageFault) => {
            let addr = stval::read();

            handle_page_fault(addr.into(), MappingFlags::USER | MappingFlags::WRITE);
        }

        Trap::Interrupt(_) => crate::trap::handle_irq_extern(scause.bits()),
        _ => {
            error!("instruction: 0x{:x}", unsafe { *(tf.sepc as *const u32) });
            panic!(
                "Unhandled trap {:?} @ {:#x}, phys addr: {:#x}\n{:#x?}",
                scause.cause(),
                tf.sepc,
                virt_to_phys(tf.sepc.into()),
                tf
            );
        }
    }
}
