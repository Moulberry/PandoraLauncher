use alloc::boxed::Box;

use super::{
    BlockHeader, ChecksumCalculator, FilterType, Index, StreamFooter, StreamHeader, XZ_MAGIC,
};
use crate::{
    CountingReader, Lzma2Reader, Read, Result, error_invalid_data,
    filter::{bcj::BcjReader, delta::DeltaReader},
};

#[allow(clippy::large_enum_variant)]
enum FilterReader<R: Read> {
    Counting(CountingReader<R>),
    Lzma2(Lzma2Reader<Box<FilterReader<R>>>),
    Delta(DeltaReader<Box<FilterReader<R>>>),
    Bcj(BcjReader<Box<FilterReader<R>>>),
    Dummy,
}

impl<R: Read> Read for FilterReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match self {
            FilterReader::Counting(reader) => reader.read(buf),
            FilterReader::Lzma2(reader) => reader.read(buf),
            FilterReader::Delta(reader) => reader.read(buf),
            FilterReader::Bcj(reader) => reader.read(buf),
            FilterReader::Dummy => unimplemented!(),
        }
    }
}

impl<R: Read> FilterReader<R> {
    fn create_filter_chain(inner: R, filters: &[Option<FilterType>], properties: &[u32]) -> Self {
        let mut chain_reader = FilterReader::Counting(CountingReader::new(inner));

        for (filter, property) in filters
            .iter()
            .copied()
            .zip(properties)
            .filter_map(|(filter, property)| filter.map(|filter| (filter, *property)))
            .rev()
        {
            chain_reader = match filter {
                FilterType::Delta => {
                    let distance = property as usize;
                    FilterReader::Delta(DeltaReader::new(Box::new(chain_reader), distance))
                }
                FilterType::BcjX86 => {
                    let start_offset = property as usize;
                    FilterReader::Bcj(BcjReader::new_x86(Box::new(chain_reader), start_offset))
                }
                FilterType::BcjPpc => {
                    let start_offset = property as usize;
                    FilterReader::Bcj(BcjReader::new_ppc(Box::new(chain_reader), start_offset))
                }
                FilterType::BcjIa64 => {
                    let start_offset = property as usize;
                    FilterReader::Bcj(BcjReader::new_ia64(Box::new(chain_reader), start_offset))
                }
                FilterType::BcjArm => {
                    let start_offset = property as usize;
                    FilterReader::Bcj(BcjReader::new_arm(Box::new(chain_reader), start_offset))
                }
                FilterType::BcjArmThumb => {
                    let start_offset = property as usize;
                    FilterReader::Bcj(BcjReader::new_arm_thumb(
                        Box::new(chain_reader),
                        start_offset,
                    ))
                }
                FilterType::BcjSparc => {
                    let start_offset = property as usize;
                    FilterReader::Bcj(BcjReader::new_sparc(Box::new(chain_reader), start_offset))
                }
                FilterType::BcjArm64 => {
                    let start_offset = property as usize;
                    FilterReader::Bcj(BcjReader::new_arm64(Box::new(chain_reader), start_offset))
                }
                FilterType::BcjRiscv => {
                    let start_offset = property as usize;
                    FilterReader::Bcj(BcjReader::new_riscv(Box::new(chain_reader), start_offset))
                }
                FilterType::Lzma2 => {
                    let dict_size = property;
                    FilterReader::Lzma2(Lzma2Reader::new(Box::new(chain_reader), dict_size, None))
                }
            };
        }

        chain_reader
    }

    fn bytes_read(&self) -> u64 {
        match self {
            FilterReader::Counting(reader) => reader.bytes_read(),
            FilterReader::Lzma2(reader) => reader.inner().bytes_read(),
            FilterReader::Delta(reader) => reader.inner().bytes_read(),
            FilterReader::Bcj(reader) => reader.inner().bytes_read(),
            FilterReader::Dummy => unimplemented!(),
        }
    }

    fn into_inner(self) -> R {
        match self {
            FilterReader::Counting(reader) => reader.inner,
            FilterReader::Lzma2(reader) => {
                let filter_reader = reader.into_inner();
                filter_reader.into_inner()
            }
            FilterReader::Delta(reader) => {
                let filter_reader = reader.into_inner();
                filter_reader.into_inner()
            }
            FilterReader::Bcj(reader) => {
                let filter_reader = reader.into_inner();
                filter_reader.into_inner()
            }
            FilterReader::Dummy => unimplemented!(),
        }
    }

    fn inner(&self) -> &R {
        match self {
            FilterReader::Counting(reader) => &reader.inner,
            FilterReader::Lzma2(reader) => {
                let filter_reader = reader.inner();

                filter_reader.inner()
            }
            FilterReader::Delta(reader) => {
                let filter_reader = reader.inner();
                filter_reader.inner()
            }
            FilterReader::Bcj(reader) => {
                let filter_reader = reader.inner();
                filter_reader.inner()
            }
            FilterReader::Dummy => unimplemented!(),
        }
    }

    fn inner_mut(&mut self) -> &mut R {
        match self {
            FilterReader::Counting(reader) => &mut reader.inner,
            FilterReader::Lzma2(reader) => {
                let filter_reader = reader.inner_mut();
                filter_reader.inner_mut()
            }
            FilterReader::Delta(reader) => {
                let filter_reader = reader.inner_mut();
                filter_reader.inner_mut()
            }
            FilterReader::Bcj(reader) => {
                let filter_reader = reader.inner_mut();
                filter_reader.inner_mut()
            }
            FilterReader::Dummy => unimplemented!(),
        }
    }
}

/// A single-threaded XZ decompressor.
pub struct XzReader<R: Read> {
    reader: FilterReader<R>,
    stream_header: Option<StreamHeader>,
    checksum_calculator: Option<ChecksumCalculator>,
    finished: bool,
    allow_multiple_streams: bool,
    blocks_processed: u64,
}

impl<R: Read> XzReader<R> {
    /// Create a new [`XzReader`].
    pub fn new(inner: R, allow_multiple_streams: bool) -> Self {
        let reader = FilterReader::Counting(CountingReader::new(inner));

        Self {
            reader,
            stream_header: None,
            checksum_calculator: None,
            finished: false,
            allow_multiple_streams,
            blocks_processed: 0,
        }
    }

    /// Consume the XzReader and return the inner reader.
    pub fn into_inner(self) -> R {
        self.reader.into_inner()
    }

    /// Returns a reference to the inner reader.
    pub fn inner(&self) -> &R {
        self.reader.inner()
    }

    /// Returns a mutable reference to the inner reader.
    pub fn inner_mut(&mut self) -> &mut R {
        self.reader.inner_mut()
    }
}

impl<R: Read> XzReader<R> {
    fn ensure_stream_header(&mut self) -> Result<()> {
        if self.stream_header.is_none() {
            let header = StreamHeader::parse(&mut self.reader)?;
            self.stream_header = Some(header);
        }
        Ok(())
    }

    fn prepare_next_block(&mut self) -> Result<bool> {
        match BlockHeader::parse(&mut self.reader)? {
            Some(block_header) => {
                let base_reader: FilterReader<R> =
                    core::mem::replace(&mut self.reader, FilterReader::Dummy);

                self.reader = FilterReader::create_filter_chain(
                    base_reader.into_inner(),
                    &block_header.filters,
                    &block_header.properties,
                );

                match self.stream_header.as_ref() {
                    Some(header) => {
                        self.checksum_calculator = Some(ChecksumCalculator::new(header.check_type));
                    }
                    None => {
                        panic!("stream_header not set");
                    }
                }

                self.blocks_processed += 1;

                Ok(true)
            }
            None => {
                // End of blocks reached, index follows.
                self.parse_index_and_footer()?;

                if self.allow_multiple_streams && self.try_start_next_stream()? {
                    return self.prepare_next_block();
                }

                self.finished = true;
                Ok(false)
            }
        }
    }

    fn consume_padding(&mut self, compressed_bytes: u64) -> Result<()> {
        let padding_needed = match (4 - (compressed_bytes % 4)) % 4 {
            0 => return Ok(()),
            n => n as usize,
        };

        let mut padding_buf = [0u8; 3];

        let bytes_read = self.reader.read(&mut padding_buf[..padding_needed])?;

        if bytes_read != padding_needed {
            return Err(error_invalid_data("incomplete XZ block padding"));
        }

        if !padding_buf[..bytes_read].iter().all(|&byte| byte == 0) {
            return Err(error_invalid_data("invalid XZ block padding"));
        }

        Ok(())
    }

    fn verify_block_checksum(&mut self) -> Result<()> {
        let checksum_calculator = self
            .checksum_calculator
            .take()
            .expect("checksum_calculator not set");

        match checksum_calculator {
            ChecksumCalculator::None => { /* Nothing to check */ }
            ChecksumCalculator::Crc32(_) => {
                let mut checksum = [0u8; 4];
                self.reader.read_exact(&mut checksum)?;

                if !checksum_calculator.verify(&checksum) {
                    return Err(error_invalid_data("invalid block checksum"));
                }
            }
            ChecksumCalculator::Crc64(_) => {
                let mut checksum = [0u8; 8];
                self.reader.read_exact(&mut checksum)?;

                if !checksum_calculator.verify(&checksum) {
                    return Err(error_invalid_data("invalid block checksum"));
                }
            }
            ChecksumCalculator::Sha256(_) => {
                let mut checksum = [0u8; 32];
                self.reader.read_exact(&mut checksum)?;

                if !checksum_calculator.verify(&checksum) {
                    return Err(error_invalid_data("invalid block checksum"));
                }
            }
        }

        Ok(())
    }

    /// Look for the start of the next stream by reading bytes one at a time
    /// and checking for the XZ magic sequence, allowing for stream padding.
    fn try_start_next_stream(&mut self) -> Result<bool> {
        let mut padding_bytes = 0;
        let mut buffer = [0u8; 6];

        loop {
            let mut byte_buffer = [0u8; 1];
            let read = self.reader.read(&mut byte_buffer)?;
            if read == 0 {
                // EOF reached, no more streams.
                return Ok(false);
            }

            let byte = byte_buffer[0];

            if byte == 0 {
                // Potential stream padding.
                padding_bytes += 1;
                continue;
            }

            // Non-zero byte found - check if it starts XZ magic.
            if byte != XZ_MAGIC[0] {
                return Err(error_invalid_data("invalid data after stream"));
            }

            buffer[0] = byte;
            let mut buffer_pos = 1;

            // Read the rest of the magic bytes.
            while buffer_pos < 6 {
                match self.reader.read(&mut byte_buffer)? {
                    0 => {
                        return Err(error_invalid_data("incomplete XZ magic bytes"));
                    }
                    1 => {
                        buffer[buffer_pos] = byte_buffer[0];
                        buffer_pos += 1;
                    }
                    _ => unreachable!(),
                }
            }

            if buffer != XZ_MAGIC {
                return Err(error_invalid_data("invalid data after stream padding"));
            }

            if padding_bytes % 4 != 0 {
                return Err(error_invalid_data("stream padding size not multiple of 4"));
            }

            let stream_header = StreamHeader::parse_stream_header_flags_and_crc(&mut self.reader)?;

            // Reset state for new stream.
            self.stream_header = Some(stream_header);
            self.blocks_processed = 0;

            return Ok(true);
        }
    }

    fn parse_index_and_footer(&mut self) -> Result<()> {
        let index = Index::parse(&mut self.reader)?;

        if index.number_of_records != self.blocks_processed {
            return Err(error_invalid_data(
                "number of blocks processed doesn't match index records",
            ));
        }

        let stream_footer = StreamFooter::parse(&mut self.reader)?;

        let header = self.stream_header.as_ref().expect("stream_header not set");

        let header_flags = [0, header.check_type as u8];
        if stream_footer.stream_flags != header_flags {
            return Err(error_invalid_data(
                "stream header and footer flags mismatch",
            ));
        }

        Ok(())
    }
}

impl<R: Read> Read for XzReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.finished {
            return Ok(0);
        }

        self.ensure_stream_header()?;

        loop {
            if self.checksum_calculator.is_some() {
                let bytes_read = self.reader.read(buf)?;

                if bytes_read > 0 {
                    if let Some(ref mut calc) = self.checksum_calculator {
                        calc.update(&buf[..bytes_read]);
                    }

                    return Ok(bytes_read);
                } else {
                    let reader = core::mem::replace(&mut self.reader, FilterReader::Dummy);
                    let compressed_bytes = reader.bytes_read();
                    self.reader = FilterReader::Counting(CountingReader::with_count(
                        reader.into_inner(),
                        compressed_bytes,
                    ));

                    self.consume_padding(compressed_bytes)?;
                    self.verify_block_checksum()?;
                }
            } else {
                // No current block, prepare the next one.
                if !self.prepare_next_block()? {
                    // No more blocks, we're done.
                    return Ok(0);
                }
            }
        }
    }
}
