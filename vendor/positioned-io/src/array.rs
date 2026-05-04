use std::{cmp::min, io};

use super::{ReadAt, Size, WriteAt};

impl ReadAt for &[u8] {
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
        if pos >= self.len() as u64 {
            return Ok(0);
        }
        let pos = pos as usize;
        let bytes = min(buf.len(), self.len() - pos);
        buf[..bytes].copy_from_slice(&self[pos..(pos + bytes)]);
        Ok(bytes)
    }
}

impl ReadAt for &mut [u8] {
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
        let immutable: &[u8] = self;
        immutable.read_at(pos, buf)
    }
}

impl WriteAt for &mut [u8] {
    fn write_at(&mut self, pos: u64, buf: &[u8]) -> io::Result<usize> {
        if pos >= self.len() as u64 {
            return Ok(0);
        }
        let pos = pos as usize;
        let bytes = min(buf.len(), self.len() - pos);
        self[pos..(pos + bytes)].copy_from_slice(&buf[..bytes]);
        Ok(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Size for &[u8] {
    fn size(&self) -> io::Result<Option<u64>> {
        Ok(Some(self.len() as u64))
    }
}

impl Size for &mut [u8] {
    fn size(&self) -> io::Result<Option<u64>> {
        Ok(Some(self.len() as u64))
    }
}
