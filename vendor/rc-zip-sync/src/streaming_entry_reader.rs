use oval::Buffer;
use rc_zip::{
    error::FormatError,
    fsm::{EntryFsm, FsmResult},
    Entry, Error,
};
use std::io::{self, Read};
use tracing::trace;

/// Reads a zip entry based on a local header. Some information is missing,
/// not all name encodings may work, and only by reading it in its entirety
/// can you move on to the next entry.
///
/// However, it only requires an [io::Read], and does not need to seek.
pub struct StreamingEntryReader<R> {
    entry: Entry,
    rd: R,
    state: State,
}

#[derive(Default)]
#[allow(clippy::large_enum_variant)]
enum State {
    Reading {
        fsm: EntryFsm,
    },
    Finished {
        /// remaining buffer for next entry
        remain: Buffer,
    },
    #[default]
    Transition,
}

impl<R> StreamingEntryReader<R>
where
    R: io::Read,
{
    pub(crate) fn new(fsm: EntryFsm, entry: Entry, rd: R) -> Self {
        Self {
            entry,
            rd,
            state: State::Reading { fsm },
        }
    }
}

impl<R> io::Read for StreamingEntryReader<R>
where
    R: io::Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        trace!("reading from streaming entry reader");

        match std::mem::take(&mut self.state) {
            State::Reading { mut fsm } => {
                if fsm.wants_read() {
                    trace!("fsm wants read");
                    let n = self.rd.read(fsm.space())?;
                    trace!("giving fsm {} bytes from rd", n);
                    fsm.fill(n);
                } else {
                    trace!("fsm does not want read");
                }

                match fsm.process(buf)? {
                    FsmResult::Continue((fsm, outcome)) => {
                        trace!("fsm wants to continue");
                        self.state = State::Reading { fsm };

                        if outcome.bytes_written > 0 {
                            trace!("bytes have been written");
                            Ok(outcome.bytes_written)
                        } else if outcome.bytes_read == 0 {
                            trace!("no bytes have been written or read");
                            // that's EOF, baby!
                            Ok(0)
                        } else {
                            trace!("read some bytes, hopefully will write more later");
                            // loop, it happens
                            self.read(buf)
                        }
                    }
                    FsmResult::Done(remain) => {
                        self.state = State::Finished { remain };

                        // neat!
                        Ok(0)
                    }
                }
            }
            State::Finished { remain } => {
                // wait for them to call finish
                self.state = State::Finished { remain };
                Ok(0)
            }
            State::Transition => unreachable!(),
        }
    }
}

impl<R> StreamingEntryReader<R>
where
    R: io::Read,
{
    /// Return entry information for this reader
    #[inline(always)]
    pub fn entry(&self) -> &Entry {
        &self.entry
    }

    /// Finish reading this entry, returning the next streaming entry reader, if
    /// any. This panics if the entry is not fully read.
    ///
    /// If this returns None, there's no entries left.
    pub fn finish(mut self) -> Result<Option<StreamingEntryReader<R>>, Error> {
        trace!("finishing streaming entry reader");

        if matches!(self.state, State::Reading { .. }) {
            // this should transition to finished if there's no data
            _ = self.read(&mut [0u8; 1])?;
        }

        match self.state {
            State::Reading { .. } => {
                panic!("entry not fully read");
            }
            State::Finished { remain } => {
                // parse the next entry, if any
                let mut fsm = EntryFsm::new(None, Some(remain));

                loop {
                    if fsm.wants_read() {
                        let n = self.rd.read(fsm.space())?;
                        trace!("read {} bytes into buf for first zip entry", n);
                        fsm.fill(n);
                    }

                    match fsm.process_till_header() {
                        Ok(Some(entry)) => {
                            let entry = entry.clone();
                            return Ok(Some(StreamingEntryReader::new(fsm, entry, self.rd)));
                        }
                        Ok(None) => {
                            // needs more turns
                        }
                        Err(e) => match e {
                            Error::Format(FormatError::InvalidLocalHeader) => {
                                // we probably reached the end of central directory!
                                // TODO: we should probably check for the end of central directory
                                return Ok(None);
                            }
                            _ => return Err(e),
                        },
                    }
                }
            }
            State::Transition => unreachable!(),
        }
    }
}
