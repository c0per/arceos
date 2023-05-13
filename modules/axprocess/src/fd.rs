use crate::stdio::{Stderr, Stdin, Stdout};
use alloc::{sync::Arc, vec, vec::Vec};
use axfs::api::{File, FileExt};
use core::{
    cell::RefCell,
    ops::{Deref, DerefMut},
};

pub struct FdList(Vec<Option<Arc<RefCell<dyn axfs::api::FileExt + Send + Sync>>>>);

impl FdList {
    pub fn query_fd(
        &self,
        fd: usize,
    ) -> Option<&Arc<RefCell<dyn axfs::api::FileExt + Send + Sync>>> {
        match self.get(fd) {
            Some(file) => file.as_ref(),
            None => None,
        }
    }

    pub fn query_fd_mut(
        &mut self,
        fd: usize,
    ) -> Option<&mut Option<Arc<RefCell<dyn axfs::api::FileExt + Send + Sync>>>> {
        self.get_mut(fd)
    }

    pub fn alloc_fd(&mut self, file: File) -> isize {
        let new_fd = match self
            .0
            .iter_mut()
            .enumerate()
            .find(|(_fd, slot)| slot.is_none())
        {
            Some((fd, slot)) => {
                slot.insert(Arc::new(RefCell::new(file)));
                fd
            }
            None => {
                self.0.push(Some(Arc::new(RefCell::new(file))));
                self.0.len() - 1
            }
        };

        new_fd as isize
    }
}

impl Deref for FdList {
    type Target = Vec<Option<Arc<RefCell<dyn axfs::api::FileExt + Send + Sync>>>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for FdList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Default for FdList {
    fn default() -> Self {
        Self(vec![
            Some(Arc::new(RefCell::new(Stdin))),
            Some(Arc::new(RefCell::new(Stdout))),
            Some(Arc::new(RefCell::new(Stderr))),
        ])
    }
}

impl Clone for FdList {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
