use std::{cell::RefCell, io};

use super::{ReadAt, Size, WriteAt};

impl<R: ReadAt + ?Sized> ReadAt for &R {
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
        R::read_at(self, pos, buf)
    }
}

impl<R: ReadAt + ?Sized> ReadAt for &mut R {
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
        R::read_at(self, pos, buf)
    }
}

impl<W: WriteAt + ?Sized> WriteAt for &mut W {
    fn write_at(&mut self, pos: u64, buf: &[u8]) -> io::Result<usize> {
        W::write_at(self, pos, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        W::flush(self)
    }
}

impl<S: Size + ?Sized> Size for &S {
    fn size(&self) -> io::Result<Option<u64>> {
        S::size(self)
    }
}
impl<S: Size + ?Sized> Size for &mut S {
    fn size(&self) -> io::Result<Option<u64>> {
        S::size(self)
    }
}

impl<R: ReadAt> ReadAt for &RefCell<R> {
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
        self.borrow().read_at(pos, buf)
    }
}

impl<W: WriteAt> WriteAt for &RefCell<W> {
    fn write_at(&mut self, pos: u64, buf: &[u8]) -> io::Result<usize> {
        self.borrow_mut().write_at(pos, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.borrow_mut().flush()
    }
}

impl<S: Size> Size for &RefCell<S> {
    fn size(&self) -> io::Result<Option<u64>> {
        self.borrow().size()
    }
}

impl<R: ReadAt + ?Sized> ReadAt for Box<R> {
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
        (**self).read_at(pos, buf)
    }
}

impl<R: WriteAt + ?Sized> WriteAt for Box<R> {
    fn write_at(&mut self, pos: u64, buf: &[u8]) -> io::Result<usize> {
        (**self).write_at(pos, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        (**self).flush()
    }
}

impl<S: Size + ?Sized> Size for Box<S> {
    fn size(&self) -> io::Result<Option<u64>> {
        (**self).size()
    }
}
