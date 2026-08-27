#![deny(unsafe_code)]
use std::convert::Infallible;
use std::fmt;
use std::io::{self, Read};
use std::iter::Map;
use std::ops::Not;

#[cfg(feature = "debug")]
macro_rules! debug {
    ($($arg:expr),*) => (
        println!($($arg),*)
    )
}
#[cfg(not(feature = "debug"))]
macro_rules! debug {
    ($($arg:expr),*) => {
        ()
    };
}

pub mod maps;

/// Decoder module
pub mod decoder;

/// Encoder module
pub mod encoder;

/// TIFF helper functions
pub mod tiff;

/// Trait used to read data bitwise.
///
/// For lazy people `ByteReader` is provided which implements this trait.
pub trait BitReader {
    type Error;

    /// look at the next (up to 16) bits of data
    ///
    /// Data is returned in the lower bits of the `u16`.
    fn peek(&self, bits: u8) -> Option<u16>;

    /// Consume the given amount of bits from the input.
    fn consume(&mut self, bits: u8) -> Result<(), Self::Error>;

    /// Assert that the next bits matches the given pattern.
    ///
    /// If it does not match, the found pattern is returned if enough bits are aviable.
    /// Otherwise None is returned.
    fn expect(&mut self, bits: Bits) -> Result<(), Option<Bits>> {
        match self.peek(bits.len) {
            None => Err(None),
            Some(val) if val == bits.data => Ok(()),
            Some(val) => Err(Some(Bits {
                data: val,
                len: bits.len,
            })),
        }
    }

    fn bits_to_byte_boundary(&self) -> u8;
}

/// Trait to write data bitwise
///
/// The `VecWriter` struct is provided for convinience.
pub trait BitWriter {
    type Error;
    fn write(&mut self, bits: Bits) -> Result<(), Self::Error>;
}
pub struct VecWriter {
    data: Vec<u8>,
    partial: u32,
    len: u8,
}
impl BitWriter for VecWriter {
    type Error = Infallible;
    fn write(&mut self, bits: Bits) -> Result<(), Self::Error> {
        self.partial |= (bits.data as u32) << (32 - self.len - bits.len);
        self.len += bits.len;
        while self.len >= 8 {
            self.data.push((self.partial >> 24) as u8);
            self.partial <<= 8;
            self.len -= 8;
        }
        Ok(())
    }
}
impl VecWriter {
    pub fn new() -> Self {
        VecWriter {
            data: Vec::new(),
            partial: 0,
            len: 0,
        }
    }
    // with capacity of `n` bits.
    pub fn with_capacity(n: usize) -> Self {
        VecWriter {
            data: Vec::with_capacity((n + 7) / 8),
            partial: 0,
            len: 0,
        }
    }

    /// Pad the output with `0` bits until it is at a byte boundary.
    pub fn pad(&mut self) {
        if self.len > 0 {
            self.data.push((self.partial >> 24) as u8);
            self.partial = 0;
            self.len = 0;
        }
    }

    /// pad and return the accumulated bytes
    pub fn finish(mut self) -> Vec<u8> {
        self.pad();
        self.data
    }
}

pub struct ByteReader<R> {
    read: R,
    partial: u32,
    valid: u8,
}
impl<E, R: Iterator<Item = Result<u8, E>>> ByteReader<R> {
    /// Construct a new `ByteReader` from an iterator of `u8`
    pub fn new(read: R) -> Result<Self, E> {
        let mut bits = ByteReader {
            read,
            partial: 0,
            valid: 0,
        };
        bits.fill()?;
        Ok(bits)
    }
    fn fill(&mut self) -> Result<(), E> {
        while self.valid < 16 {
            match self.read.next() {
                Some(Ok(byte)) => {
                    self.partial = self.partial << 8 | byte as u32;
                    self.valid += 8;
                }
                Some(Err(e)) => return Err(e),
                None => break,
            }
        }
        Ok(())
    }
    /// Print the remaining data
    ///
    /// Note: For debug purposes only, not part of the API.
    pub fn print_remaining(&mut self) {
        println!(
            "partial: {:0w$b}, valid: {}",
            self.partial & ((1 << self.valid) - 1),
            self.valid,
            w = self.valid as usize
        );
        while let Some(Ok(b)) = self.read.next() {
            print!("{:08b} ", b);
        }
        println!();
    }
    pub fn print_peek(&self) {
        println!(
            "partial: {:0w$b}, valid: {}",
            self.partial & ((1 << self.valid) - 1),
            self.valid,
            w = self.valid as usize
        );
    }
}

pub fn slice_reader(slice: &[u8]) -> ByteReader<impl Iterator<Item = Result<u8, Infallible>> + '_> {
    ByteReader::new(slice.iter().cloned().map(Ok)).unwrap()
}
pub fn slice_bits(slice: &[u8]) -> impl Iterator<Item = bool> + '_ {
    slice
        .iter()
        .flat_map(|&b| [7, 6, 5, 4, 3, 2, 1, 0].map(|i| (b >> i) & 1 != 0))
}

impl<E, R: Iterator<Item = Result<u8, E>>> BitReader for ByteReader<R> {
    type Error = E;

    fn peek(&self, bits: u8) -> Option<u16> {
        if bits > 16 {
            return None;
        }
        if self.valid >= bits {
            let shift = self.valid - bits;
            let mask = if bits >= 16 {
                u16::MAX
            } else {
                (1u16 << bits) - 1
            };
            let out = (self.partial >> shift) as u16 & mask;
            Some(out)
        } else {
            None
        }
    }
    fn consume(&mut self, bits: u8) -> Result<(), E> {
        self.valid = self.valid.saturating_sub(bits);
        self.fill()
    }
    fn bits_to_byte_boundary(&self) -> u8 {
        self.valid & 7
    }
}

#[test]
fn test_bits() {
    let mut bits = slice_reader(&[0b0000_1101, 0b1010_0000]);
    assert_eq!(maps::black::decode(&mut bits), Some(42));
}

#[test]
fn test_peek_over_16_returns_none() {
    let bits = slice_reader(&[0xFF, 0xFF, 0xFF]);
    // peek(17) should return None, not panic
    assert_eq!(bits.peek(17), None);
    assert_eq!(bits.peek(255), None);
    // peek(16) should still work
    assert!(bits.peek(16).is_some());
}

#[test]
fn test_consume_more_than_valid_saturates() {
    let mut bits = slice_reader(&[0xAB]);
    // consume more bits than available — should not panic
    let _ = bits.consume(200);
    // after saturating to 0, peek should return None for any nonzero request
    assert_eq!(bits.peek(1), None);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a Group 3 bitstream from a sequence of bits.
    fn bits_to_bytes(bits: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut byte = 0u8;
        let mut count = 0;
        for &b in bits {
            byte = (byte << 1) | (b & 1);
            count += 1;
            if count == 8 {
                bytes.push(byte);
                byte = 0;
                count = 0;
            }
        }
        if count > 0 {
            byte <<= 8 - count;
            bytes.push(byte);
        }
        bytes
    }

    #[test]
    fn test_group3_all_white_line() {
        // Build a minimal Group 3 stream:
        // - Initial EOL (000000000001)
        // - Line 1: white(8) = 10011, EOL
        // - RTC: 5 more EOLs
        let mut stream_bits = Vec::new();

        // Initial EOL
        let eol: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        stream_bits.extend_from_slice(eol);

        // Line 1: white run of 8 pixels = 10011
        stream_bits.extend_from_slice(&[1, 0, 0, 1, 1]);
        // EOL after line 1
        stream_bits.extend_from_slice(eol);

        // RTC: 5 more EOLs
        for _ in 0..5 {
            stream_bits.extend_from_slice(eol);
        }

        let data = bits_to_bytes(&stream_bits);
        let mut lines = Vec::new();
        decoder::decode_g3(data.into_iter(), |transitions| {
            lines.push(transitions.to_vec());
        });

        assert_eq!(lines.len(), 1, "expected 1 line, got {}", lines.len());
        // All-white line: single transition at position 8 (white→black at the end)
        // Actually, the run-length is 8 white pixels. The transitions list shows
        // color change positions. For an all-white line, there are no transitions
        // (white runs the full width). But the decoder adds a0 += p after each code,
        // and pushes a0. For white(8), a0 = 8, pushed once. That's one transition.
        assert_eq!(lines[0], vec![8]);
    }

    #[test]
    fn test_group3_mixed_line() {
        // Width 16: 4 white, 4 black, 8 white
        // white(4) = 1011, black(4) = 011, white(8) = 10011
        let mut stream_bits = Vec::new();

        let eol: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];

        // Initial EOL
        stream_bits.extend_from_slice(eol);

        // Line: white(4)=1011, black(4)=011, white(8)=10011
        stream_bits.extend_from_slice(&[1, 0, 1, 1]); // white 4
        stream_bits.extend_from_slice(&[0, 1, 1]); // black 4
        stream_bits.extend_from_slice(&[1, 0, 0, 1, 1]); // white 8
        stream_bits.extend_from_slice(eol);

        // RTC
        for _ in 0..5 {
            stream_bits.extend_from_slice(eol);
        }

        let data = bits_to_bytes(&stream_bits);
        let mut lines = Vec::new();
        decoder::decode_g3(data.into_iter(), |transitions| {
            lines.push(transitions.to_vec());
        });

        assert_eq!(lines.len(), 1);
        // Transitions: white(4) -> position 4, black(4) -> position 8, white(8) -> position 16
        assert_eq!(lines[0], vec![4, 8, 16]);
    }

    #[test]
    fn test_group3_with_fill_bits() {
        // T.4 allows 0-7 fill bits (zeros) before each EOL for byte
        // alignment. Test all fill counts to verify is_eol_ahead detects
        // fill+EOL without the prefix tree consuming fill bits.
        let eol: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];

        for fill_count in 0u8..=7 {
            let mut stream_bits = Vec::new();

            // Initial EOL with fill
            for _ in 0..fill_count {
                stream_bits.push(0);
            }
            stream_bits.extend_from_slice(eol);

            // Line: white(4) = 1011
            stream_bits.extend_from_slice(&[1, 0, 1, 1]);

            // EOL with fill
            for _ in 0..fill_count {
                stream_bits.push(0);
            }
            stream_bits.extend_from_slice(eol);

            // RTC: 5 more EOLs with fill
            for _ in 0..5 {
                for _ in 0..fill_count {
                    stream_bits.push(0);
                }
                stream_bits.extend_from_slice(eol);
            }

            let data = bits_to_bytes(&stream_bits);
            let mut lines = Vec::new();
            decoder::decode_g3(data.into_iter(), |transitions| {
                lines.push(transitions.to_vec());
            });

            assert_eq!(
                lines.len(),
                1,
                "fill={fill_count}: expected 1 line, got {}",
                lines.len()
            );
            assert_eq!(
                lines[0],
                vec![4],
                "fill={fill_count}: expected [4], got {:?}",
                lines[0]
            );
        }
    }

    #[test]
    fn test_group3_multiple_lines() {
        let mut stream_bits = Vec::new();
        let eol: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];

        // Initial EOL
        stream_bits.extend_from_slice(eol);

        // Line 1: white(4)=1011
        stream_bits.extend_from_slice(&[1, 0, 1, 1]);
        stream_bits.extend_from_slice(eol);

        // Line 2: white(8)=10011
        stream_bits.extend_from_slice(&[1, 0, 0, 1, 1]);
        stream_bits.extend_from_slice(eol);

        // Line 3: white(2)=0111, black(3)=10
        stream_bits.extend_from_slice(&[0, 1, 1, 1]); // white 2
        stream_bits.extend_from_slice(&[1, 0]); // black 3
        stream_bits.extend_from_slice(eol);

        // RTC
        for _ in 0..5 {
            stream_bits.extend_from_slice(eol);
        }

        let data = bits_to_bytes(&stream_bits);
        let mut lines = Vec::new();
        decoder::decode_g3(data.into_iter(), |transitions| {
            lines.push(transitions.to_vec());
        });

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], vec![4]);
        assert_eq!(lines[1], vec![8]);
        assert_eq!(lines[2], vec![2, 5]); // white 2, then black 3 = positions 2, 5
    }
}

/// Enum used to signal black/white.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Color {
    Black,
    White,
}
impl Not for Color {
    type Output = Self;
    fn not(self) -> Self {
        match self {
            Color::Black => Color::White,
            Color::White => Color::Black,
        }
    }
}

struct Transitions<'a> {
    edges: &'a [u16],
    pos: usize,
}
impl<'a> Transitions<'a> {
    fn new(edges: &'a [u16]) -> Self {
        Transitions { edges, pos: 0 }
    }
    fn seek_back(&mut self, start: u16) {
        self.pos = self.pos.min(self.edges.len().saturating_sub(1));
        while self.pos > 0 {
            if start < self.edges[self.pos - 1] {
                self.pos -= 1;
            } else {
                break;
            }
        }
    }
    fn next_color(&mut self, start: u16, color: Color, start_of_row: bool) -> Option<u16> {
        if start_of_row {
            if color == Color::Black {
                self.pos = 1;
                return self.edges.get(0).cloned();
            } else {
                self.pos = 2;
                return self.edges.get(1).cloned();
            }
        }
        while self.pos < self.edges.len() {
            if self.edges[self.pos] <= start {
                self.pos += 1;
                continue;
            }

            if (self.pos % 2 == 0) != (color == Color::Black) {
                self.pos += 1;
            }

            break;
        }
        if self.pos < self.edges.len() {
            let val = self.edges[self.pos];
            self.pos += 1;
            Some(val)
        } else {
            None
        }
    }
    fn next(&mut self) -> Option<u16> {
        if self.pos < self.edges.len() {
            let val = self.edges[self.pos];
            self.pos += 1;
            Some(val)
        } else {
            None
        }
    }
    fn peek(&self) -> Option<u16> {
        self.edges.get(self.pos).cloned()
    }
    fn skip(&mut self, n: usize) {
        self.pos += n;
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Bits {
    pub data: u16,
    pub len: u8,
}

impl fmt::Debug for Bits {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "d={:0b} w={}", self.data, self.len)
    }
}
impl fmt::Display for Bits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:0w$b}",
            self.data & ((1 << self.len) - 1),
            w = self.len as usize
        )
    }
}
