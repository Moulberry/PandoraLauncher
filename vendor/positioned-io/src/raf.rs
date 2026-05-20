#[cfg(windows)]
use std::io::{Seek, SeekFrom};
#[cfg(unix)]
use std::os::unix::fs::FileExt;
#[cfg(windows)]
use std::os::windows::fs::FileExt;
use std::{fs::File, io, io::Write, path::Path, sync::Arc};

use super::{ReadAt, Size, WriteAt};

/// A wrapper for `File` that provides optimized random access through
/// `ReadAt` and `WriteAt`.
///
/// * On Unix the operating system is advised that reads will be in random
///   order (`FADV_RANDOM`).
/// * On Windows the implementation is orders of magnitude faster than `ReadAt`
///   directly on `File`.
///
/// # Examples
///
/// Read the fifth 512-byte sector of a file:
///
/// ```
/// # use std::error::Error;
/// #
/// # fn try_main() -> Result<(), Box<dyn Error>> {
/// use positioned_io::{RandomAccessFile, ReadAt};
///
/// // open a file (note: binding does not need to be mut)
/// let raf = RandomAccessFile::open("tests/pi.txt")?;
///
/// // read up to 512 bytes
/// let mut buf = [0; 512];
/// let bytes_read = raf.read_at(2048, &mut buf)?;
/// #     assert!(buf.starts_with(b"4"));
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
#[derive(Debug)]
pub struct RandomAccessFile {
    file: File,
    #[cfg(not(unix))]
    pos: u64,
}

impl RandomAccessFile {
    /// [Opens](https://doc.rust-lang.org/std/fs/struct.File.html#method.open)
    /// a file for random access.
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<RandomAccessFile> {
        RandomAccessFile::try_new(File::open(path)?)
    }

    /// Creates a `RandomAccessFile` wrapper around a `File`.
    pub fn try_new(file: File) -> io::Result<RandomAccessFile> {
        RandomAccessFile::try_new_impl(file)
    }

    #[cfg(all(unix, target_os = "linux"))]
    fn try_new_impl(file: File) -> io::Result<RandomAccessFile> {
        unsafe {
            use std::os::unix::io::AsRawFd;
            libc::posix_fadvise(file.as_raw_fd(), 0, 0, libc::POSIX_FADV_RANDOM);
        }

        Ok(RandomAccessFile { file })
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    fn try_new_impl(file: File) -> io::Result<RandomAccessFile> {
        Ok(RandomAccessFile { file })
    }

    #[cfg(not(unix))]
    fn try_new_impl(mut file: File) -> io::Result<RandomAccessFile> {
        let pos = file.seek(SeekFrom::Current(0))?;
        Ok(RandomAccessFile { file, pos })
    }

    /// Tries to unwrap the inner `File`.
    pub fn try_into_inner(self) -> Result<File, (RandomAccessFile, io::Error)> {
        RandomAccessFile::try_into_inner_impl(self)
    }

    #[cfg(unix)]
    fn try_into_inner_impl(self) -> Result<File, (RandomAccessFile, io::Error)> {
        Ok(self.file)
    }

    #[cfg(not(unix))]
    fn try_into_inner_impl(mut self) -> Result<File, (RandomAccessFile, io::Error)> {
        match self.file.seek(SeekFrom::Start(self.pos)) {
            Ok(_) => Ok(self.file),
            Err(err) => Err((self, err)),
        }
    }
}

#[cfg(unix)]
impl ReadAt for RandomAccessFile {
    #[inline]
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
        FileExt::read_at(&self.file, buf, pos)
    }
}

#[cfg(unix)]
impl WriteAt for &RandomAccessFile {
    fn write_at(&mut self, pos: u64, buf: &[u8]) -> io::Result<usize> {
        FileExt::write_at(&self.file, buf, pos)
    }

    fn flush(&mut self) -> io::Result<()> {
        Write::flush(&mut &self.file)
    }
}

#[cfg(windows)]
impl ReadAt for RandomAccessFile {
    #[inline]
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
        FileExt::seek_read(&self.file, buf, pos)
    }
}

#[cfg(windows)]
impl WriteAt for &RandomAccessFile {
    fn write_at(&mut self, pos: u64, buf: &[u8]) -> io::Result<usize> {
        FileExt::seek_write(&self.file, buf, pos)
    }

    fn flush(&mut self) -> io::Result<()> {
        Write::flush(&mut &self.file)
    }
}

impl WriteAt for RandomAccessFile {
    fn write_at(&mut self, pos: u64, buf: &[u8]) -> io::Result<usize> {
        WriteAt::write_at(&mut &*self, pos, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        WriteAt::flush(&mut &*self)
    }
}

impl Size for RandomAccessFile {
    fn size(&self) -> io::Result<Option<u64>> {
        self.file.size()
    }
}

impl ReadAt for Arc<RandomAccessFile> {
    #[inline]
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
        (**self).read_at(pos, buf)
    }
}

impl Size for Arc<RandomAccessFile> {
    #[inline]
    fn size(&self) -> io::Result<Option<u64>> {
        (**self).size()
    }
}
