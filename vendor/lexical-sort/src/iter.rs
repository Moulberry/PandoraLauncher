//! Iterators to transliterate Unicode to ASCII. Note that only alphanumeric
//! characters are transliterated, and not all of them are supported.
//!
//! Characters can be transliterated to multiple ASCII characters. For example,
//! `æ` is converted to `ae`, and `½` is converted to `1/2`.
//!
//! The iterators don't allocate memory on the heap. I haven't benchmarked it,
//! but I believe that it's quite efficient.

use any_ascii::any_ascii_char;
use core::iter::FusedIterator;

/// An iterator over one `char`, converted to lowercase
/// and transliterated to ASCII, if it is an alphanumeric character
///
/// This iterator can be created by calling `iterate_lexical_char()` or
/// `iterate_lexical_char_only_alnum()`
pub struct LexicalChar(CharOrSlice);

impl LexicalChar {
    #[inline]
    fn from_char(c: char) -> Self {
        LexicalChar(CharOrSlice::Char(c))
    }

    #[inline]
    fn from_slice(s: &'static [u8]) -> Self {
        LexicalChar(CharOrSlice::Slice(s))
    }

    #[inline]
    fn empty() -> Self {
        LexicalChar(CharOrSlice::Slice(&[]))
    }

    #[inline]
    fn inner(&self) -> &CharOrSlice {
        &self.0
    }

    #[inline]
    fn inner_mut(&mut self) -> &mut CharOrSlice {
        &mut self.0
    }
}

enum CharOrSlice {
    Char(char),
    Slice(&'static [u8]),
}

impl Iterator for LexicalChar {
    type Item = char;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.inner_mut() {
            &mut CharOrSlice::Char(c) => {
                *self = LexicalChar::empty();
                Some(c)
            }
            CharOrSlice::Slice(slice) => match slice.get(0_usize) {
                Some(&next) => {
                    *slice = &slice[1..];
                    Some((next as char).to_ascii_lowercase())
                }
                None => None,
            },
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.inner() {
            CharOrSlice::Char(_) => (1, Some(1)),
            CharOrSlice::Slice(s) => (s.len(), Some(s.len())),
        }
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        if n == 0 {
            self.next()
        } else if let CharOrSlice::Slice(slice) = self.inner_mut() {
            match slice.get(n) {
                Some(&next) => {
                    *slice = &slice[1..];
                    Some((next as char).to_ascii_lowercase())
                }
                None => None,
            }
        } else {
            None
        }
    }
}

impl FusedIterator for LexicalChar {}

impl ExactSizeIterator for LexicalChar {}

impl DoubleEndedIterator for LexicalChar {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        match self.inner_mut() {
            &mut CharOrSlice::Char(c) => {
                *self = LexicalChar::empty();
                Some(c)
            }
            CharOrSlice::Slice(slice) => {
                if slice.len() > 0 {
                    let ix = slice.len() - 1;
                    *slice = &slice[..ix];
                    Some((slice[ix] as char).to_ascii_lowercase())
                } else {
                    None
                }
            }
        }
    }
}

/// Returns an iterator over one `char`, converted to lowercase
/// and transliterated to ASCII, if it is alphanumeric
#[inline]
pub fn iterate_lexical_char(c: char) -> LexicalChar {
    if c.is_ascii() {
        LexicalChar::from_char(c.to_ascii_lowercase())
    } else if c.is_alphanumeric() {
        match any_ascii_char(c) {
            s if s.is_empty() => LexicalChar::from_char(c),
            s => LexicalChar::from_slice(s.as_bytes()),
        }
    } else if combining_diacritical(&c) {
        LexicalChar::empty()
    } else {
        LexicalChar::from_char(c)
    }
}

/// Returns an iterator over one `char`, converted to lowercase
/// and transliterated to ASCII, if it is alphanumeric
#[inline]
pub fn iterate_lexical_char_only_alnum(c: char) -> LexicalChar {
    if c.is_ascii() {
        if c.is_ascii_alphanumeric() {
            LexicalChar::from_char(c.to_ascii_lowercase())
        } else {
            LexicalChar::empty()
        }
    } else if c.is_alphanumeric() {
        match any_ascii_char(c) {
            s if s.is_empty() => LexicalChar::from_char(c),
            s => LexicalChar::from_slice(s.as_bytes()),
        }
    } else {
        LexicalChar::empty()
    }
}

/// returns `true` for combining diacritical marks
#[inline]
fn combining_diacritical(&c: &char) -> bool {
    c >= '\u{300}' && c <= '\u{36F}'
}

/// Returns an iterator over the characters of a string, converted to lowercase
/// and transliterated to ASCII, if they're alphanumeric
pub fn iterate_lexical(s: &'_ str) -> impl Iterator<Item = char> + '_ {
    s.chars().flat_map(iterate_lexical_char)
}

/// Returns an iterator over the characters of a string, converted to lowercase
/// and transliterated to ASCII. Non-alphanumeric characters are skipped
pub fn iterate_lexical_only_alnum(s: &'_ str) -> impl Iterator<Item = char> + '_ {
    s.chars().flat_map(iterate_lexical_char_only_alnum)
}

#[test]
#[cfg(feature = "std")]
fn test_iteration() {
    fn it(s: &'static str) -> String {
        iterate_lexical(s).collect()
    }

    assert_eq!(&it("Hello, world!"), "hello, world!");
    assert_eq!(&it("Ω A æ b ö ß é"), "o a ae b o ss e");
    assert_eq!(&it("3½/⅝ £ → € ®™"), "31/2/5/8 £ → € ®™");
    assert_eq!(&it("»@« 15% ¡¹!"), "»@« 15% ¡1!");
    assert_eq!(&it("🎉🦄☣"), "🎉🦄☣");
    assert_eq!(&it("北亰"), "beijing");
    assert_eq!(&it("ΣΣΣ"), "sss");
    assert_eq!(&it("à"), "a"); // 'a' with combining diacritical mark '\u{300}'
}

#[test]
#[cfg(feature = "std")]
fn test_iteration_only_alnum() {
    fn it(s: &'static str) -> String {
        iterate_lexical_only_alnum(s).collect()
    }

    assert_eq!(&it("Hello, world!"), "helloworld");
    assert_eq!(&it("Ω A æ b ö ß é"), "oaaebosse");
    assert_eq!(&it("3½/⅝ £ → € ®™"), "31/25/8");
    assert_eq!(&it("»@« 15% ¡¹!"), "151");
    assert_eq!(&it("🎉🦄☣"), "");
    assert_eq!(&it("北亰"), "beijing");
    assert_eq!(&it("ΣΣΣ"), "sss");
    assert_eq!(&it("à"), "a"); // 'a' with combining diacritical mark '\u{300}'
}
