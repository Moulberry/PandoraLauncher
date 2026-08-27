use super::{BufRead, Error, ErrorKind, Read, Result, Seek, SeekFrom, Write};
use core::cmp;

/// A `Cursor` wraps an in-memory buffer and provides it with a
/// [`Seek`] implementation.
///
/// `Cursor`s are used with in-memory buffers, anything implementing
/// [`AsRef`]`<[u8]>`, to allow them to implement [`Read`] and/or [`Write`],
/// allowing these buffers to be used anywhere you might use a reader or writer
/// that does actual I/O.
///
/// The standard library implements some I/O traits on various types which
/// are commonly used as a buffer, like `Cursor<`[`Vec`]`<u8>>` and
/// `Cursor<`[`&[u8]`][bytes]`>`.
///
/// # Examples
///
/// We may want to write bytes to a [`File`] in our production
/// code, but use an in-memory buffer in our tests. We can do this with
/// `Cursor`:
///
/// [bytes]: crate::slice
/// [`File`]: crate::fs::File
///
/// ```
/// use std::io::prelude::*;
/// use no_std_io2::io::{self, Seek, SeekFrom, Write};
/// use std::fs::File;
///
/// // a library function we've written
/// fn write_ten_bytes_at_end<W: Write + Seek>(writer: &mut W) -> io::Result<()> {
///     writer.seek(SeekFrom::End(-10))?;
///
///     for i in 0..10 {
///         writer.write(&[i])?;
///     }
///
///     // all went well
///     Ok(())
/// }
///
/// # #[cfg(feature = "std")]
/// # fn foo() -> io::Result<()> {
/// // Here's some code that uses this library function.
/// //
/// // We might want to use a BufReader here for efficiency, but let's
/// // keep this example focused.
/// let mut file = File::create("foo.txt").map_err(|e| io::Error::from(e))?;
///
/// write_ten_bytes_at_end(&mut file)?;
/// # Ok(())
/// # }
///
/// // now let's write a test
/// #[test]
/// fn test_writes_bytes() {
///     // setting up a real File is much slower than an in-memory buffer,
///     // let's use a cursor instead
///     use no_std_io2::io::Cursor;
///     let mut buff = Cursor::new(vec![0; 15]);
///
///     write_ten_bytes_at_end(&mut buff).unwrap();
///
///     assert_eq!(&buff.get_ref()[5..15], &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
/// }
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Cursor<T> {
    inner: T,
    pos: u64,
}

impl<T> Cursor<T> {
    /// Creates a new cursor wrapping the provided underlying in-memory buffer.
    ///
    /// Cursor initial position is `0` even if underlying buffer (e.g., [`Vec`])
    /// is not empty. So writing to cursor starts with overwriting [`Vec`]
    /// content, not with appending to it.
    ///
    /// # Examples
    ///
    /// ```
    /// use no_std_io2::io::Cursor;
    ///
    /// let buff = Cursor::new(Vec::new());
    /// # fn force_inference(_: &Cursor<Vec<u8>>) {}
    /// # force_inference(&buff);
    /// ```
    pub fn new(inner: T) -> Cursor<T> {
        Cursor { pos: 0, inner }
    }

    /// Consumes this cursor, returning the underlying value.
    ///
    /// # Examples
    ///
    /// ```
    /// use no_std_io2::io::Cursor;
    ///
    /// let buff = Cursor::new(Vec::new());
    /// # fn force_inference(_: &Cursor<Vec<u8>>) {}
    /// # force_inference(&buff);
    ///
    /// let vec = buff.into_inner();
    /// ```
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Gets a reference to the underlying value in this cursor.
    ///
    /// # Examples
    ///
    /// ```
    /// use no_std_io2::io::Cursor;
    ///
    /// let buff = Cursor::new(Vec::new());
    /// # fn force_inference(_: &Cursor<Vec<u8>>) {}
    /// # force_inference(&buff);
    ///
    /// let reference = buff.get_ref();
    /// ```
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Gets a mutable reference to the underlying value in this cursor.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying value as it may corrupt this cursor's position.
    ///
    /// # Examples
    ///
    /// ```
    /// use no_std_io2::io::Cursor;
    ///
    /// let mut buff = Cursor::new(Vec::new());
    /// # fn force_inference(_: &Cursor<Vec<u8>>) {}
    /// # force_inference(&buff);
    ///
    /// let reference = buff.get_mut();
    /// ```
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Returns the current position of this cursor.
    ///
    /// # Examples
    ///
    /// ```
    /// use no_std_io2::io::{Cursor, Seek, SeekFrom};
    /// use std::io::prelude::*;
    ///
    /// let mut buff = Cursor::new(vec![1, 2, 3, 4, 5]);
    ///
    /// assert_eq!(buff.position(), 0);
    ///
    /// buff.seek(SeekFrom::Current(2)).unwrap();
    /// assert_eq!(buff.position(), 2);
    ///
    /// buff.seek(SeekFrom::Current(-1)).unwrap();
    /// assert_eq!(buff.position(), 1);
    /// ```
    pub fn position(&self) -> u64 {
        self.pos
    }

    /// Sets the position of this cursor.
    ///
    /// # Examples
    ///
    /// ```
    /// use no_std_io2::io::Cursor;
    ///
    /// let mut buff = Cursor::new(vec![1, 2, 3, 4, 5]);
    ///
    /// assert_eq!(buff.position(), 0);
    ///
    /// buff.set_position(2);
    /// assert_eq!(buff.position(), 2);
    ///
    /// buff.set_position(4);
    /// assert_eq!(buff.position(), 4);
    /// ```
    pub fn set_position(&mut self, pos: u64) {
        self.pos = pos;
    }
}

impl<T> Seek for Cursor<T>
where
    T: AsRef<[u8]>,
{
    fn seek(&mut self, style: SeekFrom) -> Result<u64> {
        let (base_pos, offset) = match style {
            SeekFrom::Start(n) => {
                self.pos = n;
                return Ok(n);
            }
            SeekFrom::End(n) => (self.inner.as_ref().len() as u64, n),
            SeekFrom::Current(n) => (self.pos, n),
        };
        let new_pos = if offset >= 0 {
            base_pos.checked_add(offset as u64)
        } else {
            base_pos.checked_sub((offset.wrapping_neg()) as u64)
        };
        match new_pos {
            Some(n) => {
                self.pos = n;
                Ok(self.pos)
            }
            None => Err(Error::new(
                ErrorKind::InvalidInput,
                "invalid seek to a negative or overflowing position",
            )),
        }
    }
}

impl<T> Read for Cursor<T>
where
    T: AsRef<[u8]>,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let n = Read::read(&mut self.fill_buf()?, buf)?;
        self.pos += n as u64;
        Ok(n)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        let n = buf.len();
        Read::read_exact(&mut self.fill_buf()?, buf)?;
        self.pos += n as u64;
        Ok(())
    }
}

impl<T> BufRead for Cursor<T>
where
    T: AsRef<[u8]>,
{
    fn fill_buf(&mut self) -> Result<&[u8]> {
        let amt = cmp::min(self.pos, self.inner.as_ref().len() as u64);
        Ok(&self.inner.as_ref()[(amt as usize)..])
    }
    fn consume(&mut self, amt: usize) {
        self.pos += amt as u64;
    }
}

// Non-resizing write implementation
#[inline]
fn slice_write(pos_mut: &mut u64, slice: &mut [u8], buf: &[u8]) -> Result<usize> {
    let pos = cmp::min(*pos_mut, slice.len() as u64);
    let amt = (&mut slice[(pos as usize)..]).write(buf)?;
    *pos_mut += amt as u64;
    Ok(amt)
}

impl Write for Cursor<&mut [u8]> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        slice_write(&mut self.pos, self.inner, buf)
    }

    #[inline]
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

// matching the behaviour of std::io::Cursor<&mut Vec> and std::io::Cursor<Vec>
#[cfg(feature = "alloc")]
fn vec_write_all(pos: usize, vec: &mut alloc::vec::Vec<u8>, buf: &[u8]) {
    let desired_end = pos.saturating_add(buf.len());
    if desired_end > vec.len() {
        vec.resize(desired_end, 0);
    }
    // SAFETY: We have just resized the vector to have at least `buf.len()` bytes of valid write space before `vec.len()`,
    // so we can just write now and don't have to worry about uninitialized bytes or buffer over-runs.
    unsafe {
        core::ptr::copy_nonoverlapping(buf.as_ptr(), vec.as_mut_ptr().add(pos), buf.len());
    }
}

#[cfg(feature = "alloc")]
impl Write for Cursor<&mut alloc::vec::Vec<u8>> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        vec_write_all(self.pos as usize, self.inner, buf);
        self.pos += buf.len() as u64;
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(feature = "alloc")]
impl Write for Cursor<alloc::vec::Vec<u8>> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        vec_write_all(self.pos as usize, &mut self.inner, buf);
        self.pos += buf.len() as u64;
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;
    #[cfg(feature = "alloc")]
    #[test]
    fn test_ref_vec_start_0_cap() {
        let mut out_buf = alloc::vec::Vec::new();
        let mut cursor = Cursor::new(&mut out_buf);
        let buf = [0x01, 0x02, 0x03];
        cursor.write(&buf).unwrap();
        assert_eq!(cursor.into_inner().as_slice(), [0x01, 0x02, 0x03]);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_vec_start_0_cap() {
        let out_buf = alloc::vec::Vec::new();
        let mut cursor = Cursor::new(out_buf);
        let buf = [0x01, 0x02, 0x03];
        cursor.write(&buf).unwrap();
        assert_eq!(cursor.into_inner().as_slice(), [0x01, 0x02, 0x03]);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_vec_reverse_and_write() {
        let mut buf = alloc::vec![0x01u8, 0x02, 0x03];
        let mut cursor = Cursor::new(&mut buf);
        cursor.set_position(0x02);
        cursor.write(&[0x03, 0x04]).unwrap();
        cursor.set_position(0);
        cursor.write(&[0x00, 0x00]).unwrap();
        drop(cursor);
        assert_eq!(buf, alloc::vec![0x00, 0x0, 0x03, 0x04]);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_vec_write_past_end() {
        let mut buf = alloc::vec![0x01u8, 0x02, 0x03];
        let mut cursor = Cursor::new(&mut buf);
        cursor.set_position(0x04);
        cursor.write(&[0x05, 0x06]).unwrap();
        drop(cursor);
        assert_eq!(buf, alloc::vec![0x01, 0x02, 0x03, 0x00, 0x05, 0x06]);
    }
}
