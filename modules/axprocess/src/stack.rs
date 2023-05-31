use axhal::{arch::TrapFrame, mem::VirtAddr};
use core::{alloc::Layout, mem::size_of, ptr::NonNull};

#[derive(Clone)]
pub(crate) struct TaskStack {
    ptr: NonNull<u8>,
    layout: Layout,
}

impl TaskStack {
    pub fn alloc(size: usize) -> Self {
        let layout = Layout::from_size_align(size, 16).unwrap();
        Self {
            ptr: NonNull::new(unsafe { alloc::alloc::alloc(layout) }).unwrap(),
            layout,
        }
    }

    pub const fn top(&self) -> VirtAddr {
        unsafe { core::mem::transmute(self.ptr.as_ptr().add(self.layout.size())) }
    }

    pub fn bottom(&self) -> VirtAddr {
        VirtAddr::from(self.ptr.as_ptr() as usize)
    }

    pub fn trap_frame_ptr(&self) -> *const TrapFrame {
        (usize::from(self.top()) - size_of::<TrapFrame>()) as *const TrapFrame
    }

    pub fn trap_frame_ptr_mut(&self) -> *mut TrapFrame {
        (usize::from(self.top()) - size_of::<TrapFrame>()) as *mut TrapFrame
    }

    pub fn trap_frame_mut(&self) -> &mut TrapFrame {
        unsafe { &mut *self.trap_frame_ptr_mut() }
    }
}

impl Drop for TaskStack {
    fn drop(&mut self) {
        unsafe { alloc::alloc::dealloc(self.ptr.as_ptr(), self.layout) }
    }
}
