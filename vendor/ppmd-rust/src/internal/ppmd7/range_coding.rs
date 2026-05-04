use std::io::{Read, Write};

use super::K_TOP_VALUE;
use crate::Error;

pub(crate) struct RangeDecoder<R: Read> {
    pub(crate) range: u32,
    pub(crate) code: u32,
    pub(crate) reader: R,
}

impl<R: Read> RangeDecoder<R> {
    pub(crate) fn new(reader: R) -> crate::Result<Self> {
        let mut encoder = Self {
            range: 0xFFFFFFFF,
            code: 0,
            reader,
        };

        if encoder.read_byte().map_err(Error::IoError)? != 0 {
            return Err(Error::RangeDecoderInitialization);
        }

        for _ in 0..4 {
            encoder.code = encoder.code << 8 | encoder.read_byte().map_err(Error::IoError)?;
        }

        if encoder.code == 0xFFFFFFFF {
            return Err(Error::RangeDecoderInitialization);
        }

        Ok(encoder)
    }

    #[inline(always)]
    pub(crate) fn get_threshold(&mut self, total: u32) -> u32 {
        self.range /= total;
        self.code / self.range
    }

    #[inline(always)]
    pub(crate) fn read_byte(&mut self) -> Result<u32, std::io::Error> {
        let mut buffer = [0];
        self.reader.read_exact(&mut buffer)?;
        Ok(buffer[0] as u32)
    }

    #[inline(always)]
    pub(crate) fn decode_bit_0(&mut self, size: u32) -> Result<(), std::io::Error> {
        self.range = size;
        self.normalize_1()?;
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn decode_bit_1(&mut self, size: u32) {
        self.code -= size;
        self.range -= size;
    }

    #[inline(always)]
    pub(crate) fn decode(&mut self, start: u32, size: u32) {
        self.code -= start * self.range;
        self.range *= size;
    }

    #[inline(always)]
    pub(crate) fn decode_final(&mut self, start: u32, freq: u32) -> Result<(), std::io::Error> {
        self.decode(start, freq);
        self.normalize_remote()?;
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn normalize_remote(&mut self) -> Result<(), std::io::Error> {
        if self.range < 1 << 24 {
            self.code = self.code << 8 | self.read_byte()?;
            self.range <<= 8;
            if self.range < 1 << 24 {
                self.code = self.code << 8 | self.read_byte()?;
                self.range <<= 8;
            }
        }
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn normalize_1(&mut self) -> Result<(), std::io::Error> {
        if self.range < 1 << 24 {
            self.code = self.code << 8 | self.read_byte()?;
            self.range <<= 8;
        }
        Ok(())
    }
}

pub(crate) struct RangeEncoder<W: Write> {
    pub(crate) range: u32,
    pub(crate) cache: u8,
    pub(crate) low: u64,
    pub(crate) cache_size: u64,
    pub(crate) writer: W,
}

impl<W: Write> RangeEncoder<W> {
    pub(crate) fn new(writer: W) -> Self {
        Self {
            range: 0xFFFFFFFF,
            cache: 0,
            low: 0,
            cache_size: 1,
            writer,
        }
    }

    pub(crate) fn shift_low(&mut self) -> Result<(), std::io::Error> {
        if (self.low) < 0xFF000000 || (self.low >> 32) != 0 {
            let mut temp: u8 = self.cache;
            loop {
                let byte = (temp as u16 + (self.low >> 32) as u8 as u16) as u8;
                self.writer.write_all(&[byte])?;
                temp = 0xFF;
                self.cache_size -= 1;
                if self.cache_size == 0 {
                    break;
                }
            }
            self.cache = (self.low as u32 >> 24) as u8;
        }
        self.cache_size += 1;
        self.low = ((self.low as u32) << 8) as u64;
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn encode_bit_0(&mut self, bound: u32) -> Result<(), std::io::Error> {
        self.range = bound;
        self.normalize_1()?;
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn encode_bit_1(&mut self, bound: u32) -> Result<(), std::io::Error> {
        self.low += bound as u64;
        self.range -= bound;
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn encode(&mut self, start: u32, size: u32) {
        self.low += (start * self.range) as u64;
        self.range *= size;
    }

    #[inline(always)]
    pub(crate) fn encode_final(&mut self, start: u32, freq: u32) -> Result<(), std::io::Error> {
        self.encode(start, freq);
        self.normalize_remote()?;
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn normalize_remote(&mut self) -> Result<(), std::io::Error> {
        if self.range < K_TOP_VALUE {
            self.range <<= 8;
            self.shift_low()?;
            if self.range < K_TOP_VALUE {
                self.range <<= 8;
                self.shift_low()?;
            }
        }
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn normalize_1(&mut self) -> Result<(), std::io::Error> {
        if self.range < 1 << 24 {
            self.range <<= 8;
            self.shift_low()?;
        }
        Ok(())
    }

    pub(crate) fn flush(&mut self) -> Result<(), std::io::Error> {
        for _ in 0..5 {
            self.shift_low()?;
        }
        self.writer.flush()?;
        Ok(())
    }
}
