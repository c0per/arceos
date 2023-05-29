use crate::stdio::{Stderr, Stdin, Stdout};
use alloc::{sync::Arc, vec, vec::Vec};
use axfs::api::FileExt;
use core::ops::{Deref, DerefMut};
use spinlock::SpinNoIrq;

pub struct FdList(Vec<Option<Arc<SpinNoIrq<dyn FileExt + Send + Sync>>>>);

impl FdList {
    /// Query and return a reference to `Arc<SpinNoIrq<...>>`.
    pub fn query_fd(&self, fd: usize) -> Option<&Arc<SpinNoIrq<dyn FileExt + Send + Sync>>> {
        match self.get(fd) {
            Some(file) => file.as_ref(),
            None => None,
        }
    }

    /// Allocate the lowest-numbered available fd. Return the allocated fd.
    pub fn alloc(&mut self, file: Arc<SpinNoIrq<dyn FileExt + Send + Sync>>) -> usize {
        self.alloc_hint(0, file)
    }

    /// Allocate the lowest-numbered available fd which is >= hint. Return the allocated fd.
    pub fn alloc_hint(
        &mut self,
        hint: usize,
        file: Arc<SpinNoIrq<dyn FileExt + Send + Sync>>,
    ) -> usize {
        if hint < self.len() {
            match self
                .iter_mut()
                .enumerate()
                .skip(hint)
                .find(|(_, slot)| slot.is_none())
            {
                Some((fd, slot)) => {
                    let _ = slot.insert(file);
                    fd
                }
                None => {
                    self.push(Some(file));
                    self.len() - 1
                }
            }
        } else {
            // NOTE: save `skip_count` first. `self.len()` is changing in the loop.
            let skip_count = hint - self.len();
            for _ in 0..skip_count {
                self.push(None);
            }

            self.push(Some(file));
            hint
        }
    }
}

impl Deref for FdList {
    type Target = Vec<Option<Arc<SpinNoIrq<dyn FileExt + Send + Sync>>>>;
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
            Some(Arc::new(SpinNoIrq::new(Stdin))),
            Some(Arc::new(SpinNoIrq::new(Stdout))),
            Some(Arc::new(SpinNoIrq::new(Stderr))),
        ])
    }
}

impl Clone for FdList {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
