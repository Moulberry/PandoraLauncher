use rc_zip::{
    fsm::{EntryFsm, FsmResult},
    Entry,
};
use std::io;
use tracing::trace;

/// Reader for an entry inside an archive
pub struct EntryReader<R>
where
    R: io::Read,
{
    rd: R,
    fsm: Option<EntryFsm>,
}

impl<R> EntryReader<R>
where
    R: io::Read,
{
    pub(crate) fn new(entry: &Entry, rd: R) -> Self {
        Self {
            rd,
            fsm: Some(EntryFsm::new(Some(entry.clone()), None)),
        }
    }
}

impl<R> io::Read for EntryReader<R>
where
    R: io::Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let mut fsm = match self.fsm.take() {
                Some(fsm) => fsm,
                None => return Ok(0),
            };

            #[allow(clippy::needless_late_init)] // don't tell me what to do
            let filled_bytes;
            if fsm.wants_read() {
                tracing::trace!(space_avail = fsm.space().len(), "fsm wants read");
                let n = self.rd.read(fsm.space())?;
                fsm.fill(n);
                filled_bytes = n;
            } else {
                trace!("fsm does not want read");
                filled_bytes = 0;
            }

            match fsm.process(buf)? {
                FsmResult::Continue((fsm, outcome)) => {
                    self.fsm = Some(fsm);

                    if outcome.bytes_written > 0 {
                        tracing::trace!("wrote {} bytes", outcome.bytes_written);
                        return Ok(outcome.bytes_written);
                    } else if filled_bytes > 0 || outcome.bytes_read > 0 {
                        // progress was made, keep reading
                        continue;
                    } else {
                        return Err(io::Error::other("entry reader: no progress"));
                    }
                }
                FsmResult::Done(_) => {
                    // neat!
                    return Ok(0);
                }
            }
        }
    }
}
