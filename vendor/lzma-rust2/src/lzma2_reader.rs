use super::{
    Read,
    decoder::LzmaDecoder,
    error_invalid_input,
    lz::LzDecoder,
    range_dec::{RangeDecoder, RangeDecoderBuffer},
};
use crate::ByteReader;

pub const COMPRESSED_SIZE_MAX: u32 = 1 << 16;

/// A single-threaded LZMA2 decompressor.
///
/// # Examples
/// ```
/// use std::io::Read;
///
/// use lzma_rust2::{Lzma2Reader, LzmaOptions};
///
/// let compressed: Vec<u8> = vec![
///     1, 0, 12, 72, 101, 108, 108, 111, 44, 32, 119, 111, 114, 108, 100, 33, 0,
/// ];
/// let mut reader = Lzma2Reader::new(compressed.as_slice(), LzmaOptions::DICT_SIZE_DEFAULT, None);
/// let mut decompressed = Vec::new();
/// reader.read_to_end(&mut decompressed).unwrap();
/// assert_eq!(&decompressed[..], b"Hello, world!");
/// ```
pub struct Lzma2Reader<R> {
    inner: R,
    lz: LzDecoder,
    rc: RangeDecoder<RangeDecoderBuffer>,
    lzma: Option<LzmaDecoder>,
    uncompressed_size: usize,
    is_lzma_chunk: bool,
    need_dict_reset: bool,
    need_props: bool,
    end_reached: bool,
}

/// Calculates the memory usage in KiB required for LZMA2 decompression.
#[inline]
pub fn get_memory_usage(dict_size: u32) -> u32 {
    40 + COMPRESSED_SIZE_MAX / 1024 + get_dict_size(dict_size) / 1024
}

#[inline]
fn get_dict_size(dict_size: u32) -> u32 {
    if dict_size >= (u32::MAX - 15) {
        return u32::MAX;
    }

    (dict_size + 15) & !15
}

impl<R> Lzma2Reader<R> {
    /// Unwraps the reader, returning the underlying reader.
    pub fn into_inner(self) -> R {
        self.inner
    }

    /// Returns a reference to the inner reader.
    pub fn inner(&self) -> &R {
        &self.inner
    }

    /// Returns a mutable reference to the inner reader.
    pub fn inner_mut(&mut self) -> &mut R {
        &mut self.inner
    }
}

impl<R: Read> Lzma2Reader<R> {
    /// Create a new LZMA2 reader.
    /// `inner` is the reader to read compressed data from.
    /// `dict_size` is the dictionary size in bytes.
    pub fn new(inner: R, dict_size: u32, preset_dict: Option<&[u8]>) -> Self {
        let has_preset = preset_dict.as_ref().map(|a| !a.is_empty()).unwrap_or(false);
        let lz = LzDecoder::new(get_dict_size(dict_size) as _, preset_dict);
        let rc = RangeDecoder::new_buffer(COMPRESSED_SIZE_MAX as _);
        Self {
            inner,
            lz,
            rc,
            lzma: None,
            uncompressed_size: 0,
            is_lzma_chunk: false,
            need_dict_reset: !has_preset,
            need_props: true,
            end_reached: false,
        }
    }

    // ### LZMA2 Control Byte Meaning
    //
    //  Control Byte    | Chunk Type      | Formal Action
    //  --------------- | --------------- | ----------------------------
    //  0x00            | End of Stream   | Terminates the LZMA2 stream.
    //  0x01            | Uncompressed    | Resets Dictionary.
    //  0x02            | Uncompressed    | Preserves Dictionary.
    //  0x03 – 0x7F     | Reserved        | Invalid stream.
    //  0x80 – 0xFF     | LZMA Compressed | Varies based on bits 6 and 5
    //
    // ### Detailed Breakdown of LZMA Compressed Chunks (0x80 - 0xFF)
    //
    //  Bits | Control Byte | Reset Action            | Suitable for Parallel Start? |
    //  ---- | ------------ | ----------------------- | ---------------------------- |
    //  00   | 0x80 – 0x9F  | None                    | No
    //  01   | 0xA0 – 0xBF  | Reset State             | No
    //  10   | 0xC0 – 0xDF  | Reset State & Props     | No
    //  11   | 0xE0 – 0xFF  | Reset Everything        | Yes
    fn decode_chunk_header(&mut self) -> crate::Result<()> {
        let control = self.inner.read_u8()?;

        if control == 0x00 {
            self.end_reached = true;
            return Ok(());
        }

        if control >= 0xE0 || control == 0x01 {
            self.need_props = true;
            self.need_dict_reset = false;
            // Reset dictionary
            self.lz.reset();
        } else if self.need_dict_reset {
            return Err(error_invalid_input("corrupted input data (LZMA2:0)"));
        }
        if control >= 0x80 {
            self.is_lzma_chunk = true;
            self.uncompressed_size = ((control & 0x1F) as usize) << 16;
            self.uncompressed_size += self.inner.read_u16_be()? as usize + 1;
            let compressed_size = self.inner.read_u16_be()? as usize + 1;

            if control >= 0xC0 {
                // Reset props and state (by re-creating it)
                self.need_props = false;
                self.decode_props()?;
            } else if self.need_props {
                return Err(error_invalid_input("corrupted input data (LZMA2:1)"));
            } else if control >= 0xA0 {
                // Reset state
                if let Some(l) = self.lzma.as_mut() {
                    l.reset()
                }
            }

            self.rc.prepare(&mut self.inner, compressed_size)?;
        } else if control > 0x02 {
            return Err(error_invalid_input("corrupted input data (LZMA2:2)"));
        } else {
            self.is_lzma_chunk = false;
            self.uncompressed_size = (self.inner.read_u16_be()? as usize) + 1;
        }
        Ok(())
    }

    /// Reads the next props and re-creates the state by creating a new decoder.
    fn decode_props(&mut self) -> crate::Result<()> {
        let props = self.inner.read_u8()?;
        if props > (4 * 5 + 4) * 9 + 8 {
            return Err(error_invalid_input("corrupted input data (LZMA2:3)"));
        }
        let pb = props / (9 * 5);
        let props = props - pb * 9 * 5;
        let lp = props / 9;
        let lc = props - lp * 9;
        if lc + lp > 4 {
            return Err(error_invalid_input("corrupted input data (LZMA2:4)"));
        }
        self.lzma = Some(LzmaDecoder::new(lc as _, lp as _, pb as _));

        Ok(())
    }
}

impl<R: Read> Read for Lzma2Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> crate::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        if self.end_reached {
            return Ok(0);
        }
        let mut size = 0;
        let mut len = buf.len();
        let mut off = 0;
        while len > 0 {
            if self.uncompressed_size == 0 {
                self.decode_chunk_header()?;
                if self.end_reached {
                    return Ok(size);
                }
            }

            let copy_size_max = self.uncompressed_size.min(len);
            if !self.is_lzma_chunk {
                self.lz.copy_uncompressed(&mut self.inner, copy_size_max)?;
            } else {
                self.lz.set_limit(copy_size_max);
                if let Some(lzma) = self.lzma.as_mut() {
                    lzma.decode(&mut self.lz, &mut self.rc)?;
                }
            }

            {
                let copied_size = self.lz.flush(buf, off)?;
                off = off.saturating_add(copied_size);
                len = len.saturating_sub(copied_size);
                size = size.saturating_add(copied_size);
                self.uncompressed_size = self.uncompressed_size.saturating_sub(copied_size);
                if self.uncompressed_size == 0 && (!self.rc.is_finished() || self.lz.has_pending())
                {
                    return Err(error_invalid_input("rc not finished or lz has pending"));
                }
            }
        }

        Ok(size)
    }
}
