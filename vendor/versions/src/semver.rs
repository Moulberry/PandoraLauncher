//! Types and logic for handling ideal [`SemVer`]s.

use crate::{Chunk, Chunks, Error, MChunk, Mess, Release, Sep, Version};
use nom::character::complete::char;
use nom::combinator::opt;
use nom::{IResult, Parser};
use std::cmp::Ordering;
use std::cmp::Ordering::{Equal, Greater, Less};
use std::hash::{Hash, Hasher};
use std::str::FromStr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// An ideal version number that conforms to Semantic Versioning.
///
/// This is a *prescriptive* scheme, meaning that it follows the [SemVer
/// standard][semver].
///
/// Legal semvers are of the form: MAJOR.MINOR.PATCH-PREREL+META
///
/// - Simple Sample: `1.2.3`
/// - Full Sample: `1.2.3-alpha.2+a1b2c3.1`
///
/// # Extra Rules
///
/// 1. Pre-release versions have *lower* precedence than normal versions.
/// 2. Build metadata does not affect version precedence.
/// 3. PREREL and META strings may only contain ASCII alphanumerics.
///
/// # Examples
///
/// ```
/// use versions::SemVer;
///
/// let orig = "1.2.3-r1+git";
/// let attempt = SemVer::new(orig).unwrap();
///
/// assert_eq!(orig, format!("{}", attempt));
/// ```
///
/// [semver]: http://semver.org
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Eq, Clone, Default)]
pub struct SemVer {
    /// The major version.
    pub major: u32,
    /// The minor version.
    pub minor: u32,
    /// The patch version.
    pub patch: u32,
    /// `Some` implies that the inner `Vec` of the `Chunks` is not empty.
    pub pre_rel: Option<Release>,
    /// `Some` implies that the inner `String` is not empty.
    pub meta: Option<String>,
}

impl SemVer {
    /// Parse a `SemVer` from some input.
    pub fn new<S>(s: S) -> Option<SemVer>
    where
        S: AsRef<str>,
    {
        match SemVer::parse(s.as_ref()) {
            Ok(("", sv)) => Some(sv),
            _ => None,
        }
    }

    /// A lossless conversion from `SemVer` to [`Version`].
    ///
    /// ```
    /// use versions::SemVer;
    ///
    /// let orig = "1.2.3-r1+git123";
    /// let ver = SemVer::new(orig).unwrap().to_version();
    ///
    /// assert_eq!("1.2.3-r1+git123", format!("{}", ver));
    /// ```
    pub fn to_version(&self) -> Version {
        let chunks = Chunks(vec![
            Chunk::Numeric(self.major),
            Chunk::Numeric(self.minor),
            Chunk::Numeric(self.patch),
        ]);

        Version {
            epoch: None,
            chunks,
            meta: self.meta.clone(),
            release: self.pre_rel.clone(),
        }
    }

    /// A lossless conversion from `SemVer` to [`Mess`].
    ///
    /// ```
    /// use versions::SemVer;
    ///
    /// let orig = "1.2.3-r1+git123";
    /// let mess = SemVer::new(orig).unwrap().to_mess();
    ///
    /// assert_eq!(orig, format!("{}", mess));
    /// ```
    pub fn to_mess(&self) -> Mess {
        let chunks = vec![
            MChunk::Digits(self.major, self.major.to_string()),
            MChunk::Digits(self.minor, self.minor.to_string()),
            MChunk::Digits(self.patch, self.patch.to_string()),
        ];
        let next = self.pre_rel.as_ref().map(|pr| {
            let chunks = pr.0.iter().map(|c| c.mchunk()).collect();
            let next = self.meta.as_ref().map(|meta| {
                let chunks = vec![MChunk::Plain(meta.clone())];
                (Sep::Plus, Box::new(Mess { chunks, next: None }))
            });

            (Sep::Hyphen, Box::new(Mess { chunks, next }))
        });

        Mess { chunks, next }
    }

    /// Analyse the `Version` as if it's a `SemVer`.
    ///
    /// `nth_lenient` pulls a leading digit from the `Version`'s chunk if it
    /// could. If it couldn't, that chunk is some string (perhaps a git hash)
    /// and is considered as marking a beta/prerelease version. It is thus
    /// considered less than the `SemVer`.
    pub(crate) fn cmp_version(&self, other: &Version) -> Ordering {
        // A `Version` with a non-zero epoch value is automatically greater than
        // any `SemVer`.
        match other.epoch {
            Some(n) if n > 0 => Less,
            _ => match other.nth_lenient(0).map(|x| self.major.cmp(&x)) {
                None => Greater,
                Some(Greater) => Greater,
                Some(Less) => Less,
                Some(Equal) => match other.nth_lenient(1).map(|x| self.minor.cmp(&x)) {
                    None => Greater,
                    Some(Greater) => Greater,
                    Some(Less) => Less,
                    Some(Equal) => match other.nth_lenient(2).map(|x| self.patch.cmp(&x)) {
                        None => Greater,
                        Some(Greater) => Greater,
                        Some(Less) => Less,
                        // By this point, the major/minor/patch positions have
                        // all been equal. If there is a fourth position, its
                        // type, not its value, will determine which overall
                        // version is greater.
                        Some(Equal) => match other.chunks.0.get(3) {
                            // 1.2.3 > 1.2.3.git
                            Some(Chunk::Alphanum(_)) => Greater,
                            // 1.2.3 < 1.2.3.0
                            Some(Chunk::Numeric(_)) => Less,
                            None => self.pre_rel.cmp(&other.release),
                        },
                    },
                },
            },
        }
    }

    /// Do our best to compare a `SemVer` and a [`Mess`].
    ///
    /// If we're lucky, the `Mess` will be well-formed enough to pull out
    /// SemVer-like values at each position, yielding sane comparisons.
    /// Otherwise we're forced to downcast the `SemVer` into a `Mess` and let
    /// the String-based `Ord` instance of `Mess` handle things.
    pub(crate) fn cmp_mess(&self, other: &Mess) -> Ordering {
        match other.nth(0).map(|x| self.major.cmp(&x)) {
            None => self.to_mess().cmp(other),
            Some(Greater) => Greater,
            Some(Less) => Less,
            Some(Equal) => match other.nth(1).map(|x| self.minor.cmp(&x)) {
                None => self.to_mess().cmp(other),
                Some(Greater) => Greater,
                Some(Less) => Less,
                Some(Equal) => match other.nth(2).map(|x| self.patch.cmp(&x)) {
                    Some(Greater) => Greater,
                    Some(Less) => Less,
                    // If they've been equal up to this point, the `Mess` will
                    // by definition have more to it, meaning that it's more
                    // likely to be newer, despite its poor shape.
                    Some(Equal) => self.to_mess().cmp(other),
                    // Even if we weren't able to extract a standalone patch
                    // number, we might still be able to find a number at the
                    // head of the `Chunk` in that position.
                    None => match other.nth_chunk(2).and_then(|c| c.single_digit_lenient()) {
                        // We were very close, but in the end the `Mess` had a
                        // nonsensical value in its patch position.
                        None => self.to_mess().cmp(other),
                        Some(p) => match self.patch.cmp(&p) {
                            Greater => Greater,
                            Less => Less,
                            // This follows SemVer's rule that pre-releases have
                            // lower precedence.
                            Equal => Greater,
                        },
                    },
                },
            },
        }
    }

    /// The raw `nom` parser for [`SemVer`]. Feel free to use this in
    /// combination with other general `nom` parsers.
    pub fn parse(i: &str) -> IResult<&str, SemVer> {
        let (i, major) = crate::parsers::unsigned(i)?;
        let (i, _) = char('.')(i)?;
        let (i, minor) = crate::parsers::unsigned(i)?;
        let (i, _) = char('.')(i)?;
        let (i, patch) = crate::parsers::unsigned(i)?;
        let (i, pre_rel) = opt(Release::parse).parse(i)?;
        let (i, meta) = opt(crate::parsers::meta).parse(i)?;

        let sv = SemVer {
            major,
            minor,
            patch,
            pre_rel,
            meta,
        };

        Ok((i, sv))
    }

    pub(crate) fn matches_tilde(&self, other: &SemVer) -> bool {
        self.major == other.major && self.minor == other.minor && other.patch >= self.patch
    }

    pub(crate) fn matches_caret(&self, other: &SemVer) -> bool {
        // Two ideal versions are caret-compatible if the first nonzero part of v1 and
        // v2 are equal and v2's parts right of the first nonzero part are greater than
        // or equal to v1's.
        if self.major == 0 && other.major == 0 {
            // If both major versions are zero, then the first nonzero part is the minor
            // version.
            if self.minor == 0 && other.minor == 0 {
                // If both minor versions are zero, then the first nonzero part is the
                // patch version.
                other.patch == self.patch
            } else {
                other.minor == self.minor && other.patch >= self.patch
            }
        } else {
            other.major == self.major
                && (other.minor > self.minor
                    || (other.minor >= self.minor && other.patch >= self.patch))
        }
    }
}

/// For Rust, it is a Law that the following must hold:
///
/// > k1 == k2 -> hash(k1) == hash(k2)
///
/// And so this is hand-implemented, since `PartialEq` also is.
impl Hash for SemVer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.major.hash(state);
        self.minor.hash(state);
        self.patch.hash(state);
        self.pre_rel.hash(state);
    }
}

/// Two SemVers are equal if all fields except metadata are equal.
impl PartialEq for SemVer {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major
            && self.minor == other.minor
            && self.patch == other.patch
            && self.pre_rel == other.pre_rel
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Build metadata does not affect version precendence, and pre-release versions
/// have *lower* precedence than normal versions.
impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        let a = (self.major, self.minor, self.patch);
        let b = (other.major, other.minor, other.patch);
        match a.cmp(&b) {
            Less => Less,
            Greater => Greater,
            Equal => match (&self.pre_rel, &other.pre_rel) {
                (None, None) => Equal,
                (None, _) => Greater,
                (_, None) => Less,
                (Some(ap), Some(bp)) => ap.cmp(bp),
            },
        }
    }
}

impl std::fmt::Display for SemVer {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;

        if let Some(p) = &self.pre_rel {
            write!(f, "-{}", p)?;
        }

        if let Some(m) = &self.meta {
            write!(f, "+{}", m)?;
        }

        Ok(())
    }
}

impl FromStr for SemVer {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        SemVer::new(s).ok_or_else(|| Error::IllegalSemver(s.to_string()))
    }
}

impl TryFrom<&str> for SemVer {
    type Error = Error;

    /// ```
    /// use versions::SemVer;
    ///
    /// let orig = "1.2.3";
    /// let prsd: SemVer = orig.try_into().unwrap();
    /// assert_eq!(orig, prsd.to_string());
    /// ```
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        SemVer::from_str(value)
    }
}
