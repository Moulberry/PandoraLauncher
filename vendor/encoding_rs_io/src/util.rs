use std::cmp;
use std::io;

use encoding_rs::{CoderResult, Decoder, Encoding};

/// This is the minimum amount of space that a decoder-to-utf8-with-replacement
/// will use for any state and any input.
const TINY_BUFFER_SIZE: usize = 7;

/// A tiny transcoder performs transcoding incrementally even when a caller
/// provided buffer is not large enough.
///
/// This use case comes up when implementing streaming transcoding in cases
/// where it is permissible to provide incomplete UTF-8 sequences to the
/// caller (e.g., when decoding into a `&[u8]` where the caller must be capable
/// of handling invalid UTF-8). In particular, this type specifically handles
/// cases where a caller provided buffer is too small to store a full UTF-8
/// sequence. Thus, this type should be used in cases where the caller provided
/// buffer has length 3 or fewer.
///
/// This could likely be done with better performance by allocating a larger
/// buffer for these cases, but we instead opt to handle this without
/// allocation under the assumption that tiny caller provided buffers are
/// probably a pathological case.
#[derive(Clone, Debug)]
pub struct TinyTranscoder {
    /// This is where we store the results of a transcoding. Since we are
    /// always decoding to UTF-8, 7 bytes is sufficient to represent any
    /// codepoint.
    partial: [u8; TINY_BUFFER_SIZE],
    /// The number of bytes written in `partial`.
    len: usize,
    /// The position in `partial` at which the next byte should be read.
    pos: usize,
}

impl TinyTranscoder {
    /// Create a new tiny transcoder that is ready for use.
    pub fn new() -> TinyTranscoder {
        TinyTranscoder { partial: [0; TINY_BUFFER_SIZE], len: 0, pos: 0 }
    }

    /// Transcode the contents of `src` into this buffer using the provided
    /// decoder, and return the number of bytes consumed in `src` and the
    /// number of bytes written to this transcoder.
    ///
    /// The results of transcoding can be read using the TinyTranscoder's
    /// `io::Read` implementation.
    ///
    /// If `last` is true, then this signals to the decoder that we've reached
    /// EOF and `src` must be empty. Otherwise, if `last` is false, then
    /// `src` must be non-empty. Violating either of these constraits will
    /// cause a panic.
    ///
    /// Finally, if this transcoder still has unconsumed bytes from a previous
    /// transcode, then this panics. Callers must consume all bytes from a
    /// previous transcoding before performing another one.
    pub fn transcode(
        &mut self,
        decoder: &mut Decoder,
        src: &[u8],
        last: bool,
    ) -> (usize, usize) {
        assert!(self.as_slice().is_empty(), "transcoder has unconsumed bytes");
        if last {
            assert!(src.is_empty(), "src must be empty when last==true");
        }
        let (res, nin, nout, _) =
            decoder.decode_to_utf8(src, &mut self.partial[..], last);
        if last {
            assert_eq!(
                res,
                CoderResult::InputEmpty,
                "input should be exhausted",
            );
        }
        self.pos = 0;
        self.len = nout;
        (nin, nout)
    }

    /// Return the the bytes remaining to be read as a slice.
    fn as_slice(&self) -> &[u8] {
        &self.partial[self.pos..self.len]
    }
}

impl io::Read for TinyTranscoder {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.len {
            return Ok(0);
        }
        let mut count = 0;
        for (src, dst) in self.as_slice().iter().zip(buf) {
            *dst = *src;
            count += 1;
        }
        self.pos += count;
        Ok(count)
    }
}

/// `BomPeeker` wraps `R` and satisfies the `io::Read` interface while also
/// providing a peek at the BOM if one exists. Peeking at the BOM does not
/// advance the reader.
#[derive(Debug)]
pub struct BomPeeker<R> {
    rdr: R,
    strip: bool,
    bom: Option<PossibleBom>,
    bom_pos: usize,
}

impl<R: io::Read> BomPeeker<R> {
    /// Create a new BomPeeker that includes the BOM in calls to `read`.
    ///
    /// The first three bytes can be read using the `peek_bom` method, but
    /// will not advance the reader.
    pub fn with_bom(rdr: R) -> BomPeeker<R> {
        BomPeeker { rdr, strip: false, bom: None, bom_pos: 0 }
    }

    /// Create a new BomPeeker that never includes the BOM in calls to `read`.
    pub fn without_bom(rdr: R) -> BomPeeker<R> {
        BomPeeker { rdr, strip: true, bom: None, bom_pos: 0 }
    }

    /// Peek at the first three bytes of the underlying reader.
    ///
    /// This does not advance the reader provided by `BomPeeker`.
    ///
    /// If the underlying reader does not have at least two bytes available,
    /// then `None` is returned.
    pub fn peek_bom(&mut self) -> io::Result<PossibleBom> {
        if let Some(bom) = self.bom {
            return Ok(bom);
        }
        // If the underlying reader fails or panics, make sure we set at least
        // an empty BOM so that we don't end up here again..
        self.bom = Some(PossibleBom::new());

        // OK, try to read the BOM.
        let mut buf = [0u8; 3];
        let bom_len = read_full(&mut self.rdr, &mut buf)?;
        self.bom = Some(PossibleBom { bytes: buf, len: bom_len });
        Ok(self.bom.unwrap())
    }
}

impl<R: io::Read> io::Read for BomPeeker<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.bom_pos < 3 {
            let bom = self.peek_bom()?;

            // If we don't have a valid BOM (e.g., no encoding for it), then
            // we always pass through the first 3 bytes. Otherwise, if we have
            // a valid BOM, we only pass it thru if we don't want to strip it.
            let bom = bom.as_slice(!self.strip);
            if self.bom_pos < bom.len() {
                let rest = &bom[self.bom_pos..];
                let len = cmp::min(buf.len(), rest.len());
                buf[..len].copy_from_slice(&rest[..len]);
                self.bom_pos += len;
                return Ok(len);
            }
            self.bom_pos = 3;
        }
        self.rdr.read(buf)
    }
}

/// A PossibleBom is a sequence of bytes at the beginning of a stream that
/// may represent an actual BOM. To detect the BOM, this must contain at
/// least 3 bytes.
///
/// If this is a valid UTF-8 or UTF-16 BOM, then an encoding_rs decoder can
/// be built from the BOM.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PossibleBom {
    bytes: [u8; 3],
    len: usize,
}

impl PossibleBom {
    /// Build a new empty BOM.
    fn new() -> PossibleBom {
        PossibleBom { bytes: [0; 3], len: 0 }
    }

    /// Return the BOM as a normal slice.
    ///
    /// If `bom` is true, then this includes any leading BOM bytes. Otherwise,
    /// this only includes non-BOM bytes.
    fn as_slice(&self, bom: bool) -> &[u8] {
        let slice = &self.bytes[0..self.len];
        if bom || slice.len() <= 1 {
            slice
        } else if &slice[0..2] == b"\xFF\xFE" || &slice[0..2] == b"\xFE\xFF" {
            &slice[2..]
        } else if slice == b"\xEF\xBB\xBF" {
            &[]
        } else {
            slice
        }
    }

    /// If this is a valid UTF-8 or UTF-16 BOM, return its corresponding
    /// encoding. Otherwise, return `None`.
    pub fn encoding(&self) -> Option<&'static Encoding> {
        let bom = self.as_slice(true);
        if bom.len() < 3 {
            return None;
        }
        if let Some((enc, _)) = Encoding::for_bom(bom) {
            return Some(enc);
        }
        None
    }
}

/// Like `io::Read::read_exact`, except it never returns `UnexpectedEof` and
/// instead returns the number of bytes read if EOF is seen before filling
/// `buf`.
pub fn read_full<R: io::Read>(
    mut rdr: R,
    mut buf: &mut [u8],
) -> io::Result<usize> {
    let mut nread = 0;
    while !buf.is_empty() {
        match rdr.read(buf) {
            Ok(0) => break,
            Ok(n) => {
                nread += n;
                let tmp = buf;
                buf = &mut tmp[n..];
            }
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(nread)
}

#[cfg(test)]
mod tests {
    use super::{BomPeeker, PossibleBom, TinyTranscoder};
    use encoding_rs::Encoding;
    use std::io::Read;

    #[test]
    fn tiny_utf16_normal() {
        let enc = Encoding::for_label(b"utf-16le").unwrap();
        let mut dec = enc.new_decoder_with_bom_removal();
        let mut bytes = &b"f\x00o\x00o\x00b\x00a\x00r\x00b\x00a\x00z\x00"[..];
        let mut tiny = TinyTranscoder::new();
        let mut tmp = [0u8; 1];

        let (nin, nout) = tiny.transcode(&mut dec, bytes, false);
        assert_eq!(nin, 14);
        assert_eq!(nout, 7);
        bytes = &bytes[nin..];

        assert_eq!(tiny.read(&mut tmp).unwrap(), 1);
        assert_eq!(tmp, [b'f'; 1]);
        assert_eq!(tiny.read(&mut tmp).unwrap(), 1);
        assert_eq!(tmp, [b'o'; 1]);
        assert_eq!(tiny.read(&mut tmp).unwrap(), 1);
        assert_eq!(tmp, [b'o'; 1]);
        assert_eq!(tiny.read(&mut tmp).unwrap(), 1);
        assert_eq!(tmp, [b'b'; 1]);
        assert_eq!(tiny.read(&mut tmp).unwrap(), 1);
        assert_eq!(tmp, [b'a'; 1]);
        assert_eq!(tiny.read(&mut tmp).unwrap(), 1);
        assert_eq!(tmp, [b'r'; 1]);
        assert_eq!(tiny.read(&mut tmp).unwrap(), 1);
        assert_eq!(tmp, [b'b'; 1]);

        let (nin, nout) = tiny.transcode(&mut dec, bytes, false);
        assert_eq!(nin, 4);
        assert_eq!(nout, 2);
        bytes = &bytes[nin..];

        assert_eq!(tiny.read(&mut tmp).unwrap(), 1);
        assert_eq!(tmp, [b'a'; 1]);
        assert_eq!(tiny.read(&mut tmp).unwrap(), 1);
        assert_eq!(tmp, [b'z'; 1]);

        let (nin, nout) = tiny.transcode(&mut dec, bytes, true);
        assert_eq!(nin, 0);
        assert_eq!(nout, 0);

        assert_eq!(tiny.read(&mut tmp).unwrap(), 0);
    }

    #[test]
    fn tiny_utf16_invalid() {
        let enc = Encoding::for_label(b"utf-16le").unwrap();
        let mut dec = enc.new_decoder_with_bom_removal();
        let mut bytes = &b"\x00"[..];
        let mut tiny = TinyTranscoder::new();
        let mut tmp = [0u8; 1];

        let (nin, nout) = tiny.transcode(&mut dec, bytes, false);
        assert_eq!(nin, 1);
        assert_eq!(nout, 0);
        assert_eq!(tiny.read(&mut tmp).unwrap(), 0);
        bytes = &bytes[nin..];

        let (nin, nout) = tiny.transcode(&mut dec, bytes, true);
        assert_eq!(nin, 0);
        assert_eq!(nout, 3);

        assert_eq!(tiny.read(&mut tmp).unwrap(), 1);
        assert_eq!(tmp, [b'\xEF'; 1]);
        assert_eq!(tiny.read(&mut tmp).unwrap(), 1);
        assert_eq!(tmp, [b'\xBF'; 1]);
        assert_eq!(tiny.read(&mut tmp).unwrap(), 1);
        assert_eq!(tmp, [b'\xBD'; 1]);
        assert_eq!(tiny.read(&mut tmp).unwrap(), 0);
    }

    #[test]
    fn peeker_empty() {
        let buf = [];
        let mut peeker = BomPeeker::with_bom(&buf[..]);
        assert_eq!(PossibleBom::new(), peeker.peek_bom().unwrap());

        let mut tmp = [0; 100];
        assert_eq!(0, peeker.read(&mut tmp).unwrap());
    }

    #[test]
    fn peeker_one() {
        let buf = [1];
        let mut peeker = BomPeeker::with_bom(&buf[..]);
        assert_eq!(
            PossibleBom { bytes: [1, 0, 0], len: 1 },
            peeker.peek_bom().unwrap()
        );

        let mut tmp = [0; 100];
        assert_eq!(1, peeker.read(&mut tmp).unwrap());
        assert_eq!(1, tmp[0]);
        assert_eq!(0, peeker.read(&mut tmp).unwrap());
    }

    #[test]
    fn peeker_two() {
        let buf = [1, 2];
        let mut peeker = BomPeeker::with_bom(&buf[..]);
        assert_eq!(
            PossibleBom { bytes: [1, 2, 0], len: 2 },
            peeker.peek_bom().unwrap()
        );

        let mut tmp = [0; 100];
        assert_eq!(2, peeker.read(&mut tmp).unwrap());
        assert_eq!(1, tmp[0]);
        assert_eq!(2, tmp[1]);
        assert_eq!(0, peeker.read(&mut tmp).unwrap());
    }

    #[test]
    fn peeker_three() {
        let buf = [1, 2, 3];
        let mut peeker = BomPeeker::with_bom(&buf[..]);
        assert_eq!(
            PossibleBom { bytes: [1, 2, 3], len: 3 },
            peeker.peek_bom().unwrap()
        );

        let mut tmp = [0; 100];
        assert_eq!(3, peeker.read(&mut tmp).unwrap());
        assert_eq!(1, tmp[0]);
        assert_eq!(2, tmp[1]);
        assert_eq!(3, tmp[2]);
        assert_eq!(0, peeker.read(&mut tmp).unwrap());
    }

    #[test]
    fn peeker_four() {
        let buf = [1, 2, 3, 4];
        let mut peeker = BomPeeker::with_bom(&buf[..]);
        assert_eq!(
            PossibleBom { bytes: [1, 2, 3], len: 3 },
            peeker.peek_bom().unwrap()
        );

        let mut tmp = [0; 100];
        assert_eq!(3, peeker.read(&mut tmp).unwrap());
        assert_eq!(1, tmp[0]);
        assert_eq!(2, tmp[1]);
        assert_eq!(3, tmp[2]);
        assert_eq!(1, peeker.read(&mut tmp).unwrap());
        assert_eq!(4, tmp[0]);
        assert_eq!(0, peeker.read(&mut tmp).unwrap());
    }

    #[test]
    fn peeker_one_at_a_time() {
        let buf = [1, 2, 3, 4];
        let mut peeker = BomPeeker::with_bom(&buf[..]);

        let mut tmp = [0; 1];
        assert_eq!(0, peeker.read(&mut tmp[..0]).unwrap());
        assert_eq!(0, tmp[0]);
        assert_eq!(1, peeker.read(&mut tmp).unwrap());
        assert_eq!(1, tmp[0]);
        assert_eq!(1, peeker.read(&mut tmp).unwrap());
        assert_eq!(2, tmp[0]);
        assert_eq!(1, peeker.read(&mut tmp).unwrap());
        assert_eq!(3, tmp[0]);
        assert_eq!(1, peeker.read(&mut tmp).unwrap());
        assert_eq!(4, tmp[0]);
    }

    #[test]
    fn peeker_without_bom() {
        let buf = [b'\xEF', b'\xBB', b'\xBF', b'a'];
        let mut peeker = BomPeeker::without_bom(&buf[..]);
        assert_eq!(
            PossibleBom { bytes: [b'\xEF', b'\xBB', b'\xBF'], len: 3 },
            peeker.peek_bom().unwrap()
        );

        let mut tmp = [0; 100];
        assert_eq!(1, peeker.read(&mut tmp).unwrap());
        assert_eq!(b'a', tmp[0]);
        assert_eq!(0, peeker.read(&mut tmp).unwrap());
    }

    #[test]
    fn peeker_without_bom_nobom() {
        let buf = [1, 2, 3, 4];
        let mut peeker = BomPeeker::without_bom(&buf[..]);
        assert_eq!(
            PossibleBom { bytes: [1, 2, 3], len: 3 },
            peeker.peek_bom().unwrap()
        );

        let mut tmp = [0; 100];
        assert_eq!(3, peeker.read(&mut tmp).unwrap());
        assert_eq!(1, tmp[0]);
        assert_eq!(2, tmp[1]);
        assert_eq!(3, tmp[2]);
        assert_eq!(1, peeker.read(&mut tmp).unwrap());
        assert_eq!(4, tmp[0]);
        assert_eq!(0, peeker.read(&mut tmp).unwrap());
    }
}
