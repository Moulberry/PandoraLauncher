//! This crate allows you to specify an offset for reads and writes,
//! without changing the current position in a file. This is similar to
//! [`pread()` and `pwrite()`][pread] in C.
//!
//! The major advantages of this type of I/O are:
//!
//! * You don't need to seek before doing a random-access read or write, which is convenient.
//! * Reads don't modify the file at all, so don't require mutability.
//!
//! [pread]: http://man7.org/linux/man-pages/man2/pread.2.html
//!
//! # Examples
//!
//! Read the fifth 512-byte sector of a file:
//!
//! ```
//! # use std::error::Error;
//! #
//! # fn try_main() -> Result<(), Box<dyn Error>> {
//! use std::fs::File;
//! use positioned_io::ReadAt;
//!
//! // note that file does not need to be mut
//! let file = File::open("tests/pi.txt")?;
//!
//! // read up to 512 bytes
//! let mut buf = [0; 512];
//! let bytes_read = file.read_at(2048, &mut buf)?;
//! #     assert!(buf.starts_with(b"4"));
//! #     Ok(())
//! # }
//! #
//! # fn main() {
//! #     try_main().unwrap();
//! # }
//! ```
//!
//! **Note:** If possible use the
//! [`RandomAccessFile`](struct.RandomAccessFile.html) wrapper. `ReadAt`
//! directly on `File` is very slow on Windows.
//!
//! Write an integer to the middle of a file:
//!
//! ```no_run
//! # #[cfg(feature = "byteorder")]
//! # extern crate byteorder;
//! # use std::io;
//! #
//! # fn try_main() -> io::Result<()> {
//! # #[cfg(feature = "byteorder")]
//! # {
//! use std::fs::OpenOptions;
//! use positioned_io::WriteAt;
//! use byteorder::{ByteOrder, LittleEndian};
//!
//! // put the integer in a buffer
//! let mut buf = [0; 4];
//! LittleEndian::write_u32(&mut buf, 1234);
//!
//! // write it to the file
//! let mut file = OpenOptions::new().write(true).open("foo.data")?;
//! file.write_all_at(1 << 20, &buf)?;
//! # }
//! #     Ok(())
//! # }
//! # fn main() {
//! #     try_main().unwrap()
//! # }
//! ```
//!
//! Or, more simply:
//!
//! ```no_run
//! # #[cfg(feature = "byteorder")]
//! # extern crate byteorder;
//! # use std::io;
//! #
//! # fn try_main() -> io::Result<()> {
//! # #[cfg(feature = "byteorder")]
//! # {
//! use std::fs::OpenOptions;
//! use byteorder::LittleEndian;
//! use positioned_io::WriteBytesAtExt;
//!
//! let mut file = OpenOptions::new().write(true).open("foo.data")?;
//! file.write_u32_at::<LittleEndian>(1 << 20, 1234)?;
//! # }
//! #     Ok(())
//! # }
//! # fn main() {
//! #     try_main().unwrap()
//! # }
//! ```
//!
//! Read from anything else that supports `ReadAt`, like a byte array:
//!
//! ```rust
//! # #[cfg(feature = "byteorder")]
//! # extern crate byteorder;
//! # use std::io;
//! #
//! # fn try_main() -> io::Result<()> {
//! # #[cfg(feature = "byteorder")]
//! {
//! use byteorder::BigEndian;
//! use positioned_io::ReadBytesAtExt;
//!
//! let buf = [0, 5, 254, 212, 0, 3];
//! let n = buf.as_ref().read_i16_at::<BigEndian>(2)?;
//! assert_eq!(n, -300);
//! # }
//! #     Ok(())
//! # }
//! # fn main() {
//! #     try_main().unwrap()
//! # }
//! ```

#![doc(html_root_url = "https://docs.rs/positioned-io/0.3.5")]
#![warn(missing_debug_implementations)]
#![warn(bare_trait_objects)]

#[cfg(feature = "byteorder")]
extern crate byteorder;
#[cfg(unix)]
extern crate libc;

mod cursor;
pub use crate::cursor::{Cursor, SizeCursor};

mod slice;
pub use crate::slice::Slice;

#[cfg(feature = "byteorder")]
mod byteio;
use std::{fs::File, io};

#[cfg(feature = "byteorder")]
pub use crate::byteio::{ByteIo, ReadBytesAtExt, WriteBytesAtExt};

/// Trait for reading bytes at an offset.
///
/// Implementations should be able to read bytes without changing any sort of
/// read position. Self should not change at all. Buffering reads is unlikely
/// to be useful, since each time `read_at()` is called, the position may be
/// completely different.
///
/// # Examples
///
/// Read the fifth 512-byte sector of a file:
///
/// ```
/// # use std::error::Error;
/// #
/// # fn try_main() -> Result<(), Box<dyn Error>> {
/// use std::fs::File;
/// use positioned_io::ReadAt;
///
/// // note that file does not need to be mut
/// let file = File::open("tests/pi.txt")?;
///
/// // read up to 512 bytes
/// let mut buf = [0; 512];
/// let bytes_read = file.read_at(2048, &mut buf)?;
/// #     assert!(buf.starts_with(b"4"));
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
pub trait ReadAt {
    /// Reads bytes from an offset in this source into a buffer, returning how
    /// many bytes were read.
    ///
    /// This function may yield fewer bytes than the size of `buf`, if it was
    /// interrupted or hit the "end of file".
    ///
    /// See [`Read::read()`](https://doc.rust-lang.org/std/io/trait.Read.html#tymethod.read)
    /// for details.
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize>;

    /// Reads the exact number of bytes required to fill `buf` from an offset.
    ///
    /// Errors if the "end of file" is encountered before filling the buffer.
    ///
    /// See [`Read::read_exact()`](https://doc.rust-lang.org/std/io/trait.Read.html#method.read_exact)
    /// for details.
    fn read_exact_at(&self, mut pos: u64, mut buf: &mut [u8]) -> io::Result<()> {
        while !buf.is_empty() {
            match self.read_at(pos, buf) {
                Ok(0) => break,
                Ok(n) => {
                    let tmp = buf;
                    buf = &mut tmp[n..];
                    pos += n as u64;
                }
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        if !buf.is_empty() {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "failed to fill whole buffer",
            ))
        } else {
            Ok(())
        }
    }
}

/// Trait for writing bytes at an offset.
///
/// Implementations should be able to write bytes at an offset, without
/// changing any sort of write position. Self should not change at all.
///
/// When writing beyond the end of the underlying object it is extended and
/// intermediate bytes are filled with the value 0.
///
/// # Examples
///
/// ```no_run
/// # use std::error::Error;
/// #
/// # fn try_main() -> Result<(), Box<dyn Error>> {
/// use std::fs::OpenOptions;
/// use positioned_io::WriteAt;
///
/// let mut file = OpenOptions::new().write(true).open("tests/pi.txt")?;
///
/// // write some bytes
/// let bytes_written = file.write_at(2, b"1415926535897932384626433")?;
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
pub trait WriteAt {
    /// Writes bytes from a buffer to an offset, returning the number of bytes
    /// written.
    ///
    /// This function may write fewer bytes than the size of `buf`, for example
    /// if it is interrupted.
    ///
    /// See [`Write::write()`](https://doc.rust-lang.org/std/io/trait.Write.html#tymethod.write)
    /// for details.
    fn write_at(&mut self, pos: u64, buf: &[u8]) -> io::Result<usize>;

    /// Writes a complete buffer at an offset.
    ///
    /// Errors if it could not write the entire buffer.
    ///
    /// See [`Write::write_all()`](https://doc.rust-lang.org/std/io/trait.Write.html#method.write_all)
    /// for details.
    fn write_all_at(&mut self, mut pos: u64, mut buf: &[u8]) -> io::Result<()> {
        while !buf.is_empty() {
            match self.write_at(pos, buf) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to write whole buffer",
                    ))
                }
                Ok(n) => {
                    buf = &buf[n..];
                    pos += n as u64;
                }
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Flush this writer, ensuring that any intermediately buffered data
    /// reaches its destination.
    ///
    /// This should rarely do anything, since buffering is not very useful for
    /// positioned writes.
    ///
    /// This should be equivalent to
    /// [`Write::flush()`](https://doc.rust-lang.org/std/io/trait.Write.html#tymethod.flush),
    /// so it does not actually sync changes to disk when writing a `File`.
    /// Use
    /// [`File::sync_data()`](https://doc.rust-lang.org/std/fs/struct.File.html#method.sync_data)
    /// instead.
    fn flush(&mut self) -> io::Result<()>;
}

/// Trait to get the size in bytes of an I/O object.
///
/// Implementing this for a types with `ReadAt` or `WriteAt` makes it easier
/// for users to predict whether they will read past end-of-file. However, it
/// may not be possible to implement for certain readers or writers that have
/// unknown size.
///
/// # Examples
///
/// ```no_run
/// # use std::error::Error;
/// #
/// # fn try_main() -> Result<(), Box<dyn Error>> {
/// use std::fs::File;
/// use positioned_io::Size;
///
/// let file = File::open("tests/pi.txt")?;
/// let size = file.size()?;
/// assert_eq!(size, Some(1000002));
///
/// // some special files do not have a known size
/// let file = File::open("/dev/stdin")?;
/// let size = file.size()?;
/// assert_eq!(size, None);
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #    try_main().unwrap();
/// # }
/// ```
pub trait Size {
    /// Get the size of this object, in bytes.
    ///
    /// This function may return `Ok(None)` if the size is unknown, for example
    /// for pipes.
    fn size(&self) -> io::Result<Option<u64>>;
}

impl Size for File {
    fn size(&self) -> io::Result<Option<u64>> {
        let md = self.metadata()?;
        if md.is_file() {
            Ok(Some(md.len()))
        } else {
            Ok(None)
        }
    }
}

// Implementation for Unix files.
#[cfg(unix)]
mod unix;

// Implementation for Windows files.
#[cfg(windows)]
mod windows;

// RandomAccess file wrapper.
#[cfg(any(windows, unix))]
mod raf;
#[cfg(any(windows, unix))]
pub use crate::raf::RandomAccessFile;

// Implementation for arrays, vectors.
mod array;
mod refs;
mod vec;

#[cfg(test)]
mod tests {
    use super::*;

    struct _AssertObjectSafe1(Box<dyn ReadAt>);
    struct _AssertObjectSafe2(Box<dyn WriteAt>);
    struct _AssertObjectSafe3(Box<dyn Size>);
}
