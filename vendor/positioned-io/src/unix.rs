use std::{fs::File, io, io::Write, os::unix::fs::FileExt};

use super::{ReadAt, WriteAt};

impl ReadAt for File {
    #[inline]
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
        FileExt::read_at(self, buf, pos)
    }
}

impl WriteAt for File {
    fn write_at(&mut self, pos: u64, buf: &[u8]) -> io::Result<usize> {
        FileExt::write_at(self, buf, pos)
    }

    fn flush(&mut self) -> io::Result<()> {
        Write::flush(self)
    }
}
