use std::io::{Read, Write};

use super::{super::PPMD_BIN_SCALE, K_BOT_VALUE, K_TOP_VALUE};
use crate::Error;

#[derive(Copy, Clone)]
#[repr(C)]
pub(crate) struct RangeDecoder<R: Read> {
    pub(crate) range: u32,
    pub(crate) code: u32,
    pub(crate) low: u32,
    pub(crate) reader: R,
}

impl<R: Read> RangeDecoder<R> {
    pub(crate) fn new(reader: R) -> crate::Result<Self> {
        let mut encoder = Self {
            range: 0xFFFFFFFF,
            code: 0,
            low: 0,
            reader,
        };

        for _ in 0..4 {
            encoder.code = encoder.code << 8 | encoder.read_byte().map_err(Error::IoError)?;
        }

        if encoder.code == 0xFFFFFFFF {
            return Err(Error::RangeDecoderInitialization);
        }

        Ok(encoder)
    }

    #[inline(always)]
    pub(crate) fn correct_sum_range(&self, sum: u32) -> u32 {
        correct_sum_range(self.range, sum)
    }

    #[inline(always)]
    pub(crate) fn get_threshold(&mut self, sum: u32) -> u32 {
        self.range /= sum;
        self.code / self.range
    }

    #[inline(always)]
    pub(crate) fn read_byte(&mut self) -> Result<u32, std::io::Error> {
        let mut buffer = [0];
        self.reader.read_exact(&mut buffer)?;
        Ok(buffer[0] as u32)
    }

    #[inline(always)]
    pub(crate) fn decode_bit_1(&mut self, size: u32) {
        self.low += size;
        self.code -= size;
        self.range = (self.range & !(PPMD_BIN_SCALE - 1)) - size;
    }

    #[inline(always)]
    pub(crate) fn decode(&mut self, mut start: u32, size: u32) {
        start *= self.range;
        self.low += start;
        self.code -= start;
        self.range *= size;
    }

    #[inline(always)]
    pub(crate) fn decode_final(&mut self, start: u32, size: u32) -> Result<(), std::io::Error> {
        self.decode(start, size);
        self.normalize_remote()
    }

    #[inline(always)]
    pub(crate) fn normalize_remote(&mut self) -> Result<(), std::io::Error> {
        while self.low ^ self.low.wrapping_add(self.range) < K_TOP_VALUE
            || self.range < K_BOT_VALUE && {
                self.range = 0u32.wrapping_sub(self.low) & (K_BOT_VALUE - 1);
                1 != 0
            }
        {
            self.code = self.code << 8 | self.read_byte()?;
            self.range <<= 8;
            self.low <<= 8;
        }

        Ok(())
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub(crate) struct RangeEncoder<W: Write> {
    pub(crate) range: u32,
    pub(crate) low: u32,
    pub(crate) writer: W,
}

impl<W: Write> RangeEncoder<W> {
    pub(crate) fn new(writer: W) -> Self {
        Self {
            range: 0xFFFFFFFF,
            low: 0,
            writer,
        }
    }

    #[inline(always)]
    pub(crate) fn correct_sum_range(&self, sum: u32) -> u32 {
        correct_sum_range(self.range, sum)
    }

    #[inline(always)]
    pub(crate) fn write_byte(&mut self, byte: u8) -> Result<(), std::io::Error> {
        self.writer.write_all(&[byte])
    }

    #[inline(always)]
    pub(crate) fn encode_bit_1(&mut self, bound: u32) {
        self.low += bound;
        self.range = (self.range & !(PPMD_BIN_SCALE - 1)) - bound;
    }

    #[inline(always)]
    pub(crate) fn encode(&mut self, start: u32, size: u32, total: u32) {
        self.range /= total;
        self.low += start * self.range;
        self.range *= size;
    }

    #[inline(always)]
    pub(crate) fn encode_final(
        &mut self,
        start: u32,
        size: u32,
        total: u32,
    ) -> Result<(), std::io::Error> {
        self.encode(start, size, total);
        self.normalize_remote()
    }

    #[inline(always)]
    pub(crate) fn normalize_remote(&mut self) -> Result<(), std::io::Error> {
        while self.low ^ self.low.wrapping_add(self.range) < K_TOP_VALUE
            || self.range < K_BOT_VALUE && {
                self.range = 0u32.wrapping_sub(self.low) & (K_BOT_VALUE - 1);
                1 != 0
            }
        {
            self.write_byte((self.low >> 24) as u8)?;
            self.range <<= 8;
            self.low <<= 8;
        }

        Ok(())
    }

    pub(crate) fn flush(&mut self) -> Result<(), std::io::Error> {
        for _ in 0..4 {
            let byte = (self.low >> 24) as u8;
            self.writer.write_all(&[byte])?;
            self.low <<= 8;
        }
        self.writer.flush()?;
        Ok(())
    }
}

// The original PPMdI encoder and decoder probably could work incorrectly in some rare cases,
// where the original PPMdI code can give "Divide by Zero" operation.
// We use the following fix to allow correct working of encoder and decoder in any cases.
// We correct (escape_freq) and (sum), if (sum) is larger than (range).
#[inline(always)]
fn correct_sum_range(range: u32, sum: u32) -> u32 {
    if sum > range {
        range
    } else {
        sum
    }
}
