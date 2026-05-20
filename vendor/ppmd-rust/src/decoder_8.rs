use std::io::Read;

use crate::{
    internal::ppmd8::{PPMd8, RangeDecoder},
    Error, RestoreMethod, PPMD8_MAX_MEM_SIZE, PPMD8_MAX_ORDER, PPMD8_MIN_MEM_SIZE, PPMD8_MIN_ORDER,
    SYM_END,
};

/// A decoder to decompress data using PPMd8 (PPMdI rev.1).
pub struct Ppmd8Decoder<R: Read> {
    ppmd: PPMd8<RangeDecoder<R>>,
    finished: bool,
}

unsafe impl<R: Read> Send for Ppmd8Decoder<R> {}
unsafe impl<R: Read> Sync for Ppmd8Decoder<R> {}

impl<R: Read> Ppmd8Decoder<R> {
    /// Creates a new [`Ppmd8Decoder`] which provides a reader over the uncompressed data.
    ///
    /// The given `order` must be between [`PPMD8_MIN_ORDER`] and [`PPMD8_MAX_ORDER`] (inclusive).
    /// The given `mem_size` must be between [`PPMD8_MIN_MEM_SIZE`] and [`PPMD8_MAX_MEM_SIZE`] (inclusive).
    pub fn new(
        reader: R,
        order: u32,
        mem_size: u32,
        restore_method: RestoreMethod,
    ) -> crate::Result<Self> {
        if !(PPMD8_MIN_ORDER..=PPMD8_MAX_ORDER).contains(&order)
            || !(PPMD8_MIN_MEM_SIZE..=PPMD8_MAX_MEM_SIZE).contains(&mem_size)
        {
            return Err(Error::InvalidParameter);
        }

        let ppmd = PPMd8::new_decoder(reader, order, mem_size, restore_method)?;

        Ok(Self {
            ppmd,
            finished: false,
        })
    }

    /// Gets a reference to the underlying reader.
    pub fn get_ref(&self) -> &R {
        self.ppmd.get_ref()
    }

    /// Gets a mutable reference to the underlying reader.
    ///
    /// Note that mutation of the stream may result in surprising results if
    /// this decoder is continued to be used.
    pub fn get_mut(&mut self) -> &mut R {
        self.ppmd.get_mut()
    }

    /// Returns the inner reader.
    pub fn into_inner(self) -> R {
        self.ppmd.into_inner()
    }
}

impl<R: Read> Read for Ppmd8Decoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.finished {
            return Ok(0);
        }

        if buf.is_empty() {
            return Ok(0);
        }

        let mut sym = 0;
        let mut decoded = 0;

        for byte in buf.iter_mut() {
            match self.ppmd.decode_symbol() {
                Ok(symbol) => sym = symbol,
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::UnexpectedEof {
                        self.finished = true;
                        return Ok(decoded);
                    }
                    return Err(err);
                }
            }

            if sym < 0 {
                break;
            }

            *byte = sym as u8;
            decoded += 1;
        }

        let code = self.ppmd.range_decoder_code();

        if sym >= 0 {
            return Ok(decoded);
        }

        self.finished = true;

        if sym != SYM_END || code != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Error during PPMd decoding",
            ));
        }

        // END_MARKER detected
        Ok(decoded)
    }
}
