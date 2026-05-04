use std::{
    io,
    io::{Read, Seek, SeekFrom, Write},
};

use super::{ReadAt, Size, WriteAt};

/// Adapts a `ReadAt` or `WriteAt` into a `Read` or `Write`.
///
/// This wraps anything that read and write at offsets, turning into an object
/// that can read or write at a file position. This allows you to use those
/// types with all the many functions that expect a `Read` or `Write`.
///
/// Note that seeking on `Cursor` has limited functionality. We don't know how
/// many bytes are available, so we can't use `SeekFrom::End`.
/// See [`SizeCursor`][SizeCursor] for another option.
///
/// [SizeCursor]: struct.SizeCursor.html
///
/// # Examples
///
/// ```no_run
/// # use std::io::{self, Result, Read};
/// # use std::fs::File;
/// use positioned_io::{ReadAt, Cursor};
///
/// struct NetworkStorage {
///     // A remote disk that supports random access.
/// }
/// # impl NetworkStorage {
/// #   fn new(i: i32) -> Self { NetworkStorage { } }
/// # }
///
/// impl ReadAt for NetworkStorage {
///     // ...
/// #   fn read_at(&self, pos: u64, buf: &mut [u8]) -> Result<usize> {
/// #       Ok(0)
/// #   }
/// }
///
/// # const SOME_LOCATION: i32 = 1;
/// # fn foo() -> Result<()> {
/// // Adapt our storage into a Read.
/// let storage = NetworkStorage::new(SOME_LOCATION);
/// let curs = Cursor::new_pos(storage, 1 << 30);
///
/// // Copy a segment to a file.
/// let mut input = curs.take(1 << 20);
/// let mut output = File::create("segment.out")?;
/// io::copy(&mut input, &mut output)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Cursor<I> {
    io: I,
    pos: u64,
}

impl<I> Cursor<I> {
    /// Create a new `Cursor` which starts reading at a specified offset.
    ///
    /// Pass in a `ReadAt` or `WriteAt` as `io`.
    #[inline]
    pub fn new_pos(io: I, pos: u64) -> Self {
        Cursor { io, pos }
    }

    /// Create a new Cursor which starts reading at offset zero.
    ///
    /// Pass in a `ReadAt` or `WriteAt` as `io`.
    #[inline]
    pub fn new(io: I) -> Self {
        Self::new_pos(io, 0)
    }

    /// Consume `self` and yield the inner `ReadAt` or `WriteAt`.
    #[inline]
    pub fn into_inner(self) -> I {
        self.io
    }

    /// Borrow the inner `ReadAt` or `WriteAt`.
    #[inline]
    pub fn get_ref(&self) -> &I {
        &self.io
    }

    /// Borrow the inner `ReadAt` or `WriteAt` mutably.
    #[inline]
    pub fn get_mut(&mut self) -> &mut I {
        &mut self.io
    }

    /// Get the current read/write position.
    #[inline]
    pub fn position(&self) -> u64 {
        self.pos
    }

    /// Set the current read/write position.
    #[inline]
    pub fn set_position(&mut self, pos: u64) {
        self.pos = pos;
    }
}

impl<I> Seek for Cursor<I> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Start(p) => self.pos = p,
            SeekFrom::Current(p) => {
                let pos = self.pos as i64 + p;
                if pos < 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "seek to a negative position",
                    ));
                }
                self.pos = pos as u64;
            }
            SeekFrom::End(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "seek from unknown end",
                ))
            }
        }
        Ok(self.pos)
    }
}

impl<I: ReadAt> Read for Cursor<I> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes = self.get_ref().read_at(self.pos, buf)?;
        self.pos += bytes as u64;
        Ok(bytes)
    }
}

impl<I: WriteAt> Write for Cursor<I> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let pos = self.pos;
        let bytes = self.get_mut().write_at(pos, buf)?;
        self.pos += bytes as u64;
        Ok(bytes)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        WriteAt::flush(self.get_mut())
    }
}

/// Adapts a `ReadAt` or `WriteAt` into a `Read` or `Write`, with better
/// seeking.
///
/// This is just like [`Cursor`][Cursor], except that it requires an object
/// that implements [`Size`][Size], and that it can seek from the end of the
/// I/O object.
///
/// Eventually it will be legal to specialize `Cursor` for types that implement
/// `Size`, see [RFC 1210][RFC].
///
/// [Cursor]: struct.Cursor.html
/// [Size]: trait.Size.html
/// [RFC]: https://github.com/rust-lang/rfcs/pull/1210
#[derive(Debug, Clone)]
pub struct SizeCursor<I: Size> {
    cursor: Cursor<I>,
}

impl<I: Size> SizeCursor<I> {
    /// Create a new `SizeCursor` which starts reading at a specified offset.
    ///
    /// Pass in a `ReadAt` or `WriteAt` as `io`.
    #[inline]
    pub fn new_pos(io: I, pos: u64) -> Self {
        SizeCursor {
            cursor: Cursor::new_pos(io, pos),
        }
    }

    /// Create a new `SizeCursor` which starts reading at offset zero.
    ///
    /// Pass in a `ReadAt` or `WriteAt` as `io`.
    #[inline]
    pub fn new(io: I) -> Self {
        SizeCursor {
            cursor: Cursor::new(io),
        }
    }

    #[inline]
    pub fn as_cursor(&self) -> &Cursor<I> {
        &self.cursor
    }

    #[inline]
    pub fn as_cursor_mut(&mut self) -> &mut Cursor<I> {
        &mut self.cursor
    }

    #[inline]
    pub fn into_cursor(self) -> Cursor<I> {
        self.cursor
    }

    #[inline]
    pub fn into_inner(self) -> I {
        self.cursor.io
    }

    #[inline]
    pub fn get_ref(&self) -> &I {
        &self.cursor.io
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut I {
        &mut self.cursor.io
    }

    #[inline]
    pub fn position(&self) -> u64 {
        self.cursor.position()
    }

    #[inline]
    pub fn set_position(&mut self, pos: u64) {
        self.cursor.set_position(pos)
    }
}

impl<I: Size + ReadAt> Read for SizeCursor<I> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.cursor.read(buf)
    }
}

impl<I: Size + WriteAt> Write for SizeCursor<I> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.cursor.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.cursor.flush()
    }
}

// We know how to seek from the end for SizeCursor.
impl<I: Size> Seek for SizeCursor<I> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let pos = match pos {
            SeekFrom::Start(p) => p as i64,
            SeekFrom::Current(p) => self.cursor.pos as i64 + p,
            SeekFrom::End(p) => match self.get_ref().size() {
                Err(e) => return Err(e),
                Ok(None) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "seek from unknown end",
                    ))
                }
                Ok(Some(s)) => s as i64 + p,
            },
        };
        self.cursor.pos = pos as u64;
        Ok(self.cursor.pos)
    }
}
