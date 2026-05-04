//! This is a library to compare and sort strings (or file paths) **lexicographically**. This
//! means that non-ASCII characters such as `á` or `ß` are treated like their closest ASCII
//! character: `á` is treated as `a`, `ß` is treated as `ss`, etc.
//!
//! Lexical comparisons are case-insensitive. Alphanumeric characters are sorted after all other
//! characters (punctuation, whitespace, special characters, emojis, ...).
//!
//! It is possible to enable **natural sorting**, which also handles ASCII numbers. For example,
//! `50` is less than `100` with natural sorting turned on. It's also possible to skip
//! characters that aren't alphanumeric, so e.g. `f-5` is next to `f5`.
//!
//! If different strings have the same ASCII representation (e.g. `"Foo"` and `"fóò"`), it
//! falls back to the default method from the standard library, so sorting is deterministic.
//!
//! <table><tr><td>
//! <b>NOTE</b>: This crate doesn't attempt to be correct for every locale, but it should work
//! reasonably well for a wide range of locales, while providing excellent performance.
//! </td></tr></table>
//!
//! ## Usage
//!
//! To sort strings or paths, you can use the `StringSort` or `PathSort` trait:
//!
//! ```rust
//! # #[cfg(feature = "std")] {
//! use lexical_sort::{StringSort, natural_lexical_cmp};
//!
//! let mut strings = vec!["ß", "é", "100", "hello", "world", "50", ".", "B!"];
//! strings.string_sort_unstable(natural_lexical_cmp);
//!
//! assert_eq!(&strings, &[".", "50", "100", "B!", "é", "hello", "ß", "world"]);
//! # }
//! ```
//!
//! There are eight comparison functions:
//!
//! | Function                         | lexico­graphical | natural | skips non-alphanumeric chars |
//! | -------------------------------- |:---------------:|:-------:|:----------------------------:|
//! | `cmp`                            |                 |         |                              |
//! | `only_alnum_cmp`                 |                 |         | yes                          |
//! | `lexical_cmp`                    | yes             |         |                              |
//! | `lexical_only_alnum_cmp`         | yes             |         | yes                          |
//! | `natural_cmp`                    |                 | yes     |                              |
//! | `natural_only_alnum_cmp`         |                 | yes     | yes                          |
//! | `natural_lexical_cmp`            | yes             | yes     |                              |
//! | `natural_lexical_­only_alnum_cmp` | yes             | yes     | yes                          |
//!
//! Note that only the functions that sort lexicographically are case insensitive.

#![cfg_attr(not(feature = "std"), no_std)]

mod cmp;
pub mod iter;

pub use cmp::{
    cmp, lexical_cmp, lexical_only_alnum_cmp, natural_cmp, natural_lexical_cmp,
    natural_lexical_only_alnum_cmp, natural_only_alnum_cmp, only_alnum_cmp,
};

use core::cmp::Ordering;
#[cfg(feature = "std")]
use std::path::Path;

/// A trait to sort strings. This is a convenient wrapper for the standard library sort functions.
///
/// This trait is implemented for all slices whose inner type implements `AsRef<str>`.
///
/// ## Example
///
/// ```rust
/// use lexical_sort::StringSort;
///
/// let slice = &mut ["Hello", " world", "!"];
/// slice.string_sort_unstable(lexical_sort::natural_lexical_cmp);
///
/// // or trim the strings before comparing:
/// slice.string_sort_unstable_by(lexical_sort::natural_lexical_cmp, str::trim_start);
/// ```
///
/// If you want to sort file paths or OsStrings, use the `PathSort` trait instead.
pub trait StringSort {
    /// Sorts the items using the provided comparison function.
    ///
    /// **This is a stable sort, which is often not required**.
    /// You can use `string_sort_unstable` instead.
    ///
    /// ## Example
    ///
    /// ```rust
    /// use lexical_sort::StringSort;
    ///
    /// let slice = &mut ["Lorem", "ipsum", "dolor", "sit", "amet"];
    /// slice.string_sort(lexical_sort::natural_lexical_cmp);
    ///
    /// assert_eq!(slice, &["amet", "dolor", "ipsum", "Lorem", "sit"]);
    /// ```
    fn string_sort(&mut self, cmp: impl FnMut(&str, &str) -> Ordering);

    /// Sorts the items using the provided comparison function.
    ///
    /// This sort is unstable: The original order of equal strings is not preserved.
    /// It is slightly more efficient than the stable alternative.
    ///
    /// ## Example
    ///
    /// ```rust
    /// use lexical_sort::StringSort;
    ///
    /// let slice = &mut ["The", "quick", "brown", "fox"];
    /// slice.string_sort_unstable(lexical_sort::natural_lexical_cmp);
    ///
    /// assert_eq!(slice, &["brown", "fox", "quick", "The"]);
    /// ```
    fn string_sort_unstable(&mut self, cmp: impl FnMut(&str, &str) -> Ordering);

    /// Sorts the items using the provided comparison function and another function that is
    /// applied to each string before the comparison. This can be used to trim the strings.
    ///
    /// If you do anything more complicated than trimming, you'll likely run into lifetime problems.
    /// In this case you should use `[_]::sort_by()` directly.
    ///
    /// **This is a stable sort, which is often not required**.
    /// You can use `string_sort_unstable` instead.
    ///
    /// ## Example
    ///
    /// ```rust
    /// use lexical_sort::StringSort;
    ///
    /// let slice = &mut ["Eeny", " meeny", " miny", " moe"];
    /// slice.string_sort_by(lexical_sort::natural_lexical_cmp, str::trim_start);
    ///
    /// assert_eq!(slice, &["Eeny", " meeny", " miny", " moe"]);
    /// ```
    fn string_sort_by<Cmp, Map>(&mut self, cmp: Cmp, map: Map)
    where
        Cmp: FnMut(&str, &str) -> Ordering,
        Map: FnMut(&str) -> &str;

    /// Sorts the items using the provided comparison function and another function that is
    /// applied to each string before the comparison. This can be used to trim the strings.
    ///
    /// If you do anything more complicated than trimming, you'll likely run into lifetime problems.
    /// In this case you should use `[_]::sort_by()` directly.
    ///
    /// This sort is unstable: The original order of equal strings is not preserved.
    /// It is slightly more efficient than the stable alternative.
    ///
    /// ## Example
    ///
    /// ```rust
    /// use lexical_sort::StringSort;
    ///
    /// let slice = &mut ["Eeny", " meeny", " miny", " moe"];
    /// slice.string_sort_unstable_by(lexical_sort::natural_lexical_cmp, str::trim_start);
    ///
    /// assert_eq!(slice, &["Eeny", " meeny", " miny", " moe"]);
    /// ```
    fn string_sort_unstable_by<Cmp, Map>(&mut self, cmp: Cmp, map: Map)
    where
        Cmp: FnMut(&str, &str) -> Ordering,
        Map: FnMut(&str) -> &str;
}

impl<A: AsRef<str>> StringSort for [A] {
    fn string_sort(&mut self, mut cmp: impl FnMut(&str, &str) -> Ordering) {
        self.sort_by(|lhs, rhs| cmp(lhs.as_ref(), rhs.as_ref()));
    }

    fn string_sort_unstable(&mut self, mut cmp: impl FnMut(&str, &str) -> Ordering) {
        self.sort_unstable_by(|lhs, rhs| cmp(lhs.as_ref(), rhs.as_ref()));
    }

    fn string_sort_by<Cmp, Map>(&mut self, mut cmp: Cmp, mut map: Map)
    where
        Cmp: FnMut(&str, &str) -> Ordering,
        Map: FnMut(&str) -> &str,
    {
        self.sort_by(|lhs, rhs| cmp(map(lhs.as_ref()), map(rhs.as_ref())));
    }

    fn string_sort_unstable_by<Cmp, Map>(&mut self, mut cmp: Cmp, mut map: Map)
    where
        Cmp: FnMut(&str, &str) -> Ordering,
        Map: FnMut(&str) -> &str,
    {
        self.sort_unstable_by(|lhs, rhs| cmp(map(lhs.as_ref()), map(rhs.as_ref())));
    }
}

/// A trait to sort paths and OsStrings. This is a convenient wrapper for the standard library
/// sort functions.
///
/// This trait is implemented for all slices whose inner type implements `AsRef<Path>`.
///
/// ## Example
///
/// ```rust
/// # use std::path::Path;
/// use lexical_sort::PathSort;
///
/// let slice: &mut [&Path] = &mut ["Hello".as_ref(), " world".as_ref(), "!".as_ref()];
/// slice.path_sort_unstable(lexical_sort::natural_lexical_cmp);
///
/// // or trim the strings before comparing:
/// slice.path_sort_unstable_by(lexical_sort::natural_lexical_cmp, str::trim_start);
/// ```
///
/// If you want to sort regular strings, use the `StringSort` trait instead.
#[cfg(feature = "std")]
pub trait PathSort {
    /// Sorts the items using the provided comparison function.
    ///
    /// **This is a stable sort, which is often not required**.
    /// You can use `path_sort_unstable` instead.
    ///
    /// ## Example
    ///
    /// ```rust
    /// # use std::path::Path;
    /// # fn paths<'a>(s: &'a[&'a str]) -> Vec<&'a Path> { s.iter().map(Path::new).collect() }
    /// use lexical_sort::PathSort;
    ///
    /// let mut vec: Vec<&Path> = paths(&["Lorem", "ipsum", "dolor", "sit", "amet"]);
    /// vec.path_sort(lexical_sort::natural_lexical_cmp);
    ///
    /// assert_eq!(vec, paths(&["amet", "dolor", "ipsum", "Lorem", "sit"]));
    /// ```
    fn path_sort(&mut self, comparator: impl FnMut(&str, &str) -> Ordering);

    /// Sorts the items using the provided comparison function.
    ///
    /// This sort is unstable: The original order of equal strings is not preserved.
    /// It is slightly more efficient than the stable alternative.
    ///
    /// ## Example
    ///
    /// ```rust
    /// # use std::path::Path;
    /// # fn paths<'a>(s: &'a[&'a str]) -> Vec<&'a Path> { s.iter().map(Path::new).collect() }
    /// use lexical_sort::PathSort;
    ///
    /// let mut vec: Vec<&Path> = paths(&["The", "quick", "brown", "fox"]);
    /// vec.path_sort_unstable(lexical_sort::natural_lexical_cmp);
    ///
    /// assert_eq!(vec, paths(&["brown", "fox", "quick", "The"]));
    /// ```
    fn path_sort_unstable(&mut self, comparator: impl FnMut(&str, &str) -> Ordering);

    /// Sorts the items using the provided comparison function and another function that is
    /// applied to each string before the comparison. This can be used to trim the strings.
    ///
    /// If you do anything more complicated than trimming, you'll likely run into lifetime problems.
    /// In this case you should use `[_]::sort_by()` directly. You'll need to call
    /// `to_string_lossy()` or `to_str().unwrap()` to convert a `Path` or `OsStr` to a `&str` first.
    ///
    /// **This is a stable sort, which is often not required**.
    /// You can use `path_sort_unstable` instead.
    ///
    /// ## Example
    ///
    /// ```rust
    /// # use std::path::Path;
    /// # fn paths<'a>(s: &'a[&'a str]) -> Vec<&'a Path> { s.iter().map(Path::new).collect() }
    /// use lexical_sort::PathSort;
    ///
    /// let mut vec: Vec<&Path> = paths(&["Eeny", " meeny", " miny", " moe"]);
    /// vec.path_sort_by(lexical_sort::natural_lexical_cmp, str::trim_start);
    ///
    /// assert_eq!(vec, paths(&["Eeny", " meeny", " miny", " moe"]));
    /// ```
    fn path_sort_by<Cmp, Map>(&mut self, cmp: Cmp, map: Map)
    where
        Cmp: FnMut(&str, &str) -> Ordering,
        Map: FnMut(&str) -> &str;

    /// Sorts the items using the provided comparison function and another function that is
    /// applied to each string before the comparison. This can be used to trim the strings.
    ///
    /// If you do anything more complicated than trimming, you'll likely run into lifetime problems.
    /// In this case you should use `[_]::sort_by()` directly. You'll need to call
    /// `to_string_lossy()` or `to_str().unwrap()` to convert a `Path` or `OsStr` to a `&str` first.
    ///
    /// This sort is unstable: The original order of equal strings is not preserved.
    /// It is slightly more efficient than the stable alternative.
    ///
    /// ## Example
    ///
    /// ```rust
    /// # use std::path::Path;
    /// # fn paths<'a>(s: &'a[&'a str]) -> Vec<&'a Path> { s.iter().map(Path::new).collect() }
    /// use lexical_sort::PathSort;
    ///
    /// let mut vec: Vec<&Path> = paths(&["Eeny", " meeny", " miny", " moe"]);
    /// vec.path_sort_by(lexical_sort::natural_lexical_cmp, str::trim_start);
    ///
    /// assert_eq!(vec, paths(&["Eeny", " meeny", " miny", " moe"]));
    /// ```
    fn path_sort_unstable_by<Cmp, Map>(&mut self, cmp: Cmp, map: Map)
    where
        Cmp: FnMut(&str, &str) -> Ordering,
        Map: FnMut(&str) -> &str;
}

#[cfg(feature = "std")]
impl<A: AsRef<Path>> PathSort for [A] {
    fn path_sort(&mut self, mut cmp: impl FnMut(&str, &str) -> Ordering) {
        self.sort_by(|lhs, rhs| {
            cmp(
                &lhs.as_ref().to_string_lossy(),
                &rhs.as_ref().to_string_lossy(),
            )
        });
    }

    fn path_sort_unstable(&mut self, mut cmp: impl FnMut(&str, &str) -> Ordering) {
        self.sort_unstable_by(|lhs, rhs| {
            cmp(
                &lhs.as_ref().to_string_lossy(),
                &rhs.as_ref().to_string_lossy(),
            )
        });
    }

    fn path_sort_by<Cmp, Map>(&mut self, mut cmp: Cmp, mut map: Map)
    where
        Cmp: FnMut(&str, &str) -> Ordering,
        Map: FnMut(&str) -> &str,
    {
        self.sort_by(|lhs, rhs| {
            cmp(
                map(&lhs.as_ref().to_string_lossy()),
                map(&rhs.as_ref().to_string_lossy()),
            )
        });
    }

    fn path_sort_unstable_by<Cmp, Map>(&mut self, mut cmp: Cmp, mut map: Map)
    where
        Cmp: FnMut(&str, &str) -> Ordering,
        Map: FnMut(&str) -> &str,
    {
        self.sort_unstable_by(|lhs, rhs| {
            cmp(
                map(&lhs.as_ref().to_string_lossy()),
                map(&rhs.as_ref().to_string_lossy()),
            )
        });
    }
}

#[test]
fn test_sort() {
    macro_rules! assert_lexically_sorted {
        ($T:ident, $array:expr, natural = $natural:expr) => {{
            let mut sorted = $array.clone();
            if $natural {
                sorted.$T(natural_lexical_cmp);
            } else {
                sorted.$T(lexical_cmp);
            }

            assert_eq!($array, sorted);
        }};
    }

    let strings = [
        "-", "-$", "-a", "100", "50", "a", "ä", "aa", "áa", "AB", "Ab", "ab", "AE", "ae", "æ", "af",
    ];
    let strings_nat = [
        "-", "-$", "-a", "50", "100", "a", "ä", "aa", "áa", "AB", "Ab", "ab", "AE", "ae", "æ", "af",
    ];

    assert_lexically_sorted!(string_sort, strings, natural = false);
    assert_lexically_sorted!(string_sort, strings_nat, natural = true);

    #[cfg(feature = "std")]
    {
        let paths: Vec<&Path> = strings.iter().map(|s| Path::new(s)).collect();
        let paths_nat: Vec<&Path> = strings_nat.iter().map(|s| Path::new(s)).collect();

        assert_lexically_sorted!(path_sort, paths, natural = false);
        assert_lexically_sorted!(path_sort, paths_nat, natural = true);
    }
}
