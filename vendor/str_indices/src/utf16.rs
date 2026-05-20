//! Index by utf16 code units.

use crate::byte_chunk::{ByteChunk, Chunk};

/// Counts the utf16 code units that would be in a string slice if it
/// were encoded as utf16.
///
/// Runs in O(N) time.
#[inline]
pub fn count(text: &str) -> usize {
    crate::chars::count_impl::<Chunk>(text.as_bytes())
        + count_surrogates_impl::<Chunk>(text.as_bytes())
}

/// Counts the utf16 surrogate pairs that would be in a string slice if
/// it were encoded as utf16.
///
/// Runs in O(N) time.
#[inline]
pub fn count_surrogates(text: &str) -> usize {
    count_surrogates_impl::<Chunk>(text.as_bytes())
}

/// Converts from byte-index to utf16-code-unit-index in a string slice.
///
/// If the byte is in the middle of a multi-byte char, returns the utf16
/// index of the char that the byte belongs to.
///
/// Any past-the-end index will return the one-past-the-end utf16 index.
///
/// Runs in O(N) time.
#[inline]
pub fn from_byte_idx(text: &str, byte_idx: usize) -> usize {
    let mut i = byte_idx.min(text.len());
    while !text.is_char_boundary(i) {
        i -= 1;
    }
    let slice = &text.as_bytes()[..i];
    crate::chars::count_impl::<Chunk>(slice) + count_surrogates_impl::<Chunk>(slice)
}

/// Converts from utf16-code-unit-index to byte-index in a string slice.
///
/// If the utf16 index is in the middle of a char, returns the bytes
/// index of the char that utf16 code unit belongs to.
///
/// Any past-the-end index will return the one-past-the-end byte index.
///
/// Runs in O(N) time.
#[inline]
pub fn to_byte_idx(text: &str, utf16_idx: usize) -> usize {
    to_byte_idx_impl::<Chunk>(text, utf16_idx)
}

//-------------------------------------------------------------

#[inline(always)]
fn to_byte_idx_impl<T: ByteChunk>(text: &str, utf16_idx: usize) -> usize {
    // Get `middle` so we can do more efficient chunk-based counting.
    // We can't use this to get `end`, however, because the start index of
    // `end` actually depends on the accumulating char counts during the
    // counting process.
    let (start, middle, _) = unsafe { text.as_bytes().align_to::<T>() };

    let mut byte_count = 0;
    let mut utf16_count = 0;

    // Take care of any unaligned bytes at the beginning.
    for byte in start.iter() {
        utf16_count += ((*byte & 0xC0) != 0x80) as usize + ((byte & 0xf0) == 0xf0) as usize;
        if utf16_count > utf16_idx {
            break;
        }
        byte_count += 1;
    }

    // Process chunks in the fast path.
    let mut chunks = middle;
    let mut max_round_len = utf16_idx.saturating_sub(utf16_count) / T::MAX_ACC;
    while max_round_len > 0 && !chunks.is_empty() {
        // Choose the largest number of chunks we can do this round
        // that will neither overflow `max_acc` nor blast past the
        // utf16 code unit we're looking for.
        let round_len = T::MAX_ACC.min(max_round_len).min(chunks.len());
        max_round_len -= round_len;
        let round = &chunks[..round_len];
        chunks = &chunks[round_len..];

        // Process the chunks in this round.
        let mut acc_inv_chars = T::zero();
        let mut acc_surrogates = T::zero();
        for chunk in round.iter() {
            acc_inv_chars = acc_inv_chars.add(chunk.bitand(T::splat(0xc0)).cmp_eq_byte(0x80));
            acc_surrogates = acc_surrogates.add(chunk.bitand(T::splat(0xf0)).cmp_eq_byte(0xf0));
        }
        utf16_count +=
            ((T::SIZE * round_len) - acc_inv_chars.sum_bytes()) + acc_surrogates.sum_bytes();
        byte_count += T::SIZE * round_len;
    }

    // Process chunks in the slow path.
    for chunk in chunks.iter() {
        let inv_chars = chunk.bitand(T::splat(0xc0)).cmp_eq_byte(0x80).sum_bytes();
        let surrogates = chunk.bitand(T::splat(0xf0)).cmp_eq_byte(0xf0).sum_bytes();
        let new_utf16_count = utf16_count + (T::SIZE - inv_chars) + surrogates;
        if new_utf16_count >= utf16_idx {
            break;
        }
        utf16_count = new_utf16_count;
        byte_count += T::SIZE;
    }

    // Take care of any unaligned bytes at the end.
    let end = &text.as_bytes()[byte_count..];
    for byte in end.iter() {
        utf16_count += ((*byte & 0xC0) != 0x80) as usize + ((byte & 0xf0) == 0xf0) as usize;
        if utf16_count > utf16_idx {
            break;
        }
        byte_count += 1;
    }

    byte_count
}

#[inline(always)]
fn count_surrogates_impl<T: ByteChunk>(text: &[u8]) -> usize {
    // We chop off the last three bytes, because all surrogate pairs are
    // four bytes in utf8, and so it prevents counting partial
    // characters.
    if text.len() <= 3 {
        return 0;
    }
    let text = &text[..(text.len() - 3)];

    // Get `middle` for more efficient chunk-based counting.
    let (start, middle, end) = unsafe { text.align_to::<T>() };

    let mut utf16_surrogate_count = 0;

    // Take care of unaligned bytes at the beginning.
    for byte in start.iter() {
        utf16_surrogate_count += ((byte & 0xf0) == 0xf0) as usize;
    }

    // Take care of the middle bytes in big chunks.
    for chunks in middle.chunks(T::MAX_ACC) {
        let mut acc = T::zero();
        for chunk in chunks.iter() {
            acc = acc.add(chunk.bitand(T::splat(0xf0)).cmp_eq_byte(0xf0));
        }
        utf16_surrogate_count += acc.sum_bytes();
    }

    // Take care of unaligned bytes at the end.
    for byte in end.iter() {
        utf16_surrogate_count += ((byte & 0xf0) == 0xf0) as usize;
    }

    utf16_surrogate_count
}

//=============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // 45 bytes, 27 utf16 code units.
    const TEXT: &str = "Hel🐸lo world! こん🐸にち🐸🐸は!";

    #[test]
    fn count_01() {
        assert_eq!(27, count(TEXT));
    }

    #[test]
    fn count_surrogates_01() {
        assert_eq!(4, count_surrogates(TEXT));
    }

    #[test]
    fn from_byte_idx_01() {
        assert_eq!(0, from_byte_idx(TEXT, 0));

        assert_eq!(3, from_byte_idx(TEXT, 3));
        assert_eq!(3, from_byte_idx(TEXT, 4));
        assert_eq!(3, from_byte_idx(TEXT, 5));
        assert_eq!(3, from_byte_idx(TEXT, 6));
        assert_eq!(5, from_byte_idx(TEXT, 7));

        assert_eq!(7, from_byte_idx(TEXT, 9));

        assert_eq!(17, from_byte_idx(TEXT, 23));
        assert_eq!(17, from_byte_idx(TEXT, 24));
        assert_eq!(17, from_byte_idx(TEXT, 25));
        assert_eq!(17, from_byte_idx(TEXT, 26));
        assert_eq!(19, from_byte_idx(TEXT, 27));

        assert_eq!(21, from_byte_idx(TEXT, 33));
        assert_eq!(21, from_byte_idx(TEXT, 34));
        assert_eq!(21, from_byte_idx(TEXT, 35));
        assert_eq!(21, from_byte_idx(TEXT, 36));
        assert_eq!(23, from_byte_idx(TEXT, 37));
        assert_eq!(23, from_byte_idx(TEXT, 38));
        assert_eq!(23, from_byte_idx(TEXT, 39));
        assert_eq!(23, from_byte_idx(TEXT, 40));
        assert_eq!(25, from_byte_idx(TEXT, 41));

        assert_eq!(27, from_byte_idx(TEXT, 45));
        assert_eq!(27, from_byte_idx(TEXT, 46)); // Index 1 past the end.
    }

    #[test]
    fn to_byte_idx_01() {
        assert_eq!(to_byte_idx(TEXT, 0), 0);

        assert_eq!(3, to_byte_idx(TEXT, 3));
        assert_eq!(3, to_byte_idx(TEXT, 4));
        assert_eq!(7, to_byte_idx(TEXT, 5));

        assert_eq!(9, to_byte_idx(TEXT, 7));

        assert_eq!(23, to_byte_idx(TEXT, 17));
        assert_eq!(23, to_byte_idx(TEXT, 18));
        assert_eq!(27, to_byte_idx(TEXT, 19));

        assert_eq!(33, to_byte_idx(TEXT, 21));
        assert_eq!(33, to_byte_idx(TEXT, 22));
        assert_eq!(37, to_byte_idx(TEXT, 23));
        assert_eq!(37, to_byte_idx(TEXT, 24));
        assert_eq!(41, to_byte_idx(TEXT, 25));

        assert_eq!(45, to_byte_idx(TEXT, 27));
        assert_eq!(45, to_byte_idx(TEXT, 27)); // Index 1 past the end.
    }
}
