use std::io::Write;

use crate::{
    internal::ppmd7::{PPMd7, RangeEncoder},
    Error, PPMD7_MAX_MEM_SIZE, PPMD7_MAX_ORDER, PPMD7_MIN_MEM_SIZE, PPMD7_MIN_ORDER, SYM_END,
};

/// An encoder to compress data using PPMd7 (PPMdH) with the 7z range coder.
pub struct Ppmd7Encoder<W: Write> {
    ppmd: PPMd7<RangeEncoder<W>>,
}

unsafe impl<W: Write> Send for Ppmd7Encoder<W> {}
unsafe impl<W: Write> Sync for Ppmd7Encoder<W> {}

impl<W: Write> Ppmd7Encoder<W> {
    /// Creates a new [`Ppmd7Encoder`] which provides a writer over the compressed data.
    ///
    /// The given `order` must be between [`PPMD7_MIN_ORDER`] and [`PPMD7_MAX_ORDER`] (inclusive).
    /// The given `mem_size` must be between [`PPMD7_MIN_MEM_SIZE`] and [`PPMD7_MAX_MEM_SIZE`] (inclusive).
    pub fn new(writer: W, order: u32, mem_size: u32) -> crate::Result<Self> {
        if !(PPMD7_MIN_ORDER..=PPMD7_MAX_ORDER).contains(&order)
            || !(PPMD7_MIN_MEM_SIZE..=PPMD7_MAX_MEM_SIZE).contains(&mem_size)
        {
            return Err(Error::InvalidParameter);
        }

        let ppmd = PPMd7::new_encoder(writer, order, mem_size)?;

        Ok(Self { ppmd })
    }

    /// Gets a reference to the underlying writer.
    pub fn get_ref(&self) -> &W {
        self.ppmd.get_ref()
    }

    /// Gets a mutable reference to the underlying writer.
    ///
    /// Note that mutating the output/input state of the stream may corrupt
    /// this object, so care must be taken when using this method.
    pub fn get_mut(&mut self) -> &mut W {
        self.ppmd.get_mut()
    }

    /// Returns the inner writer.
    pub fn into_inner(self) -> W {
        self.ppmd.into_inner()
    }

    /// Finishes the encoding process.
    ///
    /// Adds an end marker to the data if `with_end_marker` is set to `true`.
    pub fn finish(mut self, with_end_marker: bool) -> std::io::Result<W> {
        if with_end_marker {
            self.ppmd.encode_symbol(SYM_END)?;
        }
        self.flush()?;
        Ok(self.into_inner())
    }
}

impl<W: Write> Write for Ppmd7Encoder<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        for &byte in buf.iter() {
            self.ppmd.encode_symbol(byte as i32)?;
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.ppmd.flush_range_encoder()
    }
}

#[cfg(test)]
mod test {
    use std::io::{Read, Write};

    use super::{super::decoder_7::Ppmd7Decoder, Ppmd7Encoder};

    const ORDER: u32 = 8;
    const MEM_SIZE: u32 = 262144;

    #[test]
    fn ppmd7encoder_without_end_marker() {
        let test_data = include_str!("../tests/fixtures/text/apache2.txt");

        let mut data = Vec::new();
        {
            let mut encoder = Ppmd7Encoder::new(&mut data, ORDER, MEM_SIZE).unwrap();
            encoder.write_all(test_data.as_bytes()).unwrap();
            encoder.finish(false).unwrap();
        }

        let mut decoder = Ppmd7Decoder::new(data.as_slice(), ORDER, MEM_SIZE).unwrap();

        let mut decoded = vec![0; test_data.len()];
        decoder.read_exact(&mut decoded).unwrap();

        let decoded_data = String::from_utf8(decoded).unwrap();
        assert_eq!(decoded_data, test_data);
    }

    #[test]
    fn ppmd7encoder_with_end_marker() {
        let test_data = include_str!("../tests/fixtures/text/apache2.txt");

        let mut data = Vec::new();
        {
            let mut encoder = Ppmd7Encoder::new(&mut data, ORDER, MEM_SIZE).unwrap();
            encoder.write_all(test_data.as_bytes()).unwrap();
            encoder.finish(true).unwrap();
        }

        let mut decoder = Ppmd7Decoder::new(data.as_slice(), ORDER, MEM_SIZE).unwrap();

        let mut decoded = Vec::new();
        decoder.read_to_end(&mut decoded).unwrap();

        let decoded_data = String::from_utf8(decoded).unwrap();
        assert_eq!(decoded_data, test_data);
    }
}
