#![no_std]

//! Count and convert between different indexing schemes on utf8 string
//! slices.
//!
//! This crate is organized into modules by indexing scheme.  Each module
//! contains functions for counting relevant metrics for that scheme as
//! well as functions for converting to/from byte indices.
//!
//! None of the functions in this crate panic: all inputs have a defined
//! output.

mod byte_chunk;
pub mod chars;
pub mod lines;
pub mod lines_crlf;
pub mod lines_lf;
pub mod utf16;

/// Returns the alignment difference between the start of `bytes` and the
/// type `T`.
///
/// Or put differently: returns how many bytes into `bytes` you need to walk
/// to reach the alignment of `T` in memory.
///
/// Will return 0 if already aligned at the start, and will return the length
/// of `bytes` if alignment is beyond the end of `bytes`.
#[inline(always)]
fn alignment_diff<T>(bytes: &[u8]) -> usize {
    let alignment = core::mem::align_of::<T>();
    let ptr = bytes.as_ptr() as usize;
    (alignment - ((ptr - 1) & (alignment - 1)) - 1).min(bytes.len())
}

/// Utility function used in some of the lines modules.
#[inline(always)]
fn is_not_crlf_middle(byte_idx: usize, text: &[u8]) -> bool {
    byte_idx == 0
        || byte_idx >= text.len()
        || (text[byte_idx - 1] != 0x0D)
        || (text[byte_idx] != 0x0A)
}

//======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // 124 bytes, 100 chars, 4 lines
    const TEXT_LINES: &str = "Hello there!  How're you doing?\nIt's \
                              a fine day, isn't it?\nAren't you glad \
                              we're alive?\nこんにちは、みんなさん！";

    fn char_to_line_idx(text: &str, idx: usize) -> usize {
        lines::from_byte_idx(text, chars::to_byte_idx(text, idx))
    }

    fn line_to_char_idx(text: &str, idx: usize) -> usize {
        chars::from_byte_idx(text, lines::to_byte_idx(text, idx))
    }

    #[test]
    fn char_to_line_idx_01() {
        let text = "Hello せ\nか\nい!";
        assert_eq!(0, char_to_line_idx(text, 0));
        assert_eq!(0, char_to_line_idx(text, 7));
        assert_eq!(1, char_to_line_idx(text, 8));
        assert_eq!(1, char_to_line_idx(text, 9));
        assert_eq!(2, char_to_line_idx(text, 10));
    }

    #[test]
    fn char_to_line_idx_02() {
        // Line 0
        for i in 0..32 {
            assert_eq!(0, char_to_line_idx(TEXT_LINES, i));
        }

        // Line 1
        for i in 32..59 {
            assert_eq!(1, char_to_line_idx(TEXT_LINES, i));
        }

        // Line 2
        for i in 59..88 {
            assert_eq!(2, char_to_line_idx(TEXT_LINES, i));
        }

        // Line 3
        for i in 88..100 {
            assert_eq!(3, char_to_line_idx(TEXT_LINES, i));
        }

        // Past the end
        for i in 100..110 {
            assert_eq!(3, char_to_line_idx(TEXT_LINES, i));
        }
    }

    #[test]
    fn line_to_char_idx_01() {
        let text = "Hello せ\nか\nい!";
        assert_eq!(0, line_to_char_idx(text, 0));
        assert_eq!(8, line_to_char_idx(text, 1));
        assert_eq!(10, line_to_char_idx(text, 2));
    }

    #[test]
    fn line_to_char_idx_02() {
        assert_eq!(0, line_to_char_idx(TEXT_LINES, 0));
        assert_eq!(32, line_to_char_idx(TEXT_LINES, 1));
        assert_eq!(59, line_to_char_idx(TEXT_LINES, 2));
        assert_eq!(88, line_to_char_idx(TEXT_LINES, 3));

        // Past end
        assert_eq!(100, line_to_char_idx(TEXT_LINES, 4));
        assert_eq!(100, line_to_char_idx(TEXT_LINES, 5));
        assert_eq!(100, line_to_char_idx(TEXT_LINES, 6));
    }

    #[test]
    fn line_char_round_trip() {
        let text = "\nHere\nare\nsome\nwords\n";
        assert_eq!(6, line_to_char_idx(text, char_to_line_idx(text, 6)));
        assert_eq!(2, char_to_line_idx(text, line_to_char_idx(text, 2)));

        assert_eq!(0, line_to_char_idx(text, char_to_line_idx(text, 0)));
        assert_eq!(0, char_to_line_idx(text, line_to_char_idx(text, 0)));

        assert_eq!(21, line_to_char_idx(text, char_to_line_idx(text, 21)));
        assert_eq!(5, char_to_line_idx(text, line_to_char_idx(text, 5)));
    }
}
