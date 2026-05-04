use std::cmp;

use miniz_oxide::inflate::{
    core::{
        decompress,
        inflate_flags::{TINFL_FLAG_HAS_MORE_INPUT, TINFL_FLAG_IGNORE_ADLER32},
        DecompressorOxide,
    },
    TINFLStatus,
};
use tracing::trace;

use crate::{fsm::entry::HasMoreInput, parse::Method, Error};

use super::{DecompressOutcome, Decompressor};

pub(crate) struct DeflateDec {
    /// 64 KiB circular internal buffer. From miniz_oxide docs:
    ///
    /// > The decompression function normally needs access to 32KiB of the
    /// > previously decompressed data (or to the beginning of the decompressed
    /// > data if less than 32KiB has been decompressed.)
    internal_buffer: Vec<u8>,

    /// The position in the internal buffer where we should start writing the
    /// next decompressed data. Note that the buffer is circular, so we need to
    /// wrap around when we reach the end.
    out_pos: usize,

    /// If this is non-zero, there's data *after* [Self::out_pos] we haven't
    /// copied to the caller's output buffer yet. As we copy it, we'll decrease
    /// this value and increase [Self::out_pos]. When it reaches zero, we'll
    /// need to call miniz_oxide again to get more data.
    remain_in_internal_buffer: usize,

    /// The miniz_oxide decompressor state
    state: DecompressorOxide,
}

impl Default for DeflateDec {
    fn default() -> Self {
        Self {
            internal_buffer: vec![0u8; Self::INTERNAL_BUFFER_LENGTH],
            out_pos: 0,
            state: DecompressorOxide::new(),
            remain_in_internal_buffer: 0,
        }
    }
}

impl Decompressor for DeflateDec {
    fn decompress(
        &mut self,
        in_buf: &[u8],
        out: &mut [u8],
        has_more_input: HasMoreInput,
    ) -> Result<DecompressOutcome, Error> {
        tracing::trace!(
            in_buf_len = in_buf.len(),
            out_len = out.len(),
            remain_in_internal_buffer = self.remain_in_internal_buffer,
            out_pos = self.out_pos,
            "decompress",
        );

        let mut outcome: DecompressOutcome = Default::default();
        self.copy_to_out(out, &mut outcome);
        if outcome.bytes_written > 0 {
            tracing::trace!(
                "returning {} bytes from internal buffer",
                outcome.bytes_written
            );
            return Ok(outcome);
        }

        // no output bytes, let's call miniz_oxide

        let mut flags = TINFL_FLAG_IGNORE_ADLER32;
        if matches!(has_more_input, HasMoreInput::Yes) {
            flags |= TINFL_FLAG_HAS_MORE_INPUT;
        }

        let (status, bytes_read, bytes_written) = decompress(
            &mut self.state,
            in_buf,
            &mut self.internal_buffer,
            self.out_pos,
            flags,
        );
        trace!(%bytes_read, %bytes_written, ?status, "decompress returned");
        outcome.bytes_read += bytes_read;
        self.remain_in_internal_buffer += bytes_written;

        match status {
            TINFLStatus::FailedCannotMakeProgress => {
                return Err(Error::Decompression { method: Method::Deflate, msg: "Failed to make progress: more input data was expected, but the caller indicated there was no more data, so the input stream is likely truncated".to_string() })
            }
            TINFLStatus::BadParam => {
				return Err(Error::Decompression { method: Method::Deflate, msg: "The output buffer is an invalid size; consider the flags parameter".to_string() })
			}
			TINFLStatus::Adler32Mismatch => {
				return Err(Error::Decompression { method: Method::Deflate, msg: "The decompression went fine, but the adler32 checksum did not match the one provided in the header.".to_string() })
			}
            TINFLStatus::Failed => {
				return Err(Error::Decompression { method: Method::Deflate, msg: "Failed to decompress due to invalid data.".to_string() })
			},
            TINFLStatus::Done => {
				// eventually this'll return bytes_written == 0
			},
            TINFLStatus::NeedsMoreInput => {
				// that's okay, we'll get more input next time
			},
            TINFLStatus::HasMoreOutput => {
				// that's okay, as long as we return bytes_written > 0
				// the caller will keep calling
			},
        }

        trace!("calling copy_to_out");
        self.copy_to_out(out, &mut outcome);
        Ok(outcome)
    }
}

impl DeflateDec {
    const INTERNAL_BUFFER_LENGTH: usize = 64 * 1024;

    fn copy_to_out(&mut self, mut out: &mut [u8], outcome: &mut DecompressOutcome) {
        // as long as there's room in out_buf and we have remaining data in the
        // internal buffer, copy from internal_buffer wrapping as needed,
        // decreasing self.remain_in_internal_buffer and increasing self.out_pos
        // and outcome.bytes_written
        while !out.is_empty() && self.remain_in_internal_buffer > 0 {
            let copy_len = cmp::min(self.remain_in_internal_buffer, out.len());
            // take wrapping into account
            let copy_len = cmp::min(copy_len, self.internal_buffer.len() - self.out_pos);
            trace!("copying {} bytes from internal buffer to out_buf", copy_len);

            out[..copy_len].copy_from_slice(&self.internal_buffer[self.out_pos..][..copy_len]);
            self.out_pos += copy_len;
            outcome.bytes_written += copy_len;
            self.remain_in_internal_buffer -= copy_len;
            out = &mut out[copy_len..];

            // if we've reached the end of the buffer, wrap around
            if self.out_pos == self.internal_buffer.len() {
                self.out_pos = 0;
            }
        }
    }
}
