use std::{cmp, io::Write};

use crate::{parse::Method, Error};

use super::{DecompressOutcome, Decompressor, HasMoreInput};

use tracing::trace;
use zstd::stream::write::Decoder;

#[derive(Default)]
enum State {
    Writing(Box<Decoder<'static, Vec<u8>>>),
    Draining(Vec<u8>),

    #[default]
    Transition,
}

pub(crate) struct ZstdDec {
    state: State,
}

impl ZstdDec {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            state: State::Writing(Box::new(Decoder::new(vec![])?)),
        })
    }
}

impl Decompressor for ZstdDec {
    fn decompress(
        &mut self,
        mut in_buf: &[u8],
        out: &mut [u8],
        has_more_input: HasMoreInput,
    ) -> Result<DecompressOutcome, Error> {
        let mut outcome: DecompressOutcome = Default::default();

        loop {
            tracing::trace!(
                in_buf_len = in_buf.len(),
                out_len = out.len(),
                remain_in_internal_buffer = self.internal_buf_mut().len(),
                ?outcome,
                "decompress",
            );

            self.copy_to_out(out, &mut outcome);
            if outcome.bytes_written > 0 {
                trace!(
                    "still draining internal buffer, just copied {} bytes",
                    outcome.bytes_written
                );
                return Ok(outcome);
            }

            match &mut self.state {
                State::Writing(stream) => {
                    let n = stream.write(in_buf).map_err(dec_err)?;
                    trace!(
                        "wrote {} bytes to decompressor (of {} available)",
                        n,
                        in_buf.len()
                    );
                    outcome.bytes_read += n;
                    in_buf = &in_buf[n..];

                    // if we wrote some of the input, and we haven't gotten any
                    // output, then we need to loop
                    if n > 0 && n < in_buf.len() && self.internal_buf_mut().is_empty() {
                        trace!("fed _some_ to the decoder and no output yet, keep going");
                        continue;
                    }

                    match has_more_input {
                        HasMoreInput::Yes => {
                            trace!("more input to come");
                        }
                        HasMoreInput::No => {
                            trace!("no more input to come");

                            match in_buf.len() {
                                0 => {
                                    // no trailer, good
                                }
                                1 => {
                                    // TODO: figure out a good explanation for this.
                                    // in some test files the compressed size is 37 bytes but
                                    // the zstd decompressor will only accept 36 bytes.
                                    trace!("eating ZSTD trailer?");
                                    outcome.bytes_read += 1;
                                }
                                _ => {
                                    return Err(Error::Decompression { method: Method::Zstd, msg: format!("expected ZSTD trailer or no ZSTD trailer, but not a {}-byte trailer", in_buf.len()) });
                                }
                            }

                            match std::mem::take(&mut self.state) {
                                State::Writing(mut stream) => {
                                    trace!("finishing...");
                                    stream.flush().map_err(dec_err)?;
                                    self.state = State::Draining(stream.into_inner());
                                    continue;
                                }
                                _ => unreachable!(),
                            }
                        }
                    }
                }
                State::Draining(_) => {
                    // keep going
                    trace!("draining");
                }
                State::Transition => unreachable!(),
            }

            self.copy_to_out(out, &mut outcome);
            trace!("decompressor gave us {} bytes", outcome.bytes_written);
            return Ok(outcome);
        }
    }
}

fn dec_err(e: impl std::fmt::Display) -> Error {
    Error::Decompression {
        method: Method::Zstd,
        msg: e.to_string(),
    }
}

impl ZstdDec {
    #[inline(always)]
    fn internal_buf_mut(&mut self) -> &mut Vec<u8> {
        match &mut self.state {
            State::Writing(stream) => stream.get_mut(),
            State::Draining(buf) => buf,
            State::Transition => unreachable!(),
        }
    }

    fn copy_to_out(&mut self, mut out: &mut [u8], outcome: &mut DecompressOutcome) {
        let internal_buf = self.internal_buf_mut();

        while !out.is_empty() && !internal_buf.is_empty() {
            let to_copy = cmp::min(out.len(), internal_buf.len());
            trace!("copying {} bytes from internal buffer", to_copy);
            out[..to_copy].copy_from_slice(&internal_buf[..to_copy]);
            out = &mut out[to_copy..];

            // rotate the internal buffer
            internal_buf.rotate_left(to_copy);
            // and shrink it
            internal_buf.resize(internal_buf.len() - to_copy, 0);

            outcome.bytes_written += to_copy;
        }
    }
}
