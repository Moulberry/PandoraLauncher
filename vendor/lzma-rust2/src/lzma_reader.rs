use super::{
    ByteReader, DICT_SIZE_MAX, Read, decoder::LzmaDecoder, error_invalid_data, error_invalid_input,
    error_out_of_memory, lz::LzDecoder, range_dec::RangeDecoder,
};

/// Calculates the memory usage in KiB required for LZMA decompression from properties byte.
pub fn get_memory_usage_by_props(dict_size: u32, props_byte: u8) -> crate::Result<u32> {
    if dict_size > DICT_SIZE_MAX {
        return Err(error_invalid_input("dict size too large"));
    }
    if props_byte > (4 * 5 + 4) * 9 + 8 {
        return Err(error_invalid_input("invalid props byte"));
    }
    let props = props_byte % (9 * 5);
    let lp = props / 9;
    let lc = props - lp * 9;
    get_memory_usage(dict_size, lc as u32, lp as u32)
}

/// Calculates the memory usage in KiB required for LZMA decompression.
pub fn get_memory_usage(dict_size: u32, lc: u32, lp: u32) -> crate::Result<u32> {
    if lc > 8 || lp > 4 {
        return Err(error_invalid_input("invalid lc or lp"));
    }
    Ok(10 + get_dict_size(dict_size)? / 1024 + ((2 * 0x300) << (lc + lp)) / 1024)
}

fn get_dict_size(dict_size: u32) -> crate::Result<u32> {
    if dict_size > DICT_SIZE_MAX {
        return Err(error_invalid_input("dict size too large"));
    }
    let dict_size = dict_size.max(4096);
    Ok((dict_size + 15) & !15)
}

/// A single-threaded LZMA decompressor.
///
/// # Examples
/// ```
/// use std::io::Read;
///
/// use lzma_rust2::LzmaReader;
///
/// let compressed: Vec<u8> = vec![
///     93, 0, 0, 128, 0, 255, 255, 255, 255, 255, 255, 255, 255, 0, 36, 25, 73, 152, 111, 22, 2,
///     140, 232, 230, 91, 177, 71, 198, 206, 183, 99, 255, 255, 60, 172, 0, 0,
/// ];
/// let mut reader = LzmaReader::new_mem_limit(compressed.as_slice(), u32::MAX, None).unwrap();
/// let mut buf = [0; 1024];
/// let mut out = Vec::new();
/// loop {
///     let n = reader.read(&mut buf).unwrap();
///     if n == 0 {
///         break;
///     }
///     out.extend_from_slice(&buf[..n]);
/// }
/// assert_eq!(out, b"Hello, world!");
/// ```
pub struct LzmaReader<R> {
    lz: LzDecoder,
    rc: RangeDecoder<R>,
    lzma: LzmaDecoder,
    end_reached: bool,
    relaxed_end_cond: bool,
    remaining_size: u64,
}

impl<R> LzmaReader<R> {
    /// Unwraps the reader, returning the underlying reader.
    pub fn into_inner(self) -> R {
        self.rc.into_inner()
    }

    /// Returns a reference to the inner reader.
    pub fn inner(&self) -> &R {
        self.rc.inner()
    }

    /// Returns a mutable reference to the inner reader.
    pub fn inner_mut(&mut self) -> &mut R {
        self.rc.inner_mut()
    }
}

impl<R: Read> LzmaReader<R> {
    fn construct1(
        reader: R,
        uncomp_size: u64,
        mut props: u8,
        dict_size: u32,
        preset_dict: Option<&[u8]>,
    ) -> crate::Result<Self> {
        if props > (4 * 5 + 4) * 9 + 8 {
            return Err(error_invalid_input("invalid props byte"));
        }
        let pb = props / (9 * 5);
        props -= pb * 9 * 5;
        let lp = props / 9;
        let lc = props - lp * 9;
        if dict_size > DICT_SIZE_MAX {
            return Err(error_invalid_input("dict size too large"));
        }
        Self::construct2(
            reader,
            uncomp_size,
            lc as _,
            lp as _,
            pb as _,
            dict_size,
            preset_dict,
        )
    }

    fn construct2(
        reader: R,
        uncomp_size: u64,
        lc: u32,
        lp: u32,
        pb: u32,
        dict_size: u32,
        preset_dict: Option<&[u8]>,
    ) -> crate::Result<Self> {
        if lc > 8 || lp > 4 || pb > 4 {
            return Err(error_invalid_input("invalid lc or lp or pb"));
        }
        let mut dict_size = get_dict_size(dict_size)?;
        if uncomp_size <= u64::MAX / 2 && dict_size as u64 > uncomp_size {
            dict_size = get_dict_size(uncomp_size as u32)?;
        }
        let rc = RangeDecoder::new_stream(reader);
        let rc = match rc {
            Ok(r) => r,
            Err(e) => {
                return Err(e);
            }
        };
        let lz = LzDecoder::new(get_dict_size(dict_size)? as _, preset_dict);
        let lzma = LzmaDecoder::new(lc, lp, pb);
        Ok(Self {
            // reader,
            lz,
            rc,
            lzma,
            end_reached: false,
            relaxed_end_cond: true,
            remaining_size: uncomp_size,
        })
    }

    /// Creates a new .lzma file format decompressor with an optional memory usage limit.
    /// - `mem_limit_kb` - memory usage limit in kibibytes (KiB). `u32::MAX` means no limit.
    /// - `preset_dict` - preset dictionary or None to use no preset dictionary.
    pub fn new_mem_limit(
        mut reader: R,
        mem_limit_kb: u32,
        preset_dict: Option<&[u8]>,
    ) -> crate::Result<Self> {
        let props = reader.read_u8()?;
        let dict_size = reader.read_u32()?;

        let uncomp_size = reader.read_u64()?;
        let need_mem = get_memory_usage_by_props(dict_size, props)?;
        if mem_limit_kb < need_mem {
            return Err(error_out_of_memory(
                "needed memory too big for mem_limit_kb",
            ));
        }
        Self::construct1(reader, uncomp_size, props, dict_size, preset_dict)
    }

    /// Creates a new input stream that decompresses raw LZMA data (no .lzma header) from `reader` optionally with a preset dictionary.
    /// - `reader` - the reader to read compressed data from.
    /// - `uncomp_size` - the uncompressed size of the data to be decompressed.
    /// - `props` - the LZMA properties byte.
    /// - `dict_size` - the LZMA dictionary size.
    /// - `preset_dict` - preset dictionary or None to use no preset dictionary.
    pub fn new_with_props(
        reader: R,
        uncomp_size: u64,
        props: u8,
        dict_size: u32,
        preset_dict: Option<&[u8]>,
    ) -> crate::Result<Self> {
        Self::construct1(reader, uncomp_size, props, dict_size, preset_dict)
    }

    /// Creates a new input stream that decompresses raw LZMA data (no .lzma header) from `reader` optionally with a preset dictionary.
    /// - `reader` - the input stream to read compressed data from.
    /// - `uncomp_size` - the uncompressed size of the data to be decompressed.
    /// - `lc` - the number of literal context bits.
    /// - `lp` - the number of literal position bits.
    /// - `pb` - the number of position bits.
    /// - `dict_size` - the LZMA dictionary size.
    /// - `preset_dict` - preset dictionary or None to use no preset dictionary.
    pub fn new(
        reader: R,
        uncomp_size: u64,
        lc: u32,
        lp: u32,
        pb: u32,
        dict_size: u32,
        preset_dict: Option<&[u8]>,
    ) -> crate::Result<Self> {
        Self::construct2(reader, uncomp_size, lc, lp, pb, dict_size, preset_dict)
    }

    fn read_decode(&mut self, buf: &mut [u8]) -> crate::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.end_reached {
            return Ok(0);
        }
        let mut size: u64 = 0;
        let mut len = buf.len() as u64;
        let mut off: u64 = 0;
        while len > 0 {
            let mut copy_size_max = len;
            if self.remaining_size <= u64::MAX / 2 && self.remaining_size < len {
                copy_size_max = self.remaining_size;
            }
            self.lz.set_limit(copy_size_max as usize);

            match self.lzma.decode(&mut self.lz, &mut self.rc) {
                Ok(_) => {}
                Err(error) => {
                    if self.remaining_size != u64::MAX || !self.lzma.end_marker_detected() {
                        return Err(error);
                    }
                    self.end_reached = true;
                    self.rc.normalize();
                }
            }

            let copied_size = self.lz.flush(buf, off as _)? as u64;
            off = off.saturating_add(copied_size);
            len = len.saturating_sub(copied_size);
            size = size.saturating_add(copied_size);
            if self.remaining_size <= u64::MAX / 2 {
                self.remaining_size = self.remaining_size.saturating_sub(copied_size);
                if self.remaining_size == 0 {
                    self.end_reached = true;
                }
            }

            if self.end_reached {
                if self.lz.has_pending()
                    || (!self.relaxed_end_cond && !self.rc.is_stream_finished())
                {
                    return Err(error_invalid_data("end reached but not decoder finished"));
                }
                return Ok(size as _);
            }
        }
        Ok(size as _)
    }
}

impl<R: Read> Read for LzmaReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> crate::Result<usize> {
        self.read_decode(buf)
    }
}
