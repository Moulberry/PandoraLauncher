use std::{
    io,
    io::{Read, Write},
    marker::PhantomData,
};

use byteorder::{ByteOrder, ReadBytesExt, WriteBytesExt};

use super::{ReadAt, WriteAt};

/// Extends `ReadAt` with methods for reading numbers at offsets.
///
/// For most of these methods, you need to explicitly add a `ByteOrder`
/// type parameter. Similar to [`byteorder::ReadBytesExt`][byteorder].
///
/// # Examples
///
/// Read an integer from the middle of a byte array:
///
/// ```rust
/// # extern crate byteorder;
/// # use std::io;
/// use byteorder::BigEndian;
/// use positioned_io::ReadBytesAtExt;
///
/// # fn try_main() -> io::Result<()> {
/// let buf = [0, 5, 254, 212, 0, 3];
/// let n = buf.as_ref().read_i16_at::<BigEndian>(2)?;
/// assert_eq!(n, -300);
/// # Ok(())
/// # }
/// # fn main() { try_main().unwrap() }
/// ```
///
/// [byteorder]: https://docs.rs/byteorder/1.2/byteorder/trait.ReadBytesExt.html
pub trait ReadBytesAtExt: ReadAt {
    /// Reads an unsigned 8-bit integer at an offset.
    #[inline]
    fn read_u8_at(&self, pos: u64) -> io::Result<u8> {
        let mut buf = [0; 1];
        self.read_exact_at(pos, &mut buf)?;
        Ok(buf[0])
    }

    /// Reads a signed 8-bit integer at an offset.
    #[inline]
    fn read_i8_at(&self, pos: u64) -> io::Result<i8> {
        let mut buf = [0; 1];
        self.read_exact_at(pos, &mut buf)?;
        Ok(buf[0] as i8)
    }

    /// Reads an unsigned 16-bit integer at an offset.
    #[inline]
    fn read_u16_at<T: ByteOrder>(&self, pos: u64) -> io::Result<u16> {
        let mut buf = [0; 2];
        self.read_exact_at(pos, &mut buf)?;
        Ok(T::read_u16(&buf))
    }

    /// Reads a signed 16-bit integer at an offset.
    #[inline]
    fn read_i16_at<T: ByteOrder>(&self, pos: u64) -> io::Result<i16> {
        let mut buf = [0; 2];
        self.read_exact_at(pos, &mut buf)?;
        Ok(T::read_i16(&buf))
    }

    /// Reads an unsigned 32-bit integer at an offset.
    #[inline]
    fn read_u32_at<T: ByteOrder>(&self, pos: u64) -> io::Result<u32> {
        let mut buf = [0; 4];
        self.read_exact_at(pos, &mut buf)?;
        Ok(T::read_u32(&buf))
    }

    /// Reads a signed 32-bit integer at an offset.
    #[inline]
    fn read_i32_at<T: ByteOrder>(&self, pos: u64) -> io::Result<i32> {
        let mut buf = [0; 4];
        self.read_exact_at(pos, &mut buf)?;
        Ok(T::read_i32(&buf))
    }

    /// Reads an unsigned 64-bit integer at an offset.
    #[inline]
    fn read_u64_at<T: ByteOrder>(&self, pos: u64) -> io::Result<u64> {
        let mut buf = [0; 8];
        self.read_exact_at(pos, &mut buf)?;
        Ok(T::read_u64(&buf))
    }

    /// Reads a signed 64-bit integer at an offset.
    #[inline]
    fn read_i64_at<T: ByteOrder>(&self, pos: u64) -> io::Result<i64> {
        let mut buf = [0; 8];
        self.read_exact_at(pos, &mut buf)?;
        Ok(T::read_i64(&buf))
    }

    /// Reads an unsigned `nbytes`-bit integer at an offset.
    #[inline]
    fn read_uint_at<T: ByteOrder>(&self, pos: u64, nbytes: usize) -> io::Result<u64> {
        let mut buf = [0; 8];
        self.read_exact_at(pos, &mut buf[..nbytes])?;
        Ok(T::read_uint(&buf[..nbytes], nbytes))
    }

    /// Reads a signed `nbytes`-bit integer at an offset.
    #[inline]
    fn read_int_at<T: ByteOrder>(&self, pos: u64, nbytes: usize) -> io::Result<i64> {
        let mut buf = [0; 8];
        self.read_exact_at(pos, &mut buf[..nbytes])?;
        Ok(T::read_int(&buf[..nbytes], nbytes))
    }

    /// Reads a single-precision floating point number at an offset.
    #[inline]
    fn read_f32_at<T: ByteOrder>(&self, pos: u64) -> io::Result<f32> {
        let mut buf = [0; 4];
        self.read_exact_at(pos, &mut buf)?;
        Ok(T::read_f32(&buf))
    }

    /// Reads a double-precision floating point number at an offset.
    #[inline]
    fn read_f64_at<T: ByteOrder>(&self, pos: u64) -> io::Result<f64> {
        let mut buf = [0; 8];
        self.read_exact_at(pos, &mut buf)?;
        Ok(T::read_f64(&buf))
    }
}

/// Extends `WriteAt` with methods for writing numbers at offsets.
///
/// For most of these methods, you need to explicitly add a `ByteOrder` type
/// parameter. Similar to [`byteorder::WriteBytesExt`][byteorder].
///
/// # Examples
///
/// Write an integer to the middle of a byte array:
///
/// ```rust
/// # extern crate byteorder;
/// # use std::io;
/// use byteorder::BigEndian;
/// use positioned_io::WriteBytesAtExt;
///
/// # fn try_main() -> io::Result<()> {
/// let mut buf = [0; 6];
/// buf.as_mut().write_u16_at::<BigEndian>(2, 300)?;
/// assert_eq!(buf, [0, 0, 1, 44, 0, 0]);
/// # Ok(())
/// # }
/// # fn main() { try_main().unwrap() }
/// ```
///
/// [byteorder]: https://docs.rs/byteorder/1.2/byteorder/trait.WriteBytesExt.html
pub trait WriteBytesAtExt: WriteAt {
    /// Writes an unsigned 8-bit integer to an offset.
    #[inline]
    fn write_u8_at(&mut self, pos: u64, n: u8) -> io::Result<()> {
        self.write_all_at(pos, &[n])
    }

    /// Writes a signed 8-bit integer to an offset.
    #[inline]
    fn write_i8_at(&mut self, pos: u64, n: i8) -> io::Result<()> {
        self.write_all_at(pos, &[n as u8])
    }

    /// Writes an unsigned 16-bit integer to an offset.
    #[inline]
    fn write_u16_at<T: ByteOrder>(&mut self, pos: u64, n: u16) -> io::Result<()> {
        let mut buf = [0; 2];
        T::write_u16(&mut buf, n);
        self.write_all_at(pos, &buf)
    }

    /// Writes a signed 16-bit integer to an offset.
    #[inline]
    fn write_i16_at<T: ByteOrder>(&mut self, pos: u64, n: i16) -> io::Result<()> {
        let mut buf = [0; 2];
        T::write_i16(&mut buf, n);
        self.write_all_at(pos, &buf)
    }

    /// Writes an unsigned 32-bit integer to an offset.
    #[inline]
    fn write_u32_at<T: ByteOrder>(&mut self, pos: u64, n: u32) -> io::Result<()> {
        let mut buf = [0; 4];
        T::write_u32(&mut buf, n);
        self.write_all_at(pos, &buf)
    }

    /// Writes a signed 32-bit integer to an offset.
    #[inline]
    fn write_i32_at<T: ByteOrder>(&mut self, pos: u64, n: i32) -> io::Result<()> {
        let mut buf = [0; 4];
        T::write_i32(&mut buf, n);
        self.write_all_at(pos, &buf)
    }

    /// Writes an unsigned 64-bit integer to an offset.
    #[inline]
    fn write_u64_at<T: ByteOrder>(&mut self, pos: u64, n: u64) -> io::Result<()> {
        let mut buf = [0; 8];
        T::write_u64(&mut buf, n);
        self.write_all_at(pos, &buf)
    }

    /// Writes a signed 64-bit integer to an offset.
    #[inline]
    fn write_i64_at<T: ByteOrder>(&mut self, pos: u64, n: i64) -> io::Result<()> {
        let mut buf = [0; 8];
        T::write_i64(&mut buf, n);
        self.write_all_at(pos, &buf)
    }

    /// Writes an unsigned `nbytes`-bit integer to an offset.
    #[inline]
    fn write_uint_at<T: ByteOrder>(&mut self, pos: u64, n: u64, nbytes: usize) -> io::Result<()> {
        let mut buf = [0; 8];
        T::write_uint(&mut buf, n, nbytes);
        self.write_all_at(pos, &buf[..nbytes])
    }

    /// Writes a signed `nbytes`-bit integer to an offset.
    #[inline]
    fn write_int_at<T: ByteOrder>(&mut self, pos: u64, n: i64, nbytes: usize) -> io::Result<()> {
        let mut buf = [0; 8];
        T::write_int(&mut buf, n, nbytes);
        self.write_all_at(pos, &buf[..nbytes])
    }

    /// Writes a single-precision floating point number to an offset.
    #[inline]
    fn write_f32_at<T: ByteOrder>(&mut self, pos: u64, n: f32) -> io::Result<()> {
        let mut buf = [0; 4];
        T::write_f32(&mut buf, n);
        self.write_all_at(pos, &buf)
    }

    /// Writes a double-precision floating point number to an offset.
    #[inline]
    fn write_f64_at<T: ByteOrder>(&mut self, pos: u64, n: f64) -> io::Result<()> {
        let mut buf = [0; 8];
        T::write_f64(&mut buf, n);
        self.write_all_at(pos, &buf)
    }
}

// Implement for everything that does positioned I/O.
impl<R: ReadAt> ReadBytesAtExt for R {}
impl<W: WriteAt> WriteBytesAtExt for W {}

/// Read or write with a given inherent byte-order.
///
/// If you know that you'll always be using a single endianness, an instance of
/// `ByteIo` will allow you to omit the endian specifier on every read or write.
///
/// # Examples
///
/// ```rust
/// # extern crate byteorder;
/// #
/// # use std::io;
/// #
/// # fn try_main() -> io::Result<()> {
/// use byteorder::BigEndian;
/// use positioned_io::ByteIo;
///
/// let mut buf = [0; 8];
///
/// {
///     let mut io : ByteIo<_, BigEndian> = ByteIo::new(&mut buf[..]);
///     // All writes will automatically be BigEndian.
///     io.write_u16(300)?;
///     io.write_u32(1_000_000)?;
///     io.write_i16(-1)?;
/// }
///
/// assert_eq!(buf, [1, 44, 0, 15, 66, 64, 255, 255]);
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap()
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ByteIo<I, E: ByteOrder> {
    io: I,
    endianness: PhantomData<E>,
}

impl<I, E: ByteOrder> ByteIo<I, E> {
    /// Create a new `ByteIo` from some sort of reader or writer.
    ///
    /// You will need to specify the byte-order when creating a `ByteIo`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # extern crate byteorder;
    /// use byteorder::BigEndian;
    /// use positioned_io::ByteIo;
    ///
    /// # fn main() {
    /// let buf = [0; 10];
    /// // Add a type specifier for the byte order.
    /// let io : ByteIo<_, BigEndian> = ByteIo::new(&buf[..]);
    /// # }
    /// ```
    #[inline]
    pub fn new(io: I) -> Self {
        ByteIo {
            io,
            endianness: PhantomData,
        }
    }

    /// Returns the underlying reader or writer.
    #[inline]
    pub fn into_inner(self) -> I {
        self.io
    }

    /// Borrows the underlying reader or writer.
    #[inline]
    pub fn get_ref(&self) -> &I {
        &self.io
    }

    /// Mutably borrows the underlying reader or writer.
    #[inline]
    pub fn get_mut(&mut self) -> &mut I {
        &mut self.io
    }
}

impl<I: Read, E: ByteOrder> Read for ByteIo<I, E> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.io.read(buf)
    }
}

impl<I: Write, E: ByteOrder> Write for ByteIo<I, E> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.io.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.io.flush()
    }
}

impl<I: ReadAt, E: ByteOrder> ReadAt for ByteIo<I, E> {
    #[inline]
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
        self.io.read_at(pos, buf)
    }
}

impl<I: WriteAt, E: ByteOrder> WriteAt for ByteIo<I, E> {
    #[inline]
    fn write_at(&mut self, pos: u64, buf: &[u8]) -> io::Result<usize> {
        self.io.write_at(pos, buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.io.flush()
    }
}

impl<I: Read, E: ByteOrder> ByteIo<I, E> {
    #[inline]
    pub fn read_u8(&mut self) -> io::Result<u8> {
        self.io.read_u8()
    }

    #[inline]
    pub fn read_i8(&mut self) -> io::Result<i8> {
        self.io.read_i8()
    }

    #[inline]
    pub fn read_u16(&mut self) -> io::Result<u16> {
        self.io.read_u16::<E>()
    }

    #[inline]
    pub fn read_i16(&mut self) -> io::Result<i16> {
        self.io.read_i16::<E>()
    }

    #[inline]
    pub fn read_u32(&mut self) -> io::Result<u32> {
        self.io.read_u32::<E>()
    }

    #[inline]
    pub fn read_i32(&mut self) -> io::Result<i32> {
        self.io.read_i32::<E>()
    }

    #[inline]
    pub fn read_u64(&mut self) -> io::Result<u64> {
        self.io.read_u64::<E>()
    }

    #[inline]
    pub fn read_i64(&mut self) -> io::Result<i64> {
        self.io.read_i64::<E>()
    }

    #[inline]
    pub fn read_uint(&mut self, nbytes: usize) -> io::Result<u64> {
        self.io.read_uint::<E>(nbytes)
    }

    #[inline]
    pub fn read_int(&mut self, nbytes: usize) -> io::Result<i64> {
        self.io.read_int::<E>(nbytes)
    }

    #[inline]
    pub fn read_f32(&mut self) -> io::Result<f32> {
        self.io.read_f32::<E>()
    }

    #[inline]
    pub fn read_f64(&mut self) -> io::Result<f64> {
        self.io.read_f64::<E>()
    }
}

impl<I: Write, E: ByteOrder> ByteIo<I, E> {
    #[inline]
    pub fn write_u8(&mut self, n: u8) -> io::Result<()> {
        self.io.write_u8(n)
    }

    #[inline]
    pub fn write_i8(&mut self, n: i8) -> io::Result<()> {
        self.io.write_i8(n)
    }

    #[inline]
    pub fn write_u16(&mut self, n: u16) -> io::Result<()> {
        self.io.write_u16::<E>(n)
    }

    #[inline]
    pub fn write_i16(&mut self, n: i16) -> io::Result<()> {
        self.io.write_i16::<E>(n)
    }

    #[inline]
    pub fn write_u32(&mut self, n: u32) -> io::Result<()> {
        self.io.write_u32::<E>(n)
    }

    #[inline]
    pub fn write_i32(&mut self, n: i32) -> io::Result<()> {
        self.io.write_i32::<E>(n)
    }

    #[inline]
    pub fn write_u64(&mut self, n: u64) -> io::Result<()> {
        self.io.write_u64::<E>(n)
    }

    #[inline]
    pub fn write_i64(&mut self, n: i64) -> io::Result<()> {
        self.io.write_i64::<E>(n)
    }

    #[inline]
    pub fn write_uint(&mut self, n: u64, nbytes: usize) -> io::Result<()> {
        self.io.write_uint::<E>(n, nbytes)
    }

    #[inline]
    pub fn write_int(&mut self, n: i64, nbytes: usize) -> io::Result<()> {
        self.io.write_int::<E>(n, nbytes)
    }

    #[inline]
    pub fn write_f32(&mut self, n: f32) -> io::Result<()> {
        self.io.write_f32::<E>(n)
    }

    #[inline]
    pub fn write_f64(&mut self, n: f64) -> io::Result<()> {
        self.io.write_f64::<E>(n)
    }
}

impl<I: ReadAt, E: ByteOrder> ByteIo<I, E> {
    #[inline]
    pub fn read_u8_at(&self, pos: u64) -> io::Result<u8> {
        self.io.read_u8_at(pos)
    }

    #[inline]
    pub fn read_i8_at(&self, pos: u64) -> io::Result<i8> {
        self.io.read_i8_at(pos)
    }

    #[inline]
    pub fn read_u16_at(&self, pos: u64) -> io::Result<u16> {
        self.io.read_u16_at::<E>(pos)
    }

    #[inline]
    pub fn read_i16_at(&self, pos: u64) -> io::Result<i16> {
        self.io.read_i16_at::<E>(pos)
    }

    #[inline]
    pub fn read_u32_at(&self, pos: u64) -> io::Result<u32> {
        self.io.read_u32_at::<E>(pos)
    }

    #[inline]
    pub fn read_i32_at(&self, pos: u64) -> io::Result<i32> {
        self.io.read_i32_at::<E>(pos)
    }

    #[inline]
    pub fn read_u64_at(&self, pos: u64) -> io::Result<u64> {
        self.io.read_u64_at::<E>(pos)
    }

    #[inline]
    pub fn read_i64_at(&self, pos: u64) -> io::Result<i64> {
        self.io.read_i64_at::<E>(pos)
    }

    #[inline]
    pub fn read_uint_at(&self, pos: u64, nbytes: usize) -> io::Result<u64> {
        self.io.read_uint_at::<E>(pos, nbytes)
    }

    #[inline]
    pub fn read_int_at(&self, pos: u64, nbytes: usize) -> io::Result<i64> {
        self.io.read_int_at::<E>(pos, nbytes)
    }

    #[inline]
    pub fn read_f32_at(&self, pos: u64) -> io::Result<f32> {
        self.io.read_f32_at::<E>(pos)
    }

    #[inline]
    pub fn read_f64_at(&self, pos: u64) -> io::Result<f64> {
        self.io.read_f64_at::<E>(pos)
    }
}

impl<I: WriteAt, E: ByteOrder> ByteIo<I, E> {
    #[inline]
    pub fn write_u8_at(&mut self, pos: u64, n: u8) -> io::Result<()> {
        self.io.write_u8_at(pos, n)
    }

    #[inline]
    pub fn write_i8_at(&mut self, pos: u64, n: i8) -> io::Result<()> {
        self.io.write_i8_at(pos, n)
    }

    #[inline]
    pub fn write_u16_at(&mut self, pos: u64, n: u16) -> io::Result<()> {
        self.io.write_u16_at::<E>(pos, n)
    }

    #[inline]
    pub fn write_i16_at(&mut self, pos: u64, n: i16) -> io::Result<()> {
        self.io.write_i16_at::<E>(pos, n)
    }

    #[inline]
    pub fn write_u32_at(&mut self, pos: u64, n: u32) -> io::Result<()> {
        self.io.write_u32_at::<E>(pos, n)
    }

    #[inline]
    pub fn write_i32_at(&mut self, pos: u64, n: i32) -> io::Result<()> {
        self.io.write_i32_at::<E>(pos, n)
    }

    #[inline]
    pub fn write_u64_at(&mut self, pos: u64, n: u64) -> io::Result<()> {
        self.io.write_u64_at::<E>(pos, n)
    }

    #[inline]
    pub fn write_i64_at(&mut self, pos: u64, n: i64) -> io::Result<()> {
        self.io.write_i64_at::<E>(pos, n)
    }

    #[inline]
    pub fn write_uint_at(&mut self, pos: u64, n: u64, nbytes: usize) -> io::Result<()> {
        self.io.write_uint_at::<E>(pos, n, nbytes)
    }

    #[inline]
    pub fn write_int_at(&mut self, pos: u64, n: i64, nbytes: usize) -> io::Result<()> {
        self.io.write_int_at::<E>(pos, n, nbytes)
    }

    #[inline]
    pub fn write_f32_at(&mut self, pos: u64, n: f32) -> io::Result<()> {
        self.io.write_f32_at::<E>(pos, n)
    }

    #[inline]
    pub fn write_f64_at(&mut self, pos: u64, n: f64) -> io::Result<()> {
        self.io.write_f64_at::<E>(pos, n)
    }
}
