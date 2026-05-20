use crate::{parse::Method, Error};

use super::{DecompressOutcome, Decompressor, HasMoreInput};

pub(crate) struct Bzip2Dec {
    inner: bzip2::Decompress,
    eof: bool,
}

impl Default for Bzip2Dec {
    fn default() -> Self {
        // don't use the 'small' alternative decompression algorithm
        let small = false;
        Self {
            inner: bzip2::Decompress::new(small),
            eof: false,
        }
    }
}

impl Decompressor for Bzip2Dec {
    fn decompress(
        &mut self,
        in_buf: &[u8],
        out: &mut [u8],
        _has_more_input: HasMoreInput,
    ) -> Result<DecompressOutcome, Error> {
        tracing::trace!(
            in_buf_len = in_buf.len(),
            out_len = out.len(),
            total_in = self.inner.total_in(),
            total_out = self.inner.total_out(),
            "Bzip2Dec::decompress",
        );

        if self.eof {
            return Ok(DecompressOutcome {
                bytes_written: 0,
                bytes_read: 0,
            });
        }

        let before_in = self.inner.total_in();
        let before_out = self.inner.total_out();

        match self.inner.decompress(in_buf, out) {
            Ok(status) => {
                tracing::trace!("status: {:?}", status);
                if status == bzip2::Status::StreamEnd {
                    self.eof = true;
                }
            }
            Err(e) => {
                return Err(Error::Decompression {
                    method: Method::Bzip2,
                    msg: e.to_string(),
                })
            }
        };

        let outcome = DecompressOutcome {
            bytes_written: (self.inner.total_out() - before_out) as usize,
            bytes_read: (self.inner.total_in() - before_in) as usize,
        };
        Ok(outcome)
    }
}
