/*!
`base62` is a `no_std` crate that provides base62 encoding and decoding functionality.

With the `alloc` feature (enabled by default), it provides functions for converting
between `u128` values and base62 strings using both standard (0-9, A-Z, a-z) and
alternative (0-9, a-z, A-Z) alphabets.

When used without default features, it provides the core encoding and decoding
primitives that operate on byte slices.

For usage with the standard library's IO traits, enable the `std` feature.

[![Build status](https://github.com/fbernier/base62/workflows/ci/badge.svg)](https://github.com/fbernier/base62/actions)
[![Crates.io](https://img.shields.io/crates/v/base62.svg)](https://crates.io/crates/base62)
[![Docs](https://docs.rs/base62/badge.svg)](https://docs.rs/base62)
*/

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use core::{convert::TryInto, fmt};

const BASE: u64 = 62;
const BASE_TO_2: u64 = BASE * BASE;
const BASE_TO_3: u64 = BASE_TO_2 * BASE;
const BASE_TO_4: u64 = BASE_TO_3 * BASE;
const BASE_TO_5: u64 = BASE_TO_4 * BASE;
const BASE_TO_6: u64 = BASE_TO_5 * BASE;
const BASE_TO_7: u64 = BASE_TO_6 * BASE;
const BASE_TO_8: u64 = BASE_TO_7 * BASE;
const BASE_TO_9: u64 = BASE_TO_8 * BASE;
const BASE_TO_10: u128 = (BASE_TO_9 * BASE) as u128;
const BASE_TO_11: u128 = BASE_TO_10 * BASE as u128;
const BASE_TO_12: u128 = BASE_TO_11 * BASE as u128;
const BASE_TO_13: u128 = BASE_TO_12 * BASE as u128;
const BASE_TO_14: u128 = BASE_TO_13 * BASE as u128;
const BASE_TO_15: u128 = BASE_TO_14 * BASE as u128;
const BASE_TO_16: u128 = BASE_TO_15 * BASE as u128;
const BASE_TO_17: u128 = BASE_TO_16 * BASE as u128;
const BASE_TO_18: u128 = BASE_TO_17 * BASE as u128;
const BASE_TO_19: u128 = BASE_TO_18 * BASE as u128;
const BASE_TO_20: u128 = BASE_TO_19 * BASE as u128;
const BASE_TO_21: u128 = BASE_TO_20 * BASE as u128;

// Rust does not apply strength reduction to integer division by u128s, instead it uses the very slow __udivti3 intrinsic.
// This multiply and shift is equivalent to dividing by BASE_TO_10.
const DIV_BASE_TO_10_MULTIPLY: u128 = 233718071534448225491982379416108680074;
const DIV_BASE_TO_10_SHIFT: u8 = 59;

#[repr(align(64))]
struct StandardTables {
    encode: [u8; 62],
    decode: [u8; 128],
}

#[repr(align(64))]
struct AlternativeTables {
    encode: [u8; 62],
    decode: [u8; 128],
}

impl StandardTables {
    const fn new() -> Self {
        // Standard encoding table (0-9A-Za-z)
        const ENCODE: [u8; 62] = [
            b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'A', b'B', b'C', b'D',
            b'E', b'F', b'G', b'H', b'I', b'J', b'K', b'L', b'M', b'N', b'O', b'P', b'Q', b'R',
            b'S', b'T', b'U', b'V', b'W', b'X', b'Y', b'Z', b'a', b'b', b'c', b'd', b'e', b'f',
            b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't',
            b'u', b'v', b'w', b'x', b'y', b'z',
        ];

        let mut decode = [255u8; 128];

        // Populate decode table
        let mut i = 0u8;
        while i < 10 {
            decode[(b'0' + i) as usize] = i;
            i += 1;
        }
        let mut i = 0u8;
        while i < 26 {
            decode[(b'A' + i) as usize] = i + 10;
            i += 1;
        }
        let mut i = 0u8;
        while i < 26 {
            decode[(b'a' + i) as usize] = i + 36;
            i += 1;
        }

        Self {
            encode: ENCODE,
            decode,
        }
    }
}

impl AlternativeTables {
    const fn new() -> Self {
        // Alternative encoding table (0-9a-zA-Z)
        const ENCODE: [u8; 62] = [
            b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'a', b'b', b'c', b'd',
            b'e', b'f', b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r',
            b's', b't', b'u', b'v', b'w', b'x', b'y', b'z', b'A', b'B', b'C', b'D', b'E', b'F',
            b'G', b'H', b'I', b'J', b'K', b'L', b'M', b'N', b'O', b'P', b'Q', b'R', b'S', b'T',
            b'U', b'V', b'W', b'X', b'Y', b'Z',
        ];

        let mut decode = [255u8; 128];

        // Populate decode table
        let mut i = 0u8;
        while i < 10 {
            decode[(b'0' + i) as usize] = i;
            i += 1;
        }
        let mut i = 0u8;
        while i < 26 {
            decode[(b'a' + i) as usize] = i + 10;
            i += 1;
        }
        let mut i = 0u8;
        while i < 26 {
            decode[(b'A' + i) as usize] = i + 36;
            i += 1;
        }

        Self {
            encode: ENCODE,
            decode,
        }
    }
}

static STANDARD_TABLES: StandardTables = StandardTables::new();
static ALTERNATIVE_TABLES: AlternativeTables = AlternativeTables::new();

/// Indicates the cause of a decoding failure in base62 decoding operations.
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodeError {
    /// The decoded number cannot fit into a [`u128`].
    ArithmeticOverflow,
    /// The encoded input is an empty string.
    EmptyInput,
    /// The encoded input contains the given invalid byte at the given index.
    InvalidBase62Byte(u8, usize),
}

/// Indicates the cause of an encoding failure in encoding operations.
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EncodeError {
    BufferTooSmall,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            DecodeError::ArithmeticOverflow => {
                f.write_str("Decoded number cannot fit into a `u128`")
            }
            DecodeError::EmptyInput => f.write_str("Encoded input is an empty string"),
            DecodeError::InvalidBase62Byte(ch, idx) => {
                use fmt::Write;
                f.write_str("Encoded input contains the invalid byte b'")?;
                for char_in_escape in core::ascii::escape_default(ch) {
                    f.write_char(char::from(char_in_escape))?;
                }
                write!(f, "' at index {idx}")
            }
        }
    }
}

// For DecodeError
#[cfg(not(feature = "std"))]
impl core::error::Error for DecodeError {}

#[cfg(feature = "std")]
impl std::error::Error for DecodeError {}

// For EncodeError
#[cfg(not(feature = "std"))]
impl core::error::Error for EncodeError {}

#[cfg(feature = "std")]
impl std::error::Error for EncodeError {}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncodeError::BufferTooSmall => write!(f, "Buffer too small to encode the number"),
        }
    }
}

struct Fmt(u128);

impl fmt::Display for Fmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; 22];
        let digits = digit_count(self.0);
        unsafe {
            let len = _encode_buf(self.0, digits, &mut buf[..digits]);
            // Use pad() to handle formatting specifiers
            let s = core::str::from_utf8_unchecked(&buf[..len]);
            f.pad(s)
        }
    }
}

/// Writes the base62 representation of a number using the standard alphabet to any fmt::Write destination.
///
/// # Example
/// ```rust
/// use core::fmt::Write;
///
/// let mut output = String::new();
/// write!(output, "{}", base62::encode_fmt(1337_u32));
/// assert_eq!(output, "LZ");
/// ```
pub fn encode_fmt<T: Into<u128>>(num: T) -> impl fmt::Display {
    Fmt(num.into())
}

struct FmtAlternative(u128);

impl fmt::Display for FmtAlternative {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; 22];
        let digits = digit_count(self.0);
        unsafe {
            let len = _encode_alternative_buf(self.0, digits, &mut buf[..digits]);
            // Use pad() to handle formatting specifiers
            let s = core::str::from_utf8_unchecked(&buf[..len]);
            f.pad(s)
        }
    }
}

/// Writes the base62 representation of a number using the alternative alphabet to any fmt::Write destination.
///
/// # Example
/// ```rust
/// use core::fmt::Write;
///
/// let mut output = String::new();
/// write!(output, "{}", base62::encode_fmt_alt(1337_u32));
/// assert_eq!(output, "lz");
/// ```
pub fn encode_fmt_alt<T: Into<u128>>(num: T) -> impl fmt::Display {
    FmtAlternative(num.into())
}

/// Writes the base62 representation of a number using the standard alphabet to any io::Write destination.
///
/// This function is only available when the `std` feature is enabled.
///
/// # Example
/// ```rust
/// # use std::io::Write;
/// let mut output = Vec::new();
/// base62::encode_io(1337_u32, &mut output).unwrap();
/// assert_eq!(output, b"LZ");
/// ```
#[cfg(feature = "std")]
pub fn encode_io<T: Into<u128>, W: std::io::Write + ?Sized>(
    num: T,
    w: &mut W,
) -> std::io::Result<()> {
    let num = num.into();
    let digits = digit_count(num);

    let mut buf = core::mem::MaybeUninit::<[u8; 22]>::uninit();

    unsafe {
        let ptr = buf.as_mut_ptr() as *mut u8;
        let slice = core::slice::from_raw_parts_mut(ptr, digits);

        let len = _encode_buf(num, digits, slice);
        debug_assert_eq!(len, digits);

        w.write_all(&slice[..len])
    }
}

/// Writes the base62 representation of a number using the alternative alphabet (0 to 9, then a to z, then A to Z)
/// to any io::Write destination.
///
/// This function is only available when the `std` feature is enabled.
///
/// # Example
/// ```rust
/// # use std::io::Write;
/// let mut output = Vec::new();
/// base62::encode_alternative_io(1337_u32, &mut output).unwrap();
/// assert_eq!(output, b"lz");
/// ```
#[cfg(feature = "std")]
pub fn encode_alternative_io<T: Into<u128>, W: std::io::Write + ?Sized>(
    num: T,
    w: &mut W,
) -> std::io::Result<()> {
    let num = num.into();
    let digits = digit_count(num);

    let mut buf = core::mem::MaybeUninit::<[u8; 22]>::uninit();

    unsafe {
        let ptr = buf.as_mut_ptr() as *mut u8;
        let slice = core::slice::from_raw_parts_mut(ptr, digits);

        let len = _encode_alternative_buf(num, digits, slice);
        debug_assert_eq!(len, digits);

        w.write_all(&slice[..len])
    }
}

// Internal functions used by both no_std and alloc features
pub(crate) fn digit_count(n: u128) -> usize {
    const POWERS: [u128; 22] = [
        0,
        BASE as u128,
        BASE_TO_2 as u128,
        BASE_TO_3 as u128,
        BASE_TO_4 as u128,
        BASE_TO_5 as u128,
        BASE_TO_6 as u128,
        BASE_TO_7 as u128,
        BASE_TO_8 as u128,
        BASE_TO_9 as u128,
        BASE_TO_10,
        BASE_TO_11,
        BASE_TO_12,
        BASE_TO_13,
        BASE_TO_14,
        BASE_TO_15,
        BASE_TO_16,
        BASE_TO_17,
        BASE_TO_18,
        BASE_TO_19,
        BASE_TO_20,
        BASE_TO_21,
    ];

    match POWERS.binary_search(&n) {
        Ok(n) => n.wrapping_add(1),
        Err(n) => n,
    }
}

#[inline(always)]
fn decode_char(
    result: &mut u64,
    ch: u8,
    i: usize,
    decode_table: &[u8; 128],
) -> Result<(), DecodeError> {
    if ch < 128 {
        let char_value = decode_table[ch as usize];

        // avoid branching
        let is_valid = (char_value != 255) as u64;
        *result = result
            .wrapping_mul(BASE)
            .wrapping_add((char_value as u64) * is_valid);

        if char_value == 255 {
            Err(DecodeError::InvalidBase62Byte(ch, i))
        } else {
            Ok(())
        }
    } else {
        // non-ASCII character - always invalid
        Err(DecodeError::InvalidBase62Byte(ch, i))
    }
}

// Common decoding function
#[inline]
fn decode_impl(mut input: &[u8], decode_table: &[u8; 128]) -> Result<u128, DecodeError> {
    if input.is_empty() {
        return Err(DecodeError::EmptyInput);
    }

    // Remove leading zeroes
    let chopped_count = input.iter().take_while(|&&ch| ch == b'0').count();
    input = &input[chopped_count..];

    let input_len = input.len();
    if input_len <= 22 {
        const MULTIPLIERS: [(u128, u64); 23] = [
            (0, 0),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (BASE as u128, 1),
            (BASE_TO_2 as u128, 1),
            (BASE_TO_3 as u128, 1),
            (BASE_TO_4 as u128, 1),
            (BASE_TO_5 as u128, 1),
            (BASE_TO_6 as u128, 1),
            (BASE_TO_7 as u128, 1),
            (BASE_TO_8 as u128, 1),
            (BASE_TO_9 as u128, 1),
            (BASE_TO_10, 1),
            (BASE_TO_11, BASE),
            (BASE_TO_12, BASE_TO_2),
        ];

        let (a_power, b_power) = MULTIPLIERS[input_len];

        let mut iter = (chopped_count..).zip(input.iter().copied());

        // process first 10 characters
        let mut result_a = 0_u64;
        for (i, ch) in iter.by_ref().take(10) {
            decode_char(&mut result_a, ch, i, decode_table)?;
        }
        let result_a = (result_a as u128)
            .checked_mul(a_power)
            .ok_or(DecodeError::ArithmeticOverflow)?;

        // process next 10 characters
        let mut result_b = 0_u64;
        for (i, ch) in iter.by_ref().take(10) {
            decode_char(&mut result_b, ch, i, decode_table)?;
        }
        let result_b = (result_b as u128).wrapping_mul(b_power as u128);

        // process remaining characters
        let mut result_c = 0_u64;
        for (i, ch) in iter {
            decode_char(&mut result_c, ch, i, decode_table)?;
        }
        let result_c = result_c as u128;

        let result = result_a
            .checked_add(result_b.wrapping_add(result_c))
            .ok_or(DecodeError::ArithmeticOverflow)?;
        Ok(result)
    } else {
        Err(DecodeError::ArithmeticOverflow)
    }
}

/// Encodes an unsigned integer into base62, using the standard digit ordering
/// (0 to 9, then A to Z, then a to z), and writes it to the passed buffer.
///
/// The return value is the number of bytes written to the buffer.
///
/// # Safety
/// - To encode all possible values of u128, the buffer must be at least 22 bytes long. However
///   a smaller buffer may be used if the value to be encoded is known to be smaller.
///   Base62 encoding adds 37.5% overhead to the size of the input.
/// - The remaining end of the buffer is left untouched. It's up to the caller to zero it using
///   the returned len if they want to.
///
/// # Example
///
/// ```rust
/// extern crate base62;
///
/// let mut buf = [0; 22];
/// base62::encode_bytes(1337_u32, &mut buf);
/// assert_eq!(&buf[..2], b"LZ");
/// ```
pub fn encode_bytes<T: Into<u128>>(num: T, buf: &mut [u8]) -> Result<usize, EncodeError> {
    let num = num.into();
    let digits = digit_count(num);

    if buf.len() < digits {
        return Err(EncodeError::BufferTooSmall);
    }

    unsafe {
        let len = _encode_buf(num, digits, &mut buf[..digits]);
        debug_assert_eq!(len, digits);
    }

    Ok(digits)
}

/// Encodes an unsigned integer into base62, using the alternative digit ordering
/// (0 to 9, then a to z, then A to Z), and writes it to the passed buffer.
///
/// The return value is the number of bytes written to the buffer.
///
/// # Safety
/// - To encode all possible values of u128, the buffer must be at least 22 bytes long. However
///   a smaller buffer may be used if the value to be encoded is known to be smaller.
///   Base62 encoding adds 37.5% overhead to the size of the input.
/// - The remaining end of the buffer is left untouched. It's up to the caller to zero it using
///   the returned len if they want to.
///
/// # Example
///
/// ```rust
/// extern crate base62;
///
/// let mut buf = [0; 22];
/// base62::encode_bytes(1337_u32, &mut buf);
/// assert_eq!(&buf[..2], b"LZ");
/// ```
pub fn encode_alternative_bytes<T: Into<u128>>(
    num: T,
    buf: &mut [u8],
) -> Result<usize, EncodeError> {
    let num = num.into();
    let digits = digit_count(num);

    if buf.len() < digits {
        return Err(EncodeError::BufferTooSmall);
    }

    unsafe {
        let len = _encode_alternative_buf(num, digits, &mut buf[..digits]);
        debug_assert_eq!(len, digits);
    }

    Ok(digits)
}

/// Decodes a base62 byte slice or an equivalent, like a `String`,
/// using the standard digit ordering (0 to 9, then A to Z, then a to z).
///
/// Returns a [`Result`] containing the decoded
/// [`u128`] or a [`DecodeError`].
///
/// # Examples
///
/// ```rust
/// extern crate base62;
///
/// let value = base62::decode("rustlang").unwrap();
/// assert_eq!(value, 189876682536016);
/// ```
pub fn decode<T: AsRef<[u8]>>(input: T) -> Result<u128, DecodeError> {
    decode_impl(input.as_ref(), &STANDARD_TABLES.decode)
}

/// Decodes a base62 byte slice or an equivalent, like a `String`,
/// using the alternative digit ordering (0 to 9, then a to z, then A to Z) with lowercase
/// letters before uppercase letters.
///
/// Returns a [`Result`] containing the decoded
/// [`u128`] or a [`DecodeError`].
///
/// # Examples
///
/// ```rust
/// extern crate base62;
///
/// let value = base62::decode_alternative("rustlang").unwrap();
/// assert_eq!(value, 96813686712946);
/// ```
pub fn decode_alternative<T: AsRef<[u8]>>(input: T) -> Result<u128, DecodeError> {
    decode_impl(input.as_ref(), &ALTERNATIVE_TABLES.decode)
}

// Common encoding function
unsafe fn encode_impl(num: u128, digits: usize, buf: &mut [u8], encode_table: &[u8; 62]) -> usize {
    unsafe {
        if let Ok(num) = TryInto::<u64>::try_into(num) {
            encode_impl_u64(num, digits, buf, encode_table)
        } else if digits > 20 {
            encode_impl_over_20_digits(num, digits, buf, encode_table)
        } else if digits == 20 {
            //  (AAAAAAAAAA, BBBBBBBBBB)
            let (first_u64, second_u64) = div_base_to_10(num);
            // AAAAAAAAAA
            let first_u64 = first_u64 as u64;

            encode_impl_20_digits(first_u64, second_u64, buf, encode_table)
        } else {
            // digits between 11 and 20 (10 digits would always fit into a u64, which we checked first)
            encode_impl_over_10_under_20_digits(num, digits, buf, encode_table)
        }
    }
}

// >20 digits requires two u128 divisions
unsafe fn encode_impl_over_20_digits(
    num: u128,
    digits: usize,
    buf: &mut [u8],
    encode_table: &[u8; 62],
) -> usize {
    // input: AABBBBBBBBBBCCCCCCCCCC
    //
    //  (AABBBBBBBBBB, CCCCCCCCCC)
    let (num, third_u64) = div_base_to_10(num);
    //  (AA, BBBBBBBBBB)
    let (first_u64, second_u64) = div_base_to_10(num);
    //   AA - no more than two digits as num was 22 digits
    let first_u64 = first_u64 as u64;

    // encode the first one or two digits
    if digits == 21 {
        unsafe {
            *buf.get_unchecked_mut(0) = *encode_table.get_unchecked(first_u64 as usize);
        }
    } else {
        let second_digit = first_u64 % BASE;
        let first_digit = first_u64 / BASE;
        unsafe {
            *buf.get_unchecked_mut(1) = *encode_table.get_unchecked(second_digit as usize);
            *buf.get_unchecked_mut(0) = *encode_table.get_unchecked(first_digit as usize);
        }
    }

    // encode the last 20 digits
    unsafe {
        encode_impl_20_digits(
            second_u64,
            third_u64,
            &mut buf[(digits - 20)..],
            encode_table,
        );
    }

    digits
}

// 20 digit needs only u64 or u32 divisions and can be vectorised (four u32 divisions at once)
unsafe fn encode_impl_20_digits(
    first_u64: u64,
    second_u64: u64,
    buf: &mut [u8],
    encode_table: &[u8; 62],
) -> usize {
    let first_u32 = (first_u64 / BASE_TO_5) as u32;
    let second_u32 = (first_u64 % BASE_TO_5) as u32;
    let third_u32 = (second_u64 / BASE_TO_5) as u32;
    let fourth_u32 = (second_u64 % BASE_TO_5) as u32;

    // [AAAAA, BBBBB, CCCCC, DDDDD]
    let mut nums = [first_u32, second_u32, third_u32, fourth_u32];
    const STARTING_WRITE_IDXS: [usize; 4] = [5, 10, 15, 20];

    for i in 0..5 {
        nums.iter_mut()
            .zip(STARTING_WRITE_IDXS)
            .for_each(|(num, starting_write_idx)| {
                let quotient = num.wrapping_div(BASE as u32);
                let remainder = (*num - (BASE as u32) * quotient) as usize;
                *num = quotient;

                unsafe {
                    *buf.get_unchecked_mut(starting_write_idx - i - 1) =
                        *encode_table.get_unchecked(remainder)
                }
            });
    }

    20
}

// 10-20 digit implementation needs only one u128 division, then a u64 division per digit
unsafe fn encode_impl_over_10_under_20_digits(
    num: u128,
    digits: usize,
    buf: &mut [u8],
    encode_table: &[u8; 62],
) -> usize {
    let mut write_idx = digits;
    let mut digit_index = 0_usize;

    let (first_u64, mut num) = div_base_to_10(num);
    // as this number is <20 digits, once we remove the rightmost 10 digits, the remainder is a u64.
    let first_u64 = first_u64 as u64;

    while digit_index < digits {
        write_idx = write_idx.wrapping_sub(1);

        let remainder = num % BASE;
        num /= BASE;

        unsafe {
            *buf.get_unchecked_mut(write_idx) = *encode_table.get_unchecked(remainder as usize);
        }

        digit_index = digit_index.wrapping_add(1);
        if digit_index == 10 {
            num = first_u64
        }
    }

    digits
}

// u64 implementation can avoid any u128 operations
unsafe fn encode_impl_u64(
    num: u64,
    digits: usize,
    buf: &mut [u8],
    encode_table: &[u8; 62],
) -> usize {
    if digits == 11 {
        // ABBBBBBBBBB

        // A
        let first_u64 = num / (BASE_TO_10 as u64);
        // BBBBBBBBBB
        let second_u64 = num % (BASE_TO_10 as u64);

        unsafe {
            *buf.get_unchecked_mut(0) = *encode_table.get_unchecked(first_u64 as usize);

            encode_impl_u64_10_digits(second_u64, &mut buf[1..], encode_table);
        }
        digits
    } else if digits == 10 {
        unsafe { encode_impl_u64_10_digits(num, buf, encode_table) }
    } else {
        unsafe { encode_impl_u64_under_10_digits(num, digits, buf, encode_table) }
    }
}

unsafe fn encode_impl_u64_under_10_digits(
    mut num: u64,
    digits: usize,
    buf: &mut [u8],
    encode_table: &[u8; 62],
) -> usize {
    let mut write_idx = digits;
    let mut digit_index = 0_usize;

    while digit_index < digits {
        write_idx = write_idx.wrapping_sub(1);

        let remainder = num % BASE;
        num /= BASE;

        unsafe {
            *buf.get_unchecked_mut(write_idx) = *encode_table.get_unchecked(remainder as usize);
        }

        digit_index = digit_index.wrapping_add(1);
    }

    digits
}

unsafe fn encode_impl_u64_10_digits(num: u64, buf: &mut [u8], encode_table: &[u8; 62]) -> usize {
    let first_u32 = (num / BASE_TO_5) as u32;
    let second_u32 = (num % BASE_TO_5) as u32;

    // [AAAAA, BBBBB]
    let mut nums = [first_u32, second_u32];
    const STARTING_WRITE_IDXS: [usize; 2] = [5, 10];

    for i in 0..5 {
        nums.iter_mut()
            .zip(STARTING_WRITE_IDXS)
            .for_each(|(num, starting_write_idx)| {
                let quotient = num.wrapping_div(BASE as u32);
                let remainder = (*num - (BASE as u32) * quotient) as usize;
                *num = quotient;

                unsafe {
                    *buf.get_unchecked_mut(starting_write_idx - i - 1) =
                        *encode_table.get_unchecked(remainder)
                }
            });
    }

    10
}

fn div_base_to_10(num: u128) -> (u128, u64) {
    let quotient = mulh(DIV_BASE_TO_10_MULTIPLY, num) >> DIV_BASE_TO_10_SHIFT;
    let remainder = num - BASE_TO_10 * quotient;
    (quotient, remainder as u64)
}

// Multiply two u128 together, returning only the top half of the product.
const fn mulh(x: u128, y: u128) -> u128 {
    const LOWER_HALF_MASK: u128 = (1 << 64) - 1;

    let x_low = x & LOWER_HALF_MASK;
    let y_low = y & LOWER_HALF_MASK;
    let t = x_low.wrapping_mul(y_low);
    let k = t >> 64;

    let x_high = x >> 64;
    let t = x_high.wrapping_mul(y_low) + k;
    let k = t & LOWER_HALF_MASK;
    let w1 = t >> 64;

    let y_high = y >> 64;
    let t = x_low.wrapping_mul(y_high) + k;
    let k = t >> 64;

    x_high.wrapping_mul(y_high) + w1 + k
}

unsafe fn _encode_buf(num: u128, digits: usize, buf: &mut [u8]) -> usize {
    unsafe { encode_impl(num, digits, buf, &STANDARD_TABLES.encode) }
}

unsafe fn _encode_alternative_buf(num: u128, digits: usize, buf: &mut [u8]) -> usize {
    unsafe { encode_impl(num, digits, buf, &ALTERNATIVE_TABLES.encode) }
}

#[cfg(feature = "alloc")]
mod alloc_support {
    use super::*;
    use alloc::string::String;

    /// Encodes an unsigned integer into base62, using the standard digit ordering
    /// (0 to 9, then A to Z, then a to z), and returns the resulting
    /// [`String`].
    ///
    /// # Example
    ///
    /// ```rust
    /// extern crate base62;
    ///
    /// let b62 = base62::encode(1337_u32);
    /// assert_eq!(b62, "LZ");
    /// ```
    #[must_use = "this allocates a new `String`"]
    pub fn encode<T: Into<u128>>(num: T) -> String {
        let num = num.into();
        let digits = digit_count(num);

        unsafe {
            let mut buf = String::with_capacity(digits);
            let ptr = buf.as_mut_vec().as_mut_ptr();
            let slice = core::slice::from_raw_parts_mut(ptr, digits);

            let len = _encode_buf(num, digits, slice);
            debug_assert_eq!(len, digits);

            buf.as_mut_vec().set_len(digits);
            buf
        }
    }

    /// Encodes an unsigned integer into base62, using the standard digit ordering
    /// (0 to 9, then A to Z, then a to z), and then appends it onto the end of the given
    /// [`String`].
    ///
    /// # Example
    ///
    /// ```rust
    /// extern crate base62;
    ///
    /// let mut buf = String::from("1337 in base62: ");
    /// base62::encode_buf(1337_u32, &mut buf);
    /// assert_eq!(buf, "1337 in base62: LZ");
    /// ```
    pub fn encode_buf<T: Into<u128>>(num: T, buf: &mut String) {
        let num = num.into();
        let digits = digit_count(num);
        let old_len = buf.len();

        buf.reserve(digits);

        unsafe {
            let vec = buf.as_mut_vec();
            let ptr = vec.as_mut_ptr().add(old_len);
            let slice = core::slice::from_raw_parts_mut(ptr, digits);

            let len = _encode_buf(num, digits, slice);
            debug_assert_eq!(len, digits);

            vec.set_len(old_len + digits);
        }
    }

    /// Encodes an unsigned integer into base62, using the alternative digit ordering
    /// (0 to 9, then a to z, then A to Z) with lowercase letters before uppercase letters,
    /// and returns the resulting [`String`].
    ///
    /// # Example
    ///
    /// ```rust
    /// extern crate base62;
    ///
    /// let b62 = base62::encode_alternative(1337_u32);
    /// assert_eq!(b62, "lz");
    /// ```
    #[must_use = "this allocates a new `String`"]
    pub fn encode_alternative<T: Into<u128>>(num: T) -> String {
        let num = num.into();
        let digits = digit_count(num);

        unsafe {
            let mut buf = String::with_capacity(digits);
            let ptr = buf.as_mut_vec().as_mut_ptr();
            let slice = core::slice::from_raw_parts_mut(ptr, digits);

            let len = _encode_alternative_buf(num, digits, slice);
            debug_assert_eq!(len, digits);

            buf.as_mut_vec().set_len(digits);
            buf
        }
    }

    /// Encodes an unsigned integer into base62, using the alternative digit ordering
    /// (0 to 9, then a to z, then A to Z) with lowercase letters before uppercase letters, and
    /// then appends it onto the end of the given [`String`].
    ///
    /// # Example
    ///
    /// ```rust
    /// extern crate base62;
    ///
    /// let mut buf = String::from("1337 in base62 alternative: ");
    /// base62::encode_alternative_buf(1337_u32, &mut buf);
    /// assert_eq!(buf, "1337 in base62 alternative: lz");
    /// ```
    pub fn encode_alternative_buf<T: Into<u128>>(num: T, buf: &mut String) {
        let num = num.into();
        let digits = digit_count(num);
        let old_len = buf.len();

        buf.reserve(digits);

        unsafe {
            let vec = buf.as_mut_vec();
            let ptr = vec.as_mut_ptr().add(old_len);
            let slice = core::slice::from_raw_parts_mut(ptr, digits);

            let len = _encode_alternative_buf(num, digits, slice);
            debug_assert_eq!(len, digits);

            vec.set_len(old_len + digits);
        }
    }
}

#[cfg(feature = "alloc")]
pub use alloc_support::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "alloc")]
    use alloc::string::String;
    #[cfg(feature = "alloc")]
    use alloc::vec::Vec;

    // Don't run quickcheck tests under miri because that's infinitely slow
    #[cfg(all(test, feature = "alloc", not(miri)))]
    mod quickcheck_tests {
        use super::*;
        use alloc::string::ToString;
        use quickcheck::{quickcheck, TestResult};
        quickcheck! {
            fn encode_decode(num: u128) -> bool {
                decode(encode(num)) == Ok(num)
            }
        }

        quickcheck! {
            fn encode_decode_alternative(num: u128) -> bool {
                decode_alternative(encode_alternative(num)) == Ok(num)
            }
        }

        quickcheck! {
            fn decode_bad(input: Vec<u8>) -> TestResult {
                if !input.is_empty() && input.iter().all(|ch| ch.is_ascii_alphanumeric()) {
                    TestResult::discard()
                } else {
                    TestResult::from_bool(decode(&input).is_err())
                }
            }
        }

        quickcheck! {
            fn decode_good(input: Vec<u8>) -> TestResult {
                if !input.is_empty() && input.iter().all(|ch| ch.is_ascii_alphanumeric()) {
                    TestResult::from_bool(decode(&input).is_ok())
                } else {
                    TestResult::discard()
                }
            }
        }

        quickcheck! {
            fn fmt_matches_encode(num: u128) -> bool {
                let direct = encode(num);
                let formatted = encode_fmt(num).to_string();
                direct == formatted
            }
        }

        quickcheck! {
            fn fmt_alt_matches_encode_alt(num: u128) -> bool {
                let direct = encode_alternative(num);
                let formatted = encode_fmt_alt(num).to_string();
                direct == formatted
            }
        }

        #[cfg(feature = "std")]
        quickcheck! {
            fn encode_io_decode(num: u128) -> bool {
                let mut v = Vec::new();
                encode_io(num, &mut v).unwrap();
                decode(&v) == Ok(num)
            }
        }

        #[cfg(feature = "std")]
        quickcheck! {
            fn encode_alternative_io_decode(num: u128) -> bool {
                let mut v = Vec::new();
                encode_alternative_io(num, &mut v).unwrap();
                decode_alternative(&v) == Ok(num)
            }
        }

        quickcheck! {
            fn encode_buf_decode(num: u128) -> bool {
                let mut s = String::new();
                encode_buf(num, &mut s);
                decode(&s) == Ok(num)
            }
        }

        quickcheck! {
            fn encode_alternative_buf_decode(num: u128) -> bool {
                let mut s = String::new();
                encode_alternative_buf(num, &mut s);
                decode_alternative(&s) == Ok(num)
            }
        }

        quickcheck! {
            fn encode_bytes_decode(num: u128) -> bool {
                let mut buf = [0u8; 22];  // Max size needed for u128
                if let Ok(len) = encode_bytes(num, &mut buf) {
                    decode(&buf[..len]) == Ok(num)
                } else {
                    false
                }
            }
        }

        quickcheck! {
            fn encode_alternative_bytes_decode(num: u128) -> bool {
                let mut buf = [0u8; 22];  // Max size needed for u128
                if let Ok(len) = encode_alternative_bytes(num, &mut buf) {
                    decode_alternative(&buf[..len]) == Ok(num)
                } else {
                    false
                }
            }
        }

        quickcheck! {
            fn div_base_to_10_matches_std(num: u128) -> bool {
                let (quotient_fast, remainder_fast) = div_base_to_10(num);
                let quotient = num / BASE_TO_10;
                let remainder = (num % BASE_TO_10) as u64;

                quotient == quotient_fast && remainder == remainder_fast
            }
        }
    }

    // Pure computation tests that don't need allocation
    #[test]
    fn test_digit_count() {
        // Test boundaries for each power of 62
        for pow in 1..22 {
            let this_power = (BASE as u128).pow(pow as u32);
            assert_eq!(digit_count(this_power - 1), pow);
            assert_eq!(digit_count(this_power), pow + 1);
        }

        // Test specific boundary cases
        assert_eq!(digit_count(0), 1);
        assert_eq!(digit_count(1), 1);
        assert_eq!(digit_count(u64::MAX as u128), 11);
        assert_eq!(digit_count(u64::MAX as u128 + 1), 11);
        assert_eq!(digit_count(u128::MAX), 22);
    }

    #[test]
    fn test_decode_empty_input() {
        assert_eq!(decode([]), Err(DecodeError::EmptyInput));
    }

    #[test]
    fn test_decode_overflow() {
        let long_input = [b'1'; 23];
        assert_eq!(decode(long_input), Err(DecodeError::ArithmeticOverflow));
    }

    #[test]
    fn test_decode_invalid_char() {
        assert_eq!(decode(b"!"), Err(DecodeError::InvalidBase62Byte(b'!', 0)));
    }

    #[test]
    fn test_decode_alternative_empty_input() {
        assert_eq!(decode_alternative([]), Err(DecodeError::EmptyInput));
    }

    #[test]
    fn test_decode_alternative_overflow() {
        let long_input = [b'1'; 23];
        assert_eq!(
            decode_alternative(long_input),
            Err(DecodeError::ArithmeticOverflow)
        );
    }

    #[test]
    fn test_decode_alternative_invalid_char() {
        assert_eq!(
            decode_alternative(b"!"),
            Err(DecodeError::InvalidBase62Byte(b'!', 0))
        );
    }

    #[test]
    fn test_encode_bytes() {
        let mut buf = [0; 22];

        // Test numeric type boundaries
        assert!(encode_bytes(u128::MAX, &mut buf).is_ok());
        assert!(&buf[..].starts_with(b"7n42DGM5Tflk9n8mt7Fhc7"));
        buf.fill(0);

        assert!(encode_bytes(u64::MAX as u128 + 1, &mut buf).is_ok());
        assert!(&buf[..].starts_with(b"LygHa16AHYG"));
        buf.fill(0);

        assert!(encode_bytes(u64::MAX, &mut buf).is_ok());
        assert!(&buf[..].starts_with(b"LygHa16AHYF"));
        buf.fill(0);

        assert!(encode_bytes(0_u8, &mut buf).is_ok());
        assert!(&buf[..].starts_with(b"0"));
        buf.fill(0);

        #[cfg(feature = "alloc")]
        {
            use alloc::string::String;
            // Test base62 length-change boundaries
            let mut power = 1_u128;
            let mut power_minus_one_str = String::with_capacity(21);
            let mut power_str = String::with_capacity(22);
            power_str.push('1');
            for _ in 1..22 {
                power *= BASE as u128;
                power_minus_one_str.push('z');
                power_str.push('0');

                assert!(encode_bytes(power - 1, &mut buf).is_ok());
                assert!(&buf[..].starts_with(power_minus_one_str.as_bytes()));
                buf.fill(0);

                assert!(encode_bytes(power, &mut buf).is_ok());
                assert!(&buf[..].starts_with(power_str.as_bytes()));
                buf.fill(0);
            }
        }

        // Test cases that failed due to earlier bugs
        assert!(encode_bytes(691337691337_u64, &mut buf).is_ok());
        assert!(&buf[..].starts_with(b"CAcoUun"));
        buf.fill(0);

        assert!(encode_bytes(92202686130861137968548313400401640448_u128, &mut buf).is_ok());
        assert!(&buf[..].starts_with(b"26tF05fvSIgh0000000000"));
    }

    #[test]
    fn test_encode_alternative_bytes() {
        let mut buf = [0; 22];

        // Test numeric type boundaries
        assert!(encode_alternative_bytes(u128::MAX, &mut buf).is_ok());
        assert!(&buf[..].starts_with(b"7N42dgm5tFLK9N8MT7fHC7"));
        buf.fill(0);

        assert!(encode_alternative_bytes(u64::MAX as u128 + 1, &mut buf).is_ok());
        assert!(&buf[..].starts_with(b"lYGhA16ahyg"));
        buf.fill(0);

        assert!(encode_alternative_bytes(u64::MAX, &mut buf).is_ok());
        assert!(&buf[..].starts_with(b"lYGhA16ahyf"));
        buf.fill(0);

        assert!(encode_alternative_bytes(0_u8, &mut buf).is_ok());
        assert!(&buf[..].starts_with(b"0"));
        buf.fill(0);

        #[cfg(feature = "alloc")]
        {
            use alloc::string::String;
            // Test base62 length-change boundaries
            let mut power = 1_u128;
            let mut power_minus_one_str = String::with_capacity(21);
            let mut power_str = String::with_capacity(22);
            power_str.push('1');
            for _ in 1..22 {
                power *= BASE as u128;
                power_minus_one_str.push('Z');
                power_str.push('0');

                assert!(encode_alternative_bytes(power - 1, &mut buf).is_ok());
                assert!(&buf[..].starts_with(power_minus_one_str.as_bytes()));
                buf.fill(0);

                assert!(encode_alternative_bytes(power, &mut buf).is_ok());
                assert!(&buf[..].starts_with(power_str.as_bytes()));
                buf.fill(0);
            }
        }

        // Test cases that failed due to earlier bugs
        assert!(encode_alternative_bytes(691337691337_u64, &mut buf).is_ok());
        assert!(&buf[..].starts_with(b"caCOuUN"));
        buf.fill(0);

        assert!(
            encode_alternative_bytes(92202686130861137968548313400401640448_u128, &mut buf).is_ok()
        );
        assert!(&buf[..].starts_with(b"26Tf05FVsiGH0000000000"));
    }

    #[test]
    fn test_decode() {
        // Test leading zeroes handling
        assert_eq!(
            decode("00001000000000000000000000"),
            Ok((BASE as u128).pow(21))
        );

        // Test numeric type boundaries
        assert_eq!(decode("7n42DGM5Tflk9n8mt7Fhc7"), Ok(u128::MAX));
        assert_eq!(decode("LygHa16AHYG"), Ok(u64::MAX as u128 + 1));
        assert_eq!(decode("LygHa16AHYF"), Ok(u64::MAX as u128));
        assert_eq!(decode("0"), Ok(0));

        #[cfg(feature = "alloc")]
        {
            use alloc::string::String;
            // Test base62 length-change boundaries
            let mut power = 1_u128;
            let mut power_minus_one_str = String::with_capacity(21);
            let mut power_str = String::with_capacity(22);
            power_str.push('1');
            for _ in 1..22 {
                power *= BASE as u128;
                power_minus_one_str.push('z');
                power_str.push('0');

                assert_eq!(decode(&power_minus_one_str), Ok(power - 1));
                assert_eq!(decode(&power_str), Ok(power));
            }
        }

        // Test cases that failed due to earlier bugs
        assert_eq!(decode("CAcoUun"), Ok(691337691337));
        assert_eq!(
            decode("26tF05fvSIgh0000000000"),
            Ok(92202686130861137968548313400401640448)
        );
    }

    #[test]
    fn test_decode_alternative() {
        // Test leading zeroes handling
        assert_eq!(
            decode_alternative("00001000000000000000000000"),
            Ok((BASE as u128).pow(21))
        );

        // Test numeric type boundaries
        assert_eq!(decode_alternative("7N42dgm5tFLK9N8MT7fHC7"), Ok(u128::MAX));
        assert_eq!(decode_alternative("lYGhA16ahyg"), Ok(u64::MAX as u128 + 1));
        assert_eq!(decode_alternative("lYGhA16ahyf"), Ok(u64::MAX as u128));
        assert_eq!(decode_alternative("0"), Ok(0));

        #[cfg(feature = "alloc")]
        {
            use alloc::string::String;
            // Test base62 length-change boundaries
            let mut power = 1_u128;
            let mut power_minus_one_str = String::with_capacity(21);
            let mut power_str = String::with_capacity(22);
            power_str.push('1');
            for _ in 1..22 {
                power *= BASE as u128;
                power_minus_one_str.push('Z');
                power_str.push('0');

                assert_eq!(decode_alternative(&power_minus_one_str), Ok(power - 1));
                assert_eq!(decode_alternative(&power_str), Ok(power));
            }
        }

        // Test cases that failed due to earlier bugs
        assert_eq!(decode_alternative("caCOuUN"), Ok(691337691337));
        assert_eq!(
            decode_alternative("26Tf05FVsiGH0000000000"),
            Ok(92202686130861137968548313400401640448)
        );
    }

    // Tests requiring alloc
    #[cfg(feature = "alloc")]
    mod alloc_tests {
        use super::*;
        use alloc::{format, string::ToString};

        #[test]
        fn test_encode() {
            // Test numeric type boundaries
            assert_eq!(encode(u128::MAX), "7n42DGM5Tflk9n8mt7Fhc7");
            assert_eq!(encode(u64::MAX as u128 + 1), "LygHa16AHYG");
            assert_eq!(encode(u64::MAX), "LygHa16AHYF");
            assert_eq!(encode(0_u8), "0");

            // Test base62 length-change boundaries
            let mut power = 1_u128;
            let mut power_minus_one_str = String::with_capacity(21);
            let mut power_str = String::with_capacity(22);
            power_str.push('1');
            for _ in 1..22 {
                power *= BASE as u128;
                power_minus_one_str.push('z');
                power_str.push('0');

                assert_eq!(encode(power - 1), power_minus_one_str);
                assert_eq!(encode(power), power_str);
            }
        }

        #[test]
        fn test_encode_buf() {
            let mut buf = String::new();

            // Test append functionality
            buf.push_str("Base62: ");
            encode_buf(10622433094_u64, &mut buf);
            assert_eq!(buf, "Base62: Base62");
            buf.clear();

            // Test numeric type boundaries
            encode_buf(u128::MAX, &mut buf);
            assert_eq!(buf, "7n42DGM5Tflk9n8mt7Fhc7");
            buf.clear();
        }

        #[test]
        fn test_encode_alternative() {
            assert_eq!(encode_alternative(u128::MAX), "7N42dgm5tFLK9N8MT7fHC7");
            assert_eq!(encode_alternative(u64::MAX as u128 + 1), "lYGhA16ahyg");
            assert_eq!(encode_alternative(u64::MAX), "lYGhA16ahyf");
            assert_eq!(encode_alternative(0_u8), "0");
        }

        #[test]
        fn test_encode_alternative_buf() {
            let mut buf = String::new();
            buf.push_str("Base62: ");
            encode_alternative_buf(34051405518_u64, &mut buf);
            assert_eq!(buf, "Base62: Base62");
        }

        #[test]
        fn test_fmt_basics() {
            assert_eq!(encode_fmt(1337u32).to_string(), "LZ");
            assert_eq!(encode_fmt(0u32).to_string(), "0");
            assert_eq!(encode_fmt(u128::MAX).to_string(), "7n42DGM5Tflk9n8mt7Fhc7");
        }

        #[test]
        fn test_fmt_padding() {
            assert_eq!(
                format!("{:0>22}", encode_fmt(1337u32)),
                "00000000000000000000LZ"
            );
            assert_eq!(
                format!("{:0>22}", encode_fmt(0u32)),
                "0000000000000000000000"
            );
        }

        #[test]
        fn test_fmt_alternative_basics() {
            assert_eq!(encode_fmt_alt(1337u32).to_string(), "lz");
            assert_eq!(encode_fmt_alt(0u32).to_string(), "0");
            assert_eq!(
                encode_fmt_alt(u128::MAX).to_string(),
                "7N42dgm5tFLK9N8MT7fHC7"
            );
        }

        #[test]
        fn test_fmt_alternative_padding() {
            assert_eq!(
                format!("{:0>22}", encode_fmt_alt(1337u32)),
                "00000000000000000000lz"
            );
            assert_eq!(
                format!("{:0>22}", encode_fmt_alt(0u32)),
                "0000000000000000000000"
            );
        }

        #[test]
        fn test_fmt_alignment() {
            let num = 1337u32;
            assert_eq!(format!("{:<5}", encode_fmt(num)), "LZ   ");
            assert_eq!(format!("{:>5}", encode_fmt(num)), "   LZ");
            assert_eq!(format!("{:^5}", encode_fmt(num)), " LZ  ");
        }

        #[test]
        fn test_fmt_with_prefix() {
            assert_eq!(format!("base62: {}", encode_fmt(1337u32)), "base62: LZ");
            assert_eq!(format!("base62: {}", encode_fmt_alt(1337u32)), "base62: lz");
        }
    }

    // Tests requiring std
    #[cfg(feature = "std")]
    mod std_tests {
        use super::*;

        #[test]
        fn test_encode_io() {
            let mut output = Vec::new();
            encode_io(1337_u32, &mut output).unwrap();
            assert_eq!(output, b"LZ");
            output.clear();

            encode_io(0_u8, &mut output).unwrap();
            assert_eq!(output, b"0");
        }

        #[test]
        fn test_encode_alternative_io() {
            let mut output = Vec::new();
            encode_alternative_io(1337_u32, &mut output).unwrap();
            assert_eq!(output, b"lz");
            output.clear();

            encode_alternative_io(0_u8, &mut output).unwrap();
            assert_eq!(output, b"0");

            // Test numeric type boundaries
            output.clear();
            encode_alternative_io(u128::MAX, &mut output).unwrap();
            assert_eq!(output, b"7N42dgm5tFLK9N8MT7fHC7");

            output.clear();
            encode_alternative_io(u64::MAX as u128 + 1, &mut output).unwrap();
            assert_eq!(output, b"lYGhA16ahyg");
        }

        #[test]
        fn test_encode_bytes_buffer_too_small() {
            let mut buf = [0; 1];
            assert_eq!(
                encode_bytes(1337_u16, &mut buf),
                Err(EncodeError::BufferTooSmall)
            );
        }
    }
}
