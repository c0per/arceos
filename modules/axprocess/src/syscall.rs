struct SyscallTaskImpl;

#[crate_interface::impl_interface]
impl axsyscall::task::SyscallTask for SyscallTaskImpl {
    fn exit(status: i32) -> ! {
        panic!("Exit!");
    }
}
