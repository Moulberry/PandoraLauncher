use std::cmp;

use crate::Error;

use super::{DecompressOutcome, Decompressor, HasMoreInput};

#[derive(Default)]
pub(crate) struct StoreDec;

impl Decompressor for StoreDec {
    fn decompress(
        &mut self,
        in_buf: &[u8],
        out_buf: &mut [u8],
        _has_more_input: HasMoreInput,
    ) -> Result<DecompressOutcome, Error> {
        let len = cmp::min(in_buf.len(), out_buf.len());
        out_buf[..len].copy_from_slice(&in_buf[..len]);
        Ok(DecompressOutcome {
            bytes_read: len,
            bytes_written: len,
        })
    }
}
