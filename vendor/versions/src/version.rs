//! Types and logic for handling general [`Version`]s.

use crate::{Chunk, Chunks, Error, MChunk, Mess, Release, Sep};
use nom::character::complete::char;
use nom::combinator::opt;
use nom::{IResult, Parser};
use std::cmp::Ordering;
use std::cmp::Ordering::{Equal, Greater, Less};
use std::hash::Hash;
use std::str::FromStr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A version number with decent structure and comparison logic.
///
/// This is a *descriptive* scheme, meaning that it encapsulates the most
/// common, unconscious patterns that developers use when assigning version
/// numbers to their software. If not [`crate::SemVer`], most version numbers
/// found in the wild will parse as a `Version`. These generally conform to the
/// `x.x.x-x` pattern, and may optionally have an *epoch*.
///
/// # Epochs
///
/// Epochs are prefixes marked by a colon, like in `1:2.3.4`. When comparing two
/// `Version` values, epochs take precedent. So `2:1.0.0 > 1:9.9.9`. If one of
/// the given `Version`s has no epoch, its epoch is assumed to be `0`.
///
/// # Examples
///
/// ```
/// use versions::{SemVer, Version};
///
/// // None of these are SemVer, but can still be parsed and compared.
/// let vers = vec!["0.25-2", "8.u51-1", "20150826-1", "1:2.3.4"];
///
/// for v in vers {
///     assert!(SemVer::new(v).is_none());
///     assert!(Version::new(v).is_some());
/// }
/// ```

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Eq, Hash, Clone, Default)]
pub struct Version {
    /// An optional prefix that marks that some paradigm shift in versioning has
    /// occurred between releases of some software.
    pub epoch: Option<u32>,
    /// The main sections of the `Version`. Unlike [`crate::SemVer`], these
    /// sections are allowed to contain letters.
    pub chunks: Chunks,
    /// This either indicates a prerelease like [`crate::SemVer`], or a
    /// "release" revision for software packages. In the latter case, a version
    /// like `1.2.3-2` implies that the software itself hasn't changed, but that
    /// this is the second bundling/release (etc.) of that particular package.
    pub release: Option<Release>,
    /// Some extra metadata that doesn't factor into comparison.
    pub meta: Option<String>,
}

impl Version {
    /// Parse a `Version` from some input.
    pub fn new<S>(s: S) -> Option<Version>
    where
        S: AsRef<str>,
    {
        match Version::parse(s.as_ref()) {
            Ok(("", v)) => Some(v),
            _ => None,
        }
    }

    /// Try to extract a position from the `Version` as a nice integer, as if it
    /// were a [`crate::SemVer`].
    ///
    /// ```
    /// use versions::Version;
    ///
    /// let mess = Version::new("1:2.a.4.5.6.7-r1").unwrap();
    /// assert_eq!(Some(2), mess.nth(0));
    /// assert_eq!(None, mess.nth(1));
    /// assert_eq!(Some(4), mess.nth(2));
    /// ```
    pub fn nth(&self, n: usize) -> Option<u32> {
        self.chunks.0.get(n).and_then(Chunk::single_digit)
    }

    /// Like `nth`, but pulls a number even if it was followed by letters.
    pub fn nth_lenient(&self, n: usize) -> Option<u32> {
        self.chunks.0.get(n).and_then(Chunk::single_digit_lenient)
    }

    /// A lossless conversion from `Version` to [`Mess`].
    ///
    /// ```
    /// use versions::Version;
    ///
    /// let orig = "1:1.2.3-r1";
    /// let mess = Version::new(orig).unwrap().to_mess();
    ///
    /// assert_eq!(orig, format!("{}", mess));
    /// ```
    pub fn to_mess(&self) -> Mess {
        match self.epoch {
            None => self.to_mess_continued(),
            Some(e) => {
                let chunks = vec![MChunk::Digits(e, e.to_string())];
                let next = Some((Sep::Colon, Box::new(self.to_mess_continued())));
                Mess { chunks, next }
            }
        }
    }

    /// Convert to a `Mess` without considering the epoch.
    fn to_mess_continued(&self) -> Mess {
        let chunks = self.chunks.0.iter().map(|c| c.mchunk()).collect();
        let next = self.release.as_ref().map(|cs| {
            let chunks = cs.0.iter().map(|c| c.mchunk()).collect();
            (Sep::Hyphen, Box::new(Mess { chunks, next: None }))
        });
        Mess { chunks, next }
    }

    /// If we're lucky, we can pull specific numbers out of both inputs and
    /// accomplish the comparison without extra allocations.
    pub(crate) fn cmp_mess(&self, other: &Mess) -> Ordering {
        match self.epoch {
            Some(e) if e > 0 && other.chunks.len() == 1 => match &other.next {
                // A near-nonsense case where a `Mess` is comprised of a single
                // digit and nothing else. In this case its epoch would be
                // considered 0.
                None => Greater,
                Some((Sep::Colon, m)) => match other.nth(0) {
                    // The Mess's epoch is a letter, etc.
                    None => Greater,
                    Some(me) => match e.cmp(&me) {
                        Equal => Version::cmp_mess_continued(self, m),
                        ord => ord,
                    },
                },
                // Similar nonsense, where the Mess had a single *something*
                // before some non-colon separator. We then consider the epoch
                // to be 0.
                Some(_) => Greater,
            },
            // The `Version` has an epoch but the `Mess` doesn't. Or if it does,
            // it's malformed.
            Some(e) if e > 0 => Greater,
            _ => Version::cmp_mess_continued(self, other),
        }
    }

    /// It's assumed the epoch check has already been done, and we're comparing
    /// the main parts of each version now.
    fn cmp_mess_continued(&self, other: &Mess) -> Ordering {
        (0..)
            .find_map(
                |n| match self.nth(n).and_then(|x| other.nth(n).map(|y| x.cmp(&y))) {
                    // Sane values can't be extracted from one or both of the
                    // arguments.
                    None => Some(self.to_mess().cmp(other)),
                    Some(Greater) => Some(Greater),
                    Some(Less) => Some(Less),
                    // Continue to the next position.
                    Some(Equal) => None,
                },
            )
            .unwrap_or_else(|| self.to_mess().cmp(other))
    }

    /// The raw `nom` parser for [`Version`]. Feel free to use this in
    /// combination with other general `nom` parsers.
    pub fn parse(i: &str) -> IResult<&str, Version> {
        let (i, epoch) = opt(Version::epoch).parse(i)?;
        let (i, chunks) = Chunks::parse(i)?;
        let (i, release) = opt(Release::parse).parse(i)?;
        let (i, meta) = opt(crate::parsers::meta).parse(i)?;

        let v = Version {
            epoch,
            chunks,
            meta,
            release,
        };

        Ok((i, v))
    }

    fn epoch(i: &str) -> IResult<&str, u32> {
        let (i, epoch) = crate::parsers::unsigned(i)?;
        let (i, _) = char(':')(i)?;

        Ok((i, epoch))
    }

    pub(crate) fn matches_tilde(&self, other: &Version) -> bool {
        if self.chunks.0.len() != other.chunks.0.len() {
            false
        } else {
            // Compare all but the final chunk.
            let inits_equal = self
                .chunks
                .0
                .iter()
                .rev()
                .skip(1)
                .rev()
                .zip(other.chunks.0.iter().rev().skip(1).rev())
                .all(|(a, b)| a == b);

            let last_good = match (self.chunks.0.last(), other.chunks.0.last()) {
                // TODO: Do our best with strings. Right now, the alpha patch version can be "less" than the
                // first one and this will still be true
                (Some(Chunk::Alphanum(_)), Some(Chunk::Alphanum(_))) => true,
                (Some(Chunk::Numeric(n1)), Some(Chunk::Numeric(n2))) => n2 >= n1,
                _ => false,
            };

            inits_equal && last_good
        }
    }

    // TODO 2024-01-11 Refactor this to be more functional-style.
    pub(crate) fn matches_caret(&self, other: &Version) -> bool {
        let mut got_first_nonzero = false;

        for (v1_chunk, v2_chunk) in self.chunks.0.iter().zip(other.chunks.0.iter()) {
            if !got_first_nonzero {
                if !v1_chunk.single_digit().is_some_and(|n| n == 0) {
                    got_first_nonzero = true;

                    if v1_chunk != v2_chunk {
                        return false;
                    }
                }
            } else if v2_chunk.cmp_lenient(v1_chunk).is_lt() {
                return false;
            }
        }

        true
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    /// If two epochs are equal, we need to compare their actual version
    /// numbers. Otherwise, the comparison of the epochs is the only thing that
    /// matters.
    fn cmp(&self, other: &Self) -> Ordering {
        let ae = self.epoch.unwrap_or(0);
        let be = other.epoch.unwrap_or(0);
        match ae.cmp(&be) {
            Equal => match self.chunks.cmp(&other.chunks) {
                Equal => self.release.cmp(&other.release),
                ord => ord,
            },
            ord => ord,
        }
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(e) = self.epoch {
            write!(f, "{}:", e)?;
        }

        write!(f, "{}", self.chunks)?;

        if let Some(r) = &self.release {
            write!(f, "-{}", r)?;
        }

        if let Some(m) = &self.meta {
            write!(f, "+{}", m)?;
        }

        Ok(())
    }
}

impl FromStr for Version {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Version::new(s).ok_or_else(|| Error::IllegalVersion(s.to_string()))
    }
}

impl TryFrom<&str> for Version {
    type Error = Error;

    /// ```
    /// use versions::Version;
    ///
    /// let orig = "1.2.3.4";
    /// let prsd: Version = orig.try_into().unwrap();
    /// assert_eq!(orig, prsd.to_string());
    /// ```
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Version::from_str(value)
    }
}
