//! Types and logic for handling complex [`Mess`] versions.

use crate::{Chunk, Error};
use itertools::Itertools;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{alphanumeric1, char, digit1};
use nom::combinator::eof;
use nom::combinator::{map_res, opt, peek, recognize, value};
use nom::multi::separated_list1;
use nom::{IResult, Parser};
use std::cmp::Ordering;
use std::cmp::Ordering::{Equal, Greater, Less};
use std::hash::Hash;
use std::str::FromStr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A complex version number with no specific structure.
///
/// Like [`crate::Version`] this is a *descriptive* scheme, but it is based on
/// examples of stupidly crafted, near-lawless version numbers used in the wild.
/// Versions like this are a considerable burden to package management software.
///
/// With `Mess`, groups of letters/numbers are separated by a period, but can be
/// further separated by the symbols `_-+:`.
///
/// Unfortunately, [`Chunk`] cannot be used here, as some developers have
/// numbers like `1.003.04` which make parsers quite sad.
///
/// Some `Mess` values have a shape that is tantalizingly close to a
/// [`crate::SemVer`]. Example: `1.6.0a+2014+m872b87e73dfb-1`. For values like
/// these, we can extract the SemVer-compatible values out with [`Mess::nth`].
///
/// In general this is not guaranteed to have well-defined ordering behaviour,
/// but existing tests show sufficient consistency. [`Mess::nth`] is used
/// internally where appropriate to enhance accuracy.
///
/// # Examples
///
/// ```
/// use versions::{Mess, SemVer, Version};
///
/// let mess = "20.0026.1_0-2+0.93";
///
/// let s = SemVer::new(mess);
/// let v = Version::new(mess);
/// let m = Mess::new(mess);
///
/// assert!(s.is_none());
/// assert!(v.is_none());
/// assert_eq!(Some(mess.to_string()), m.map(|v| format!("{}", v)));
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Eq, Hash, Clone, Default)]
pub struct Mess {
    /// The first section of a `Mess`.
    pub chunks: Vec<MChunk>,
    /// The rest of the `Mess`.
    pub next: Option<(Sep, Box<Mess>)>,
}

impl Mess {
    /// Parse a `Mess` from some input.
    pub fn new<S>(s: S) -> Option<Mess>
    where
        S: AsRef<str>,
    {
        match Mess::parse(s.as_ref()) {
            Ok(("", m)) => Some(m),
            _ => None,
        }
    }

    /// Try to extract a position from the `Mess` as a nice integer, as if it
    /// were a [`crate::SemVer`].
    ///
    /// ```
    /// use versions::Mess;
    ///
    /// let mess = Mess::new("1.6a.0+2014+m872b87e73dfb-1").unwrap();
    /// assert_eq!(Some(1), mess.nth(0));
    /// assert_eq!(None, mess.nth(1));
    /// assert_eq!(Some(0), mess.nth(2));
    ///
    /// let mess = Mess::new("0:1.6a.0+2014+m872b87e73dfb-1").unwrap();
    /// assert_eq!(Some(1), mess.nth(0));
    /// ```
    pub fn nth(&self, x: usize) -> Option<u32> {
        if let Some((Sep::Colon, next)) = self.next.as_ref() {
            next.nth(x)
        } else {
            self.chunks.get(x).and_then(|chunk| match chunk {
                MChunk::Digits(i, _) => Some(*i),
                _ => None,
            })
        }
    }

    /// Like [`Mess::nth`], but tries to parse out a full [`Chunk`] instead.
    pub(crate) fn nth_chunk(&self, x: usize) -> Option<Chunk> {
        let chunk = self.chunks.get(x)?.text();
        let (i, c) = Chunk::parse_without_hyphens(chunk).ok()?;
        match i {
            "" => Some(c),
            _ => None,
        }
    }

    /// The raw `nom` parser for [`Mess`]. Feel free to use this in combination
    /// with other general `nom` parsers.
    pub fn parse(i: &str) -> IResult<&str, Mess> {
        let (i, chunks) = separated_list1(char('.'), MChunk::parse).parse(i)?;
        let (i, next) = opt(Mess::next).parse(i)?;

        let m = Mess {
            chunks,
            next: next.map(|(s, m)| (s, Box::new(m))),
        };

        Ok((i, m))
    }

    fn next(i: &str) -> IResult<&str, (Sep, Mess)> {
        let (i, sep) = Mess::sep(i)?;
        let (i, mess) = Mess::parse(i)?;

        Ok((i, (sep, mess)))
    }

    fn sep(i: &str) -> IResult<&str, Sep> {
        alt((
            value(Sep::Colon, char(':')),
            value(Sep::Hyphen, char('-')),
            value(Sep::Plus, char('+')),
            value(Sep::Underscore, char('_')),
            value(Sep::Tilde, char('~')),
        ))
        .parse(i)
    }
}

impl PartialOrd for Mess {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Build metadata does not affect version precendence, and pre-release versions
/// have *lower* precedence than normal versions.
impl Ord for Mess {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.chunks.cmp(&other.chunks) {
            Equal => {
                let an = self.next.as_ref().map(|(_, m)| m);
                let bn = other.next.as_ref().map(|(_, m)| m);
                an.cmp(&bn)
            }
            ord => ord,
        }
    }
}

impl std::fmt::Display for Mess {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.chunks.iter().join("."))?;

        if let Some((sep, m)) = &self.next {
            write!(f, "{}{}", sep, m)?;
        }

        Ok(())
    }
}

impl FromStr for Mess {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Mess::new(s).ok_or_else(|| Error::IllegalMess(s.to_string()))
    }
}

impl TryFrom<&str> for Mess {
    type Error = Error;

    /// ```
    /// use versions::Mess;
    ///
    /// let orig = "1.2.3.4_123_abc+101a";
    /// let prsd: Mess = orig.try_into().unwrap();
    /// assert_eq!(orig, prsd.to_string());
    /// ```
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Mess::from_str(value)
    }
}

/// Possible values of a section of a [`Mess`].
///
/// A numeric value is extracted if it could be, alongside the original text it
/// came from. This preserves both `Ord` and `Display` behaviour for versions
/// like `1.003.0`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum MChunk {
    /// A nice numeric value.
    Digits(u32, String),
    /// A numeric value preceeded by an `r`, indicating a revision.
    Rev(u32, String),
    /// Anything else.
    Plain(String),
}

impl MChunk {
    /// Extract the original `String`, no matter which variant it parsed into.
    pub fn text(&self) -> &str {
        match self {
            MChunk::Digits(_, s) => s,
            MChunk::Rev(_, s) => s,
            MChunk::Plain(s) => s,
        }
    }

    pub(crate) fn parse(i: &str) -> IResult<&str, MChunk> {
        alt((MChunk::digits, MChunk::rev, MChunk::plain)).parse(i)
    }

    fn digits(i: &str) -> IResult<&str, MChunk> {
        let (i, (u, s)) = map_res(recognize(digit1), |s: &str| {
            s.parse::<u32>().map(|u| (u, s))
        })
        .parse(i)?;
        let (i, _) = alt((peek(recognize(char('.'))), peek(recognize(Mess::sep)), eof)).parse(i)?;
        let chunk = MChunk::Digits(u, s.to_string());
        Ok((i, chunk))
    }

    fn rev(i: &str) -> IResult<&str, MChunk> {
        let (i, _) = tag("r")(i)?;
        let (i, (u, s)) = map_res(recognize(digit1), |s: &str| {
            s.parse::<u32>().map(|u| (u, s))
        })
        .parse(i)?;
        let (i, _) = alt((peek(recognize(char('.'))), peek(recognize(Mess::sep)), eof)).parse(i)?;
        let chunk = MChunk::Rev(u, format!("r{}", s));
        Ok((i, chunk))
    }

    fn plain(i: &str) -> IResult<&str, MChunk> {
        let (i, s) = alphanumeric1(i)?;
        let chunk = MChunk::Plain(s.to_string());
        Ok((i, chunk))
    }
}

impl PartialOrd for MChunk {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MChunk {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            // Normal cases.
            (MChunk::Digits(a, _), MChunk::Digits(b, _)) => a.cmp(b),
            (MChunk::Rev(a, _), MChunk::Rev(b, _)) => a.cmp(b),
            // If I'm a concrete number and you're just a revision, then I'm greater no matter what.
            (MChunk::Digits(_, _), MChunk::Rev(_, _)) => Greater,
            (MChunk::Rev(_, _), MChunk::Digits(_, _)) => Less,
            // There's no sensible pairing, so we fall back to String-based comparison.
            (a, b) => a.text().cmp(b.text()),
        }
    }
}

impl std::fmt::Display for MChunk {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            MChunk::Digits(_, s) => write!(f, "{}", s),
            MChunk::Rev(_, s) => write!(f, "{}", s),
            MChunk::Plain(s) => write!(f, "{}", s),
        }
    }
}

/// Symbols that separate groups of digits/letters in a version number. Used in
/// the [`Mess`].
///
/// These are:
///
/// - A colon (:). Often denotes an "epoch".
/// - A hyphen (-).
/// - A tilde (~). Example: `12.0.0-3ubuntu1~20.04.5`
/// - A plus (+). Stop using this outside of metadata if you are. Example: `10.2+0.93+1-1`
/// - An underscore (_). Stop using this if you are.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Sep {
    /// `:`
    Colon,
    /// `-`
    Hyphen,
    /// `+`
    Plus,
    /// `_`
    Underscore,
    /// `~`
    Tilde,
}

impl std::fmt::Display for Sep {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let c = match self {
            Sep::Colon => ':',
            Sep::Hyphen => '-',
            Sep::Plus => '+',
            Sep::Underscore => '_',
            Sep::Tilde => '~',
        };
        write!(f, "{}", c)
    }
}
