use alloc::sync::Arc;
use axfs::api::FileExt;
use axio::{Read, Seek, SeekFrom};
use spinlock::SpinNoIrq;

pub struct MemBackend {
    file: Arc<SpinNoIrq<dyn FileExt>>,
}

impl MemBackend {
    pub fn new(file: Arc<SpinNoIrq<dyn FileExt>>, offset: u64) -> Self {
        let _ = file.lock().seek(SeekFrom::Start(offset)).unwrap();

        Self { file }
    }

    pub fn clone_with_delta(&self, delta: i64) -> Self {
        let mut new_backend = self.clone();

        let _ = new_backend.seek(SeekFrom::Current(delta)).unwrap();

        new_backend
    }

    pub fn read_from_seek(&mut self, pos: SeekFrom, buf: &mut [u8]) -> Result<usize, axio::Error> {
        self.file.lock().read_from_seek(pos, buf)
    }

    pub fn write_to_seek(&self, pos: SeekFrom, buf: &[u8]) -> Result<usize, axio::Error> {
        self.file.lock().write_to_seek(pos, buf)
    }
}

impl Clone for MemBackend {
    fn clone(&self) -> Self {
        let file = self.file.lock().clone_file();

        Self {
            file: Arc::new(SpinNoIrq::new(file)),
        }
    }
}

impl Seek for MemBackend {
    fn seek(&mut self, pos: SeekFrom) -> axio::Result<u64> {
        self.file.lock().seek(pos)
    }
}

impl Read for MemBackend {
    fn read(&mut self, buf: &mut [u8]) -> axio::Result<usize> {
        self.file.lock().read(buf)
    }
}
