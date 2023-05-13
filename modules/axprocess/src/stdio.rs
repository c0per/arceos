use axfs::api::FileExt;
use axhal::console;
use axio::{Read, Seek, Write};

pub(crate) struct Stdin;

impl Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> axio::Result<usize> {
        todo!()
    }
}

impl Write for Stdin {
    fn write(&mut self, buf: &[u8]) -> axio::Result<usize> {
        panic!("Writing to stdin")
    }

    fn flush(&mut self) -> axio::Result {
        panic!("Flushing stdin")
    }
}

impl Seek for Stdin {
    fn seek(&mut self, pos: axio::SeekFrom) -> axio::Result<u64> {
        todo!()
    }
}

impl FileExt for Stdin {}

pub(crate) struct Stdout;

impl Read for Stdout {
    fn read(&mut self, buf: &mut [u8]) -> axio::Result<usize> {
        panic!("Reading from stdout")
    }
}

impl Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> axio::Result<usize> {
        console::write_bytes(buf);

        Ok(buf.len())
    }

    /// Stdout is always flushed
    fn flush(&mut self) -> axio::Result {
        Ok(())
    }
}

impl Seek for Stdout {
    fn seek(&mut self, pos: axio::SeekFrom) -> axio::Result<u64> {
        todo!()
    }
}

impl FileExt for Stdout {}

pub(crate) struct Stderr;

impl Read for Stderr {
    fn read(&mut self, buf: &mut [u8]) -> axio::Result<usize> {
        panic!("Reading from stderr")
    }
}

impl Write for Stderr {
    fn write(&mut self, buf: &[u8]) -> axio::Result<usize> {
        console::write_bytes(buf);

        Ok(buf.len())
    }

    /// Stderr is always flushed
    fn flush(&mut self) -> axio::Result {
        Ok(())
    }
}

impl Seek for Stderr {
    fn seek(&mut self, pos: axio::SeekFrom) -> axio::Result<u64> {
        todo!()
    }
}

impl FileExt for Stderr {}
