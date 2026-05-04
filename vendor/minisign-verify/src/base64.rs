#![forbid(unsafe_code)]

use core::fmt::{self, Display};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Error {
    /// The provided output buffer would be too small.
    Overflow,
    /// The input isn't valid for the given encoding.
    InvalidInput,
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Overflow => write!(f, "Overflow"),
            Error::InvalidInput => write!(f, "Invalid input"),
        }
    }
}

pub trait Decoder {
    /// Decode `encoded` into `bin`.
    /// The output buffer can be larger than required; the returned slice is
    /// a view of the buffer with the correct length.
    fn decode<IN: AsRef<[u8]>>(bin: &mut [u8], encoded: IN) -> Result<&[u8], Error>;

    /// Decode `encoded` into a `Vec<u8>`.
    fn decode_to_vec<IN: AsRef<[u8]>>(encoded: IN) -> Result<Vec<u8>, Error> {
        let mut bin = vec![0u8; encoded.as_ref().len()];
        let bin_len = Self::decode(&mut bin, encoded)?.len();
        bin.truncate(bin_len);
        Ok(bin)
    }
}

struct Base64Impl;

impl Base64Impl {
    #[inline]
    fn _eq(x: u8, y: u8) -> u8 {
        !(((0u16.wrapping_sub((x as u16) ^ (y as u16))) >> 8) as u8)
    }

    #[inline]
    fn _gt(x: u8, y: u8) -> u8 {
        (((y as u16).wrapping_sub(x as u16)) >> 8) as u8
    }

    #[inline]
    fn _ge(x: u8, y: u8) -> u8 {
        !Self::_gt(y, x)
    }

    #[inline]
    fn _lt(x: u8, y: u8) -> u8 {
        Self::_gt(y, x)
    }

    #[inline]
    fn _le(x: u8, y: u8) -> u8 {
        Self::_ge(y, x)
    }

    #[inline]
    fn b64_char_to_byte(c: u8) -> u8 {
        let x = (Self::_ge(c, b'A') & Self::_le(c, b'Z') & (c.wrapping_sub(b'A')))
            | (Self::_ge(c, b'a') & Self::_le(c, b'z') & (c.wrapping_sub(b'a'.wrapping_sub(26))))
            | (Self::_ge(c, b'0') & Self::_le(c, b'9') & (c.wrapping_sub(b'0'.wrapping_sub(52))))
            | (Self::_eq(c, b'+') & 62)
            | (Self::_eq(c, b'/') & 63);
        x | (Self::_eq(x, 0) & (Self::_eq(c, b'A') ^ 0xff))
    }

    fn skip_padding(b64: &[u8], mut padding_len: usize) -> Result<&[u8], Error> {
        let b64_len = b64.len();
        let mut b64_pos = 0usize;
        while padding_len > 0 {
            if b64_pos >= b64_len {
                return Err(Error::InvalidInput);
            }
            let c = b64[b64_pos];
            if c == b'=' {
                padding_len -= 1
            } else {
                return Err(Error::InvalidInput);
            }
            b64_pos += 1
        }
        Ok(&b64[b64_pos..])
    }

    pub fn decode<'t>(bin: &'t mut [u8], b64: &[u8]) -> Result<&'t [u8], Error> {
        let bin_maxlen = bin.len();
        let mut acc = 0u16;
        let mut acc_len = 0usize;
        let mut bin_pos = 0usize;
        let mut premature_end = None;
        for (b64_pos, &c) in b64.iter().enumerate() {
            let d = Self::b64_char_to_byte(c);
            if d == 0xff {
                premature_end = Some(b64_pos);
                break;
            }
            acc = (acc << 6) + d as u16;
            acc_len += 6;
            if acc_len >= 8 {
                acc_len -= 8;
                if bin_pos >= bin_maxlen {
                    return Err(Error::Overflow);
                }
                bin[bin_pos] = (acc >> acc_len) as u8;
                bin_pos += 1;
            }
        }
        if acc_len > 4 || (acc & ((1u16 << acc_len).wrapping_sub(1))) != 0 {
            return Err(Error::InvalidInput);
        }
        let padding_len = acc_len / 2;
        if let Some(premature_end) = premature_end {
            let remaining = Self::skip_padding(&b64[premature_end..], padding_len)?;
            if !remaining.is_empty() {
                return Err(Error::InvalidInput);
            }
        } else if padding_len != 0 {
            return Err(Error::InvalidInput);
        }
        Ok(&bin[..bin_pos])
    }
}

pub struct Base64;

impl Decoder for Base64 {
    #[inline]
    fn decode<IN: AsRef<[u8]>>(bin: &mut [u8], b64: IN) -> Result<&[u8], Error> {
        Base64Impl::decode(bin, b64.as_ref())
    }
}

#[test]
fn test_base64_mising_padding() {
    let missing_padding = "AA";
    assert!(Base64::decode_to_vec(missing_padding).is_err());
    let missing_padding = "AAA";
    assert!(Base64::decode_to_vec(missing_padding).is_err());
}

#[test]
fn test_base64_invalid_padding() {
    let valid_padding = "AA==";
    assert_eq!(Base64::decode_to_vec(valid_padding), Ok(vec![0u8; 1]));
    let invalid_padding = "AA=";
    assert_eq!(
        Base64::decode_to_vec(invalid_padding),
        Err(Error::InvalidInput)
    );
}
