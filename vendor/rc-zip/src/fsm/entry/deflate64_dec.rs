use deflate64::InflaterManaged;

use crate::{parse::Method, Error};

use super::{DecompressOutcome, Decompressor, HasMoreInput};

pub(crate) struct Deflate64Dec {
    inflater: InflaterManaged,
}

impl Default for Deflate64Dec {
    fn default() -> Self {
        Self {
            inflater: InflaterManaged::new(),
        }
    }
}

impl Decompressor for Deflate64Dec {
    fn decompress(
        &mut self,
        in_buf: &[u8],
        out: &mut [u8],
        _has_more_input: HasMoreInput,
    ) -> Result<DecompressOutcome, Error> {
        tracing::trace!(
            in_buf_len = in_buf.len(),
            out_len = out.len(),
            remain_in_internal_buffer = self.inflater.available_output(),
            "decompress",
        );

        let res = self.inflater.inflate(in_buf, out);
        if res.data_error {
            return Err(Error::Decompression {
                method: Method::Deflate64,
                msg: "data error".into(),
            });
        }

        let outcome = DecompressOutcome {
            bytes_read: res.bytes_consumed,
            bytes_written: res.bytes_written,
        };
        Ok(outcome)
    }
}
