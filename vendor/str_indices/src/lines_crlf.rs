//! Index by lines (carriage return and line feed).
//!
//! This module recognizes the following as line breaks:
//!
//! - `U+000A`          &mdash; LF (Line Feed)
//! - `U+000D`          &mdash; CR (Carriage Return)
//! - `U+000D` `U+000A` &mdash; CRLF (Carriage Return + Line Feed)
//!
//! (Note: if you only want to recognize LF and CRLF, without
//! recognizing CR individually, see the [`lines_lf`](crate::lines_lf) module.)

use crate::byte_chunk::{ByteChunk, Chunk};

/// Counts the line breaks in a string slice.
///
/// Runs in O(N) time.
#[inline]
pub fn count_breaks(text: &str) -> usize {
    count_breaks_impl::<Chunk>(text.as_bytes())
}

/// Converts from byte-index to line-index in a string slice.
///
/// Line break characters are considered to be a part of the line they
/// end.  And a string that ends with a line break is considered to have
/// a final empty line.  So this function is equivalent to counting the
/// line breaks before the specified byte.
///
/// Any past-the-end index will return the last line index.
///
/// Runs in O(N) time.
#[inline]
pub fn from_byte_idx(text: &str, byte_idx: usize) -> usize {
    let i = byte_idx.min(text.len());
    let nl_count = count_breaks_impl::<Chunk>(&text.as_bytes()[..i]);
    if crate::is_not_crlf_middle(i, text.as_bytes()) {
        nl_count
    } else {
        nl_count - 1
    }
}

/// Converts from line-index to byte-index in a string slice.
///
/// Returns the byte index of the start of the specified line.  Line 0 is
/// the start of the string, and subsequent lines start immediately
/// *after* each line break character.
///
/// Any past-the-end index will return the one-past-the-end byte index.
///
/// Runs in O(N) time.
#[inline]
pub fn to_byte_idx(text: &str, line_idx: usize) -> usize {
    to_byte_idx_impl::<Chunk>(text.as_bytes(), line_idx)
}

//-------------------------------------------------------------
const LF: u8 = b'\n';
const CR: u8 = b'\r';

#[inline(always)]
fn to_byte_idx_impl<T: ByteChunk>(text: &[u8], line_idx: usize) -> usize {
    // Get `middle` so we can do more efficient chunk-based counting.
    // We can't use this to get `end`, however, because the start index of
    // `end` actually depends on the accumulating line counts during the
    // counting process.
    let (start, middle, _) = unsafe { text.align_to::<T>() };

    let mut byte_count = 0;
    let mut break_count = 0;

    // Take care of any unaligned bytes at the beginning.
    let mut last_was_cr = false;
    for byte in start.iter().copied() {
        let is_lf = byte == LF;
        let is_cr = byte == CR;
        if break_count == line_idx {
            if last_was_cr && is_lf {
                byte_count += 1;
            }
            return byte_count;
        }
        if is_cr || (is_lf && !last_was_cr) {
            break_count += 1;
        }
        last_was_cr = is_cr;
        byte_count += 1;
    }

    // Process the chunks 2 at a time.
    let mut chunk_count = 0;
    let mut prev = T::splat(last_was_cr as u8);
    for chunks in middle.chunks_exact(2) {
        let lf_flags0 = chunks[0].cmp_eq_byte(LF);
        let cr_flags0 = chunks[0].cmp_eq_byte(CR);
        let crlf_flags0 = prev.shift_across(cr_flags0).bitand(lf_flags0);

        let lf_flags1 = chunks[1].cmp_eq_byte(LF);
        let cr_flags1 = chunks[1].cmp_eq_byte(CR);
        let crlf_flags1 = cr_flags0.shift_across(cr_flags1).bitand(lf_flags1);
        let new_break_count = break_count
            + lf_flags0
                .add(cr_flags0)
                .add(lf_flags1)
                .add(cr_flags1)
                .sub(crlf_flags0)
                .sub(crlf_flags1)
                .sum_bytes();
        if new_break_count >= line_idx {
            break;
        }
        break_count = new_break_count;
        byte_count += T::SIZE * 2;
        chunk_count += 2;
        prev = cr_flags1;
    }

    // Process the rest of the chunks.
    for chunk in middle[chunk_count..].iter() {
        let lf_flags = chunk.cmp_eq_byte(LF);
        let cr_flags = chunk.cmp_eq_byte(CR);
        let crlf_flags = prev.shift_across(cr_flags).bitand(lf_flags);
        let new_break_count = break_count + lf_flags.add(cr_flags).sub(crlf_flags).sum_bytes();
        if new_break_count >= line_idx {
            break;
        }
        break_count = new_break_count;
        byte_count += T::SIZE;
        prev = cr_flags;
    }

    // Take care of any unaligned bytes at the end.
    last_was_cr = text.get(byte_count.saturating_sub(1)) == Some(&CR);
    for byte in text[byte_count..].iter().copied() {
        let is_lf = byte == LF;
        let is_cr = byte == CR;
        if break_count == line_idx {
            if last_was_cr && is_lf {
                byte_count += 1;
            }
            break;
        }
        if is_cr || (is_lf && !last_was_cr) {
            break_count += 1;
        }
        last_was_cr = is_cr;
        byte_count += 1;
    }

    // Finish up
    byte_count
}

/// Counts the line breaks in a utf8 encoded string.
///
/// The following unicode sequences are considered newlines by this function:
/// - u{000A}        (Line Feed)
/// - u{000D}        (Carriage Return)
#[inline(always)]
fn count_breaks_impl<T: ByteChunk>(text: &[u8]) -> usize {
    // Get `middle` so we can do more efficient chunk-based counting.
    let (start, middle, end) = unsafe { text.align_to::<T>() };

    let mut count = 0;

    // Take care of unaligned bytes at the beginning.
    let mut last_was_cr = false;
    for byte in start.iter().copied() {
        let is_lf = byte == LF;
        let is_cr = byte == CR;
        count += (is_cr | (is_lf & !last_was_cr)) as usize;
        last_was_cr = is_cr;
    }

    // Take care of the middle bytes in big chunks.
    let mut prev = T::splat(last_was_cr as u8);
    for chunks in middle.chunks_exact(2) {
        let lf_flags0 = chunks[0].cmp_eq_byte(LF);
        let cr_flags0 = chunks[0].cmp_eq_byte(CR);
        let crlf_flags0 = prev.shift_across(cr_flags0).bitand(lf_flags0);

        let lf_flags1 = chunks[1].cmp_eq_byte(LF);
        let cr_flags1 = chunks[1].cmp_eq_byte(CR);
        let crlf_flags1 = cr_flags0.shift_across(cr_flags1).bitand(lf_flags1);
        count += lf_flags0
            .add(cr_flags0)
            .sub(crlf_flags0)
            .add(lf_flags1)
            .add(cr_flags1)
            .sub(crlf_flags1)
            .sum_bytes();
        prev = cr_flags1;
    }

    if let Some(chunk) = middle.chunks_exact(2).remainder().iter().next() {
        let lf_flags = chunk.cmp_eq_byte(LF);
        let cr_flags = chunk.cmp_eq_byte(CR);
        let crlf_flags = prev.shift_across(cr_flags).bitand(lf_flags);
        count += lf_flags.add(cr_flags).sub(crlf_flags).sum_bytes();
    }

    // Take care of unaligned bytes at the end.
    last_was_cr = text.get((text.len() - end.len()).saturating_sub(1)) == Some(&CR);
    for byte in end.iter().copied() {
        let is_lf = byte == LF;
        let is_cr = byte == CR;
        count += (is_cr | (is_lf & !last_was_cr)) as usize;
        last_was_cr = is_cr;
    }

    count
}

//=============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // 124 bytes, 100 chars, 4 lines
    const TEXT_LINES: &str = "Hello there!  How're you doing?\nIt's \
                              a fine day, isn't it?\nAren't you glad \
                              we're alive?\nこんにちは、みんなさん！";

    #[test]
    fn count_breaks_01() {
        let text = "\u{000A}Hello\u{000D}\u{000A}せ\u{000B}か\u{000C}い\u{0085}. \
                    There\u{000A}is something.\u{2029}";
        assert_eq!(45, text.len());
        assert_eq!(3, count_breaks(text));
    }

    #[test]
    fn from_byte_idx_01() {
        let text = "Here\nare\nsome\nwords";
        assert_eq!(0, from_byte_idx(text, 0));
        assert_eq!(0, from_byte_idx(text, 4));
        assert_eq!(1, from_byte_idx(text, 5));
        assert_eq!(1, from_byte_idx(text, 8));
        assert_eq!(2, from_byte_idx(text, 9));
        assert_eq!(2, from_byte_idx(text, 13));
        assert_eq!(3, from_byte_idx(text, 14));
        assert_eq!(3, from_byte_idx(text, 19));
    }

    #[test]
    fn from_byte_idx_02() {
        let text = "\nHere\nare\nsome\nwords\n";
        assert_eq!(0, from_byte_idx(text, 0));
        assert_eq!(1, from_byte_idx(text, 1));
        assert_eq!(1, from_byte_idx(text, 5));
        assert_eq!(2, from_byte_idx(text, 6));
        assert_eq!(2, from_byte_idx(text, 9));
        assert_eq!(3, from_byte_idx(text, 10));
        assert_eq!(3, from_byte_idx(text, 14));
        assert_eq!(4, from_byte_idx(text, 15));
        assert_eq!(4, from_byte_idx(text, 20));
        assert_eq!(5, from_byte_idx(text, 21));
    }

    #[test]
    fn from_byte_idx_03() {
        let text = "Here\r\nare\r\nsome\r\nwords";
        assert_eq!(0, from_byte_idx(text, 0));
        assert_eq!(0, from_byte_idx(text, 4));
        assert_eq!(0, from_byte_idx(text, 5));
        assert_eq!(1, from_byte_idx(text, 6));
        assert_eq!(1, from_byte_idx(text, 9));
        assert_eq!(1, from_byte_idx(text, 10));
        assert_eq!(2, from_byte_idx(text, 11));
        assert_eq!(2, from_byte_idx(text, 15));
        assert_eq!(2, from_byte_idx(text, 16));
        assert_eq!(3, from_byte_idx(text, 17));
    }

    #[test]
    fn from_byte_idx_04() {
        // Line 0
        for i in 0..32 {
            assert_eq!(0, from_byte_idx(TEXT_LINES, i));
        }

        // Line 1
        for i in 32..59 {
            assert_eq!(1, from_byte_idx(TEXT_LINES, i));
        }

        // Line 2
        for i in 59..88 {
            assert_eq!(2, from_byte_idx(TEXT_LINES, i));
        }

        // Line 3
        for i in 88..125 {
            assert_eq!(3, from_byte_idx(TEXT_LINES, i));
        }

        // Past the end
        for i in 125..130 {
            assert_eq!(3, from_byte_idx(TEXT_LINES, i));
        }
    }

    #[test]
    fn to_byte_idx_01() {
        let text = "Here\r\nare\r\nsome\r\nwords";
        assert_eq!(0, to_byte_idx(text, 0));
        assert_eq!(6, to_byte_idx(text, 1));
        assert_eq!(11, to_byte_idx(text, 2));
        assert_eq!(17, to_byte_idx(text, 3));
    }

    #[test]
    fn to_byte_idx_02() {
        let text = "\nHere\nare\nsome\nwords\n";
        assert_eq!(0, to_byte_idx(text, 0));
        assert_eq!(1, to_byte_idx(text, 1));
        assert_eq!(6, to_byte_idx(text, 2));
        assert_eq!(10, to_byte_idx(text, 3));
        assert_eq!(15, to_byte_idx(text, 4));
        assert_eq!(21, to_byte_idx(text, 5));
    }

    #[test]
    fn to_byte_idx_03() {
        assert_eq!(0, to_byte_idx(TEXT_LINES, 0));
        assert_eq!(32, to_byte_idx(TEXT_LINES, 1));
        assert_eq!(59, to_byte_idx(TEXT_LINES, 2));
        assert_eq!(88, to_byte_idx(TEXT_LINES, 3));

        // Past end
        assert_eq!(124, to_byte_idx(TEXT_LINES, 4));
        assert_eq!(124, to_byte_idx(TEXT_LINES, 5));
        assert_eq!(124, to_byte_idx(TEXT_LINES, 6));
    }

    #[test]
    fn line_byte_round_trip() {
        let text = "\nHere\nare\nsome\nwords\n";
        assert_eq!(6, to_byte_idx(text, from_byte_idx(text, 6)));
        assert_eq!(2, from_byte_idx(text, to_byte_idx(text, 2)));

        assert_eq!(0, to_byte_idx(text, from_byte_idx(text, 0)));
        assert_eq!(0, from_byte_idx(text, to_byte_idx(text, 0)));

        assert_eq!(21, to_byte_idx(text, from_byte_idx(text, 21)));
        assert_eq!(5, from_byte_idx(text, to_byte_idx(text, 5)));
    }
}
