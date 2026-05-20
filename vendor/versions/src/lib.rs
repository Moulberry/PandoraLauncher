//! A library for parsing and comparing software version numbers.
//!
//! We like to give version numbers to our software in a myriad of different
//! ways. Some ways follow strict guidelines for incrementing and comparison.
//! Some follow conventional wisdom and are generally self-consistent. Some are
//! just plain asinine. This library provides a means of parsing and comparing
//! *any* style of versioning, be it a nice Semantic Version like this:
//!
//! > 1.2.3-r1
//!
//! ...or a monstrosity like this:
//!
//! > 2:10.2+0.0093r3+1-1
//!
//! # Usage
//!
//! If you're parsing several version numbers that don't follow a single scheme
//! (say, as in system packages), then use the [`Versioning`] type and its
//! parser [`Versioning::new`]. Otherwise, each main type - [`SemVer`],
//! [`Version`], or [`Mess`] - can be parsed on their own via the `new` method
//! (e.g. [`SemVer::new`]).
//!
//! # Examples
//!
//! ```
//! use versions::Versioning;
//!
//! let good = Versioning::new("1.6.0").unwrap();
//! let evil = Versioning::new("1.6.0a+2014+m872b87e73dfb-1").unwrap();
//!
//! assert!(good.is_ideal());   // It parsed as a `SemVer`.
//! assert!(evil.is_complex()); // It parsed as a `Mess`.
//! assert!(good > evil);       // We can compare them anyway!
//! ```
//!
//! # Version Constraints
//!
//! Tools like `cargo` also allow version constraints to be prepended to a
//! version number, like in `^1.2.3`.
//!
//! ```
//! use versions::{Requirement, Versioning};
//!
//! let req = Requirement::new("^1.2.3").unwrap();
//! let ver = Versioning::new("1.2.4").unwrap();
//! assert!(req.matches(&ver));
//! ```
//!
//! In this case, the incoming version `1.2.4` satisfies the "caret" constraint,
//! which demands anything greater than or equal to `1.2.3`.
//!
//! See the [`Requirement`] type for more details.
//!
//! # Usage with `nom`
//!
//! In constructing your own [`nom`](https://lib.rs/nom) parsers, you can
//! integrate the parsers used for the types in this crate via
//! [`Versioning::parse`], [`SemVer::parse`], [`Version::parse`], and
//! [`Mess::parse`].
//!
//! # Features
//!
//! You can enable [`Serde`](https://serde.rs/) support for serialization and
//! deserialization with the `serde` feature.
//!
//! By default the version structs are serialized/deserialized as-is. If instead
//! you'd like to deserialize directly from a raw version string like `1.2.3`,
//! see [`Versioning::deserialize_pretty`].

#![allow(clippy::many_single_char_names)]
#![warn(missing_docs)]

mod mess;
mod parsers;
mod require;
mod semver;
mod version;
mod versioning;

pub use mess::{MChunk, Mess, Sep};
pub use require::{Op, Requirement};
pub use semver::SemVer;
pub use version::Version;
pub use versioning::Versioning;

use itertools::EitherOrBoth::{Both, Left, Right};
use itertools::Itertools;
use nom::branch::alt;
use nom::character::complete::char;
use nom::combinator::{fail, map};
use nom::multi::separated_list1;
use nom::{IResult, Parser};
use parsers::{alphanums, hyphenated_alphanums, unsigned};
use std::cmp::Ordering;
use std::cmp::Ordering::{Equal, Greater, Less};
use std::hash::Hash;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Errors unique to the parsing of version numbers.
#[derive(Debug, Clone)]
pub enum Error {
    /// Some string failed to parse into a [`SemVer`] via functions like
    /// [`std::str::FromStr::from_str`] or [`TryFrom::try_from`].
    IllegalSemver(String),
    /// Some string failed to parse into a [`Version`].
    IllegalVersion(String),
    /// Some string failed to parse into a [`Mess`].
    IllegalMess(String),
    /// Some string failed to parse into a [`Versioning`].
    IllegalVersioning(String),
    /// An operator failed to parse into a [`Op`].
    IllegalOp(String),
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::IllegalSemver(s) => write!(f, "Illegal SemVer: {s}"),
            Error::IllegalVersion(s) => write!(f, "Illegal Version: {s}"),
            Error::IllegalMess(s) => write!(f, "Illegal Mess: {s}"),
            Error::IllegalVersioning(s) => write!(f, "Illegal Versioning: {s}"),
            Error::IllegalOp(s) => write!(f, "Illegal Op: {s}"),
        }
    }
}

/// [`Chunk`]s that have comparison behaviour according to SemVer's rules for
/// prereleases.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Release(pub Vec<Chunk>);

impl Release {
    fn parse(i: &str) -> IResult<&str, Release> {
        let (i, _) = char('-')(i)?;
        map(separated_list1(char('.'), Chunk::parse), Release).parse(i)
    }
}

impl PartialOrd for Release {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Release {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .iter()
            .zip_longest(&other.0)
            .find_map(|eob| match eob {
                Both(a, b) => match a.cmp_semver(b) {
                    Less => Some(Less),
                    Greater => Some(Greater),
                    Equal => None,
                },
                // From the Semver spec: A larger set of pre-release fields has
                // a higher precedence than a smaller set, if all the preceding
                // identifiers are equal.
                Left(_) => Some(Greater),
                Right(_) => Some(Less),
            })
            .unwrap_or(Equal)
    }
}

impl std::fmt::Display for Release {
    // FIXME Fri Jan  7 11:44:50 2022
    //
    // Use `itersperse` here once it stabilises.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.0.as_slice() {
            [] => Ok(()),
            [c] => write!(f, "{}", c),
            [c, rest @ ..] => {
                write!(f, "{}", c)?;

                for r in rest {
                    write!(f, ".{}", r)?;
                }

                Ok(())
            }
        }
    }
}

/// [`Chunk`]s that have a comparison behaviour specific to [`Version`].
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Eq, Hash, Clone, Default)]
pub struct Chunks(pub Vec<Chunk>);

impl Chunks {
    // Intended for parsing a `Version`.
    fn parse(i: &str) -> IResult<&str, Chunks> {
        map(
            separated_list1(char('.'), Chunk::parse_without_hyphens),
            Chunks,
        )
        .parse(i)
    }
}

impl PartialOrd for Chunks {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Chunks {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .iter()
            .zip_longest(&other.0)
            .find_map(|eob| match eob {
                Both(a, b) => match a.cmp_lenient(b) {
                    Less => Some(Less),
                    Greater => Some(Greater),
                    Equal => None,
                },
                // From the Semver spec: A larger set of pre-release fields has
                // a higher precedence than a smaller set, if all the preceding
                // identifiers are equal.
                Left(_) => Some(Greater),
                Right(_) => Some(Less),
            })
            .unwrap_or(Equal)
    }
}

impl std::fmt::Display for Chunks {
    // FIXME Fri Jan  7 11:44:50 2022
    //
    // Use `itersperse` here once it stabilises.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.0.as_slice() {
            [] => Ok(()),
            [c] => write!(f, "{}", c),
            [c, rest @ ..] => {
                write!(f, "{}", c)?;

                for r in rest {
                    write!(f, ".{}", r)?;
                }

                Ok(())
            }
        }
    }
}

/// A logical unit of a version number.
///
/// Either entirely numerical (with no leading zeroes) or entirely
/// alphanumerical (with a free mixture of numbers, letters, and hyphens).
///
/// Groups of these (like [`Release`]) are separated by periods to form a full
/// section of a version number.
///
/// # Examples
///
/// - `1`
/// - `20150826`
/// - `r3`
/// - `0rc1-abc3`
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Chunk {
    /// A nice, pure number.
    Numeric(u32),
    /// A mixture of letters, numbers, and hyphens.
    Alphanum(String),
}

impl Chunk {
    /// If this `Chunk` is made up of a single digit, then pull out the inner
    /// value.
    ///
    /// ```
    /// use versions::Chunk;
    ///
    /// let v = Chunk::Numeric(1);
    /// assert_eq!(Some(1), v.single_digit());
    ///
    /// let v = Chunk::Alphanum("abc".to_string());
    /// assert_eq!(None, v.single_digit());
    ///
    /// let v = Chunk::Alphanum("1abc".to_string());
    /// assert_eq!(None, v.single_digit());
    /// ```
    pub fn single_digit(&self) -> Option<u32> {
        match self {
            Chunk::Numeric(n) => Some(*n),
            Chunk::Alphanum(_) => None,
        }
    }

    /// Like [`Chunk::single_digit`], but will grab a leading `u32` even if
    /// followed by letters.
    ///
    /// ```
    /// use versions::Chunk;
    ///
    /// let v = Chunk::Numeric(1);
    /// assert_eq!(Some(1), v.single_digit_lenient());
    ///
    /// let v = Chunk::Alphanum("abc".to_string());
    /// assert_eq!(None, v.single_digit_lenient());
    ///
    /// let v = Chunk::Alphanum("1abc".to_string());
    /// assert_eq!(Some(1), v.single_digit_lenient());
    /// ```
    pub fn single_digit_lenient(&self) -> Option<u32> {
        match self {
            Chunk::Numeric(n) => Some(*n),
            Chunk::Alphanum(s) => unsigned(s).ok().map(|(_, n)| n),
        }
    }

    /// Like [`Chunk::single_digit`], but will grab a trailing `u32`.
    ///
    /// ```
    /// use versions::Chunk;
    ///
    /// let v = Chunk::Alphanum("r23".to_string());
    /// assert_eq!(Some(23), v.single_digit_lenient_post());
    /// ```
    pub fn single_digit_lenient_post(&self) -> Option<u32> {
        match self {
            Chunk::Numeric(n) => Some(*n),
            Chunk::Alphanum(s) => {
                // FIXME 2024-08-05 `strip_prefix` may be too aggressive. Should
                // we only strip one char instead?
                s.strip_prefix(|c: char| c.is_ascii_alphabetic())
                    .and_then(|stripped| unsigned(stripped).ok())
                    .map(|(_, n)| n)
            }
        }
    }

    fn parse(i: &str) -> IResult<&str, Chunk> {
        alt((Chunk::alphanum, Chunk::numeric)).parse(i)
    }

    fn parse_without_hyphens(i: &str) -> IResult<&str, Chunk> {
        alt((Chunk::alphanum_without_hyphens, Chunk::numeric)).parse(i)
    }

    // A clever interpretation of the grammar of "alphanumeric identifier".
    // Instead of having a big, composed parser that structurally accounts for
    // the presence of a "non-digit", we just check for one after the fact.
    fn alphanum(i: &str) -> IResult<&str, Chunk> {
        let (i2, ids) = hyphenated_alphanums(i)?;

        if ids.contains(|c: char| c.is_ascii_alphabetic() || c == '-') {
            Ok((i2, Chunk::Alphanum(ids.to_string())))
        } else {
            fail().parse(i)
        }
    }

    fn alphanum_without_hyphens(i: &str) -> IResult<&str, Chunk> {
        let (i2, ids) = alphanums(i)?;

        if ids.contains(|c: char| c.is_ascii_alphabetic()) {
            Ok((i2, Chunk::Alphanum(ids.to_string())))
        } else {
            fail().parse(i)
        }
    }

    fn numeric(i: &str) -> IResult<&str, Chunk> {
        map(unsigned, Chunk::Numeric).parse(i)
    }

    fn mchunk(&self) -> MChunk {
        // FIXME Fri Jan  7 12:34:24 2022
        //
        // Is there going to be an issue here, having not accounted for an `r`?
        //
        // 2023-08-05
        // This actually just came up in Aura, but for Versions.
        match self {
            Chunk::Numeric(n) => MChunk::Digits(*n, n.to_string()),
            Chunk::Alphanum(s) => MChunk::Plain(s.clone()),
        }
        // match self.0.as_slice() {
        //     [] => None,
        //     [Unit::Digits(u)] => Some(MChunk::Digits(*u, u.to_string())),
        //     [Unit::Letters(s), Unit::Digits(u)] if s == "r" => {
        //         Some(MChunk::Rev(*u, format!("r{}", u)))
        //     }
        //     [Unit::Letters(s)] => Some(MChunk::Plain(s.clone())),
        //     _ => Some(MChunk::Plain(format!("{}", self))),
        // }
    }

    fn cmp_semver(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Chunk::Numeric(a), Chunk::Numeric(b)) => a.cmp(b),
            (Chunk::Numeric(_), Chunk::Alphanum(_)) => Less,
            (Chunk::Alphanum(_), Chunk::Numeric(_)) => Greater,
            (Chunk::Alphanum(a), Chunk::Alphanum(b)) => a.cmp(b),
        }
    }

    fn cmp_lenient(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Chunk::Numeric(a), Chunk::Numeric(b)) => a.cmp(b),
            (a @ Chunk::Alphanum(x), b @ Chunk::Alphanum(y)) => {
                match (x.chars().next(), y.chars().next()) {
                    (Some(xc), Some(yc)) if xc.is_ascii_alphabetic() && xc == yc => {
                        match (a.single_digit_lenient_post(), b.single_digit_lenient_post()) {
                            // r8 < r23
                            (Some(m), Some(n)) => m.cmp(&n),
                            _ => x.cmp(y),
                        }
                    }
                    (Some(xc), Some(yc)) if xc.is_ascii_digit() && yc.is_ascii_digit() => {
                        match (a.single_digit_lenient(), b.single_digit_lenient()) {
                            // 0rc1 < 1rc1
                            (Some(i), Some(j)) => i.cmp(&j),
                            _ => x.cmp(y),
                        }
                    }
                    _ => x.cmp(y),
                }
            }
            (Chunk::Numeric(n), b @ Chunk::Alphanum(_)) => match b.single_digit_lenient() {
                None => Greater,
                Some(m) => match n.cmp(&m) {
                    // 1.2.0 > 1.2.0rc1
                    Equal => Greater,
                    c => c,
                },
            },
            (a @ Chunk::Alphanum(_), Chunk::Numeric(n)) => match a.single_digit_lenient() {
                None => Less,
                Some(m) => match m.cmp(n) {
                    // 1.2.0rc1 < 1.2.0
                    Equal => Less,
                    c => c,
                },
            },
        }
    }
}

impl std::fmt::Display for Chunk {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Chunk::Numeric(n) => write!(f, "{}", n),
            Chunk::Alphanum(a) => write!(f, "{}", a),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn chanks() {
        assert_eq!(Ok(("", Chunk::Numeric(123))), Chunk::parse("123"));
        assert_eq!(
            Ok(("", Chunk::Alphanum("123a".to_string()))),
            Chunk::parse("123a")
        );
        assert_eq!(
            Ok(("", Chunk::Alphanum("123-456".to_string()))),
            Chunk::parse("123-456")
        );
        assert_eq!(
            Ok(("", Chunk::Alphanum("00a".to_string()))),
            Chunk::parse("00a")
        );
    }

    #[test]
    fn mess_chunks() {
        assert_eq!(
            Ok(("", MChunk::Digits(1, "1".to_owned()))),
            MChunk::parse("1")
        );

        assert_eq!(
            Ok(("", MChunk::Digits(1, "01".to_owned()))),
            MChunk::parse("01")
        );

        assert_eq!(
            Ok(("", MChunk::Digits(987, "987".to_owned()))),
            MChunk::parse("987")
        );

        assert_eq!(
            Ok(("", MChunk::Rev(1, "r1".to_owned()))),
            MChunk::parse("r1")
        );

        assert_eq!(
            Ok(("", MChunk::Rev(1, "r01".to_owned()))),
            MChunk::parse("r01")
        );

        assert_eq!(
            Ok(("", MChunk::Rev(987, "r987".to_owned()))),
            MChunk::parse("r987")
        );

        assert_eq!(
            Ok(("", MChunk::Plain("abcd".to_owned()))),
            MChunk::parse("abcd")
        );

        assert_eq!(
            Ok(("", MChunk::Plain("1r3".to_owned()))),
            MChunk::parse("1r3")
        );

        assert_eq!(
            Ok(("", MChunk::Plain("alpha0".to_owned()))),
            MChunk::parse("alpha0")
        );
    }

    #[test]
    fn parse_mess() {
        assert_eq!(
            Ok((
                "",
                Mess {
                    chunks: vec![MChunk::Digits(1, "1".to_owned()),],
                    next: None
                }
            )),
            Mess::parse("1")
        );
    }

    #[test]
    fn official_semvers() {
        let goods = vec![
            "0.1.0",
            "1.2.3",
            "1.2.3-1",
            "1.2.3-alpha",
            "1.2.3-alpha.2",
            "1.2.3+a1b2c3.1",
            "1.2.3-alpha.2+a1b2c3.1",
            "1.0.0-x-y-z.-",
            "1.0.0-alpha+001",
            "1.0.0+21AF26D3---117B344092BD",
        ];

        for s in goods {
            assert_eq!(
                Some(s.to_string()),
                SemVer::new(s).map(|sv| format!("{}", sv))
            )
        }
    }

    #[test]
    fn good_semvers() {
        let goods = vec![
            "0.4.8-1",
            "7.42.13-4",
            "2.1.16102-2",
            "2.2.1-b05",
            "1.11.0+20200830-1",
        ];

        for s in goods {
            assert_eq!(
                Some(s.to_string()),
                SemVer::new(s).map(|sv| format!("{}", sv))
            )
        }
    }

    #[test]
    fn tricky_semvers() {
        let v = "1.2.2-00a";

        assert_eq!("", SemVer::parse(v).unwrap().0);
    }

    #[test]
    fn bad_semvers() {
        let bads = vec![
            "1",
            "1.2",
            "a.b.c",
            "1.01.1",
            "1.2.3+a1b!2c3.1",
            "",
            "1.2.3 ",
        ];

        bads.iter().for_each(|s| assert_eq!(None, SemVer::new(s)));
    }

    #[test]
    /// The exact example from http://semver.org
    fn semver_ord() {
        let svs = vec![
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
        ];

        for (a, b) in svs.iter().zip(&svs[1..]) {
            let x = SemVer::new(a).unwrap();
            let y = SemVer::new(b).unwrap();

            assert!(x < y, "{} < {}", x, y);
        }
    }

    #[test]
    fn good_versions() {
        let goods = vec![
            "1",
            "1.2",
            "1.0rc0",
            "1.0rc1",
            "1.1rc1",
            "44.0.2403.157-1",
            "0.25-2",
            "8.u51-1",
            "21-2",
            "7.1p1-1",
            "20150826-1",
            "1:0.10.16-3",
            "8.64.0.81-1",
            "1:3.20-1",
        ];

        for s in goods {
            assert!(SemVer::new(s).is_none(), "Shouldn't be SemVer: {}", s);
            assert_eq!(
                Some(s.to_string()),
                Version::new(s).map(|v| format!("{}", v))
            )
        }
    }

    #[test]
    fn bad_versions() {
        let bads = vec!["", "1.2 "];

        bads.iter().for_each(|b| assert_eq!(None, Version::new(b)));
    }

    #[test]
    fn version_ord() {
        let vs = vec!["0.9.9.9", "1.0.0.0", "1.0.0.1", "2"];

        for (a, b) in vs.iter().zip(&vs[1..]) {
            cmp_versions(a, b);
        }

        cmp_versions("1.2-5", "1.2.3-1");
        cmp_versions("1.0rc1", "1.0");
        cmp_versions("1.0", "1:1.0");
        cmp_versions("1.1", "1:1.0");
        cmp_versions("1.1", "1:1.1");
    }

    // https://github.com/fosskers/rs-versions/issues/25
    #[test]
    fn versions_25() {
        let bad = Versioning::new("0:8.7p1-38.el9").unwrap();
        let good = Versioning::new("0:8.7p1-38.el9_4.1").unwrap();

        assert!(bad.is_general());
        assert!(good.is_complex());
        assert!(good > bad);
    }

    // https://github.com/fosskers/rs-versions/issues/29
    //
    // If a `Chunk` goes longer than `u32`, we can't parse it as a `SemVer`. I
    // decided to mark this "wontfix", as accounting for it increases complexity
    // and might screw up comparison precedence if something that is actually
    // true a number is treated instead as a string.
    #[test]
    fn versions_29() {
        let bad = Versioning::new("0.0.0-0.1730239248325").unwrap();
        assert!(bad.is_complex());
    }

    // https://github.com/fosskers/aura/issues/876
    #[test]
    fn aura_876() {
        let x = Versioning::new("2.14.r8.g28399bf-1").unwrap();
        let y = Versioning::new("2.14.r23.ga07d3df-1").unwrap();
        assert!(x.is_general());
        assert!(y.is_general());
        cmp_versions("2.14.r8.g28399bf-1", "2.14.r23.ga07d3df-1");
        cmp_versions("2.14.r23.ga07d3df-1", "2.14.8");
    }

    fn cmp_versions(a: &str, b: &str) {
        let x = Version::new(a).unwrap();
        let y = Version::new(b).unwrap();

        assert!(x < y, "{} < {}", x, y);
        assert!(y > x, "{} > {}", y, x);
    }

    #[test]
    fn good_messes() {
        let messes = vec![
            "10.2+0.93+1-1",
            "003.03-3",
            "002.000-7",
            "20.26.1_0-2",
            "1.6.0a+2014+m872b87e73dfb-1",
            "0.17.0+r8+gc41db5f1-1",
            "0.17.0+r157+g584760cf-1",
            "1.002.3+r003",
            "1.3.00.16851-1",
            "5.2.458699.0906-1",
            "12.0.0-3ubuntu1~20.04.5",
        ];

        for s in messes {
            let sv = SemVer::new(s);
            let vr = Version::new(s);
            assert!(sv.is_none(), "Shouldn't be SemVer: {} -> {:#?}", s, sv);
            assert!(vr.is_none(), "Shouldn't be Version: {} -> {:#?}", s, vr);
            assert_eq!(Some(s.to_string()), Mess::new(s).map(|v| format!("{}", v)));
        }
    }

    #[test]
    fn bad_messes() {
        let bads = vec!["", "003.03-3 "];

        bads.iter().for_each(|b| assert_eq!(None, Mess::new(b)));
    }

    #[test]
    fn mess_ord() {
        let messes = vec![
            "10.2+0.93+1-1",
            "10.2+0.93+1-2",
            "10.2+0.93+2-1",
            "10.2+0.94+1-1",
            "10.3+0.93+1-1",
            "11.2+0.93+1-1",
            "12",
        ];

        for (a, b) in messes.iter().zip(&messes[1..]) {
            cmp_messes(a, b);
        }
    }

    #[test]
    fn mess_7zip() {
        cmp_messes("22.01-ZS-v1.5.5-R2", "22.01-ZS-v1.5.6-R2");
        cmp_messes("22.01-ZS-v1.5.5-R2", "24.02-ZS-v1.6.0");
    }

    fn cmp_messes(a: &str, b: &str) {
        let x = Mess::new(a).unwrap();
        let y = Mess::new(b).unwrap();

        assert!(x < y, "{} < {}", x, y);
        assert!(y > x, "{} > {}", y, x);
    }

    #[test]
    fn equality() {
        let vers = vec![
            "1:3.20.1-1",
            "1.3.00.25560-1",
            "150_28-3",
            "1.0.r15.g3fc772c-5",
            "0.88-2",
        ];

        for v in vers {
            let x = Versioning::new(v).unwrap();

            assert_eq!(Equal, x.cmp(&x));
        }
    }

    #[test]
    fn mixed_comparisons() {
        cmp_versioning("1.2.3", "1.2.3.0");
        cmp_versioning("1.2.3.git", "1.2.3");
        cmp_versioning("1.2.2r1-1", "1.2.3-1");
        cmp_versioning("1.2.3-1", "1.2.4r1-1");
        cmp_versioning("1.2.3-1", "2+0007-1");
        cmp_versioning("1.2.3r1-1", "2+0007-1");
        cmp_versioning("1.2-5", "1.2.3-1");
        cmp_versioning("1.6.0a+2014+m872b87e73dfb-1", "1.6.0-1");
        cmp_versioning("1.11.0.git.20200404-1", "1.11.0+20200830-1");
        cmp_versioning("0.17.0+r8+gc41db5f1-1", "0.17.0+r157+g584760cf-1");
        cmp_versioning("2.2.3", "10e");
        cmp_versioning("e.2.3", "1.2.3");
        cmp_versioning("0.4.8-1", "0.4.9-1");
        cmp_versioning("2.1.16102-2", "2.1.17627-1");
        cmp_versioning("8.64.0.81-1", "8.65.0.78-1");
        cmp_versioning("1.3.00.16851-1", "1.3.00.25560-1");
        cmp_versioning("1:3.20-1", "1:3.20.1-1");
        cmp_versioning("5.2.458699.0906-1", "5.3.472687.1012-1");
        cmp_versioning("1.2.3", "1:1.2.0");
    }

    fn cmp_versioning(a: &str, b: &str) {
        let x = Versioning::new(a).unwrap();
        let y = Versioning::new(b).unwrap();

        assert!(
            x < y,
            "\nAttempted: {} < {}\nLesser: {:?}\nGreater: {:?}\nThinks: {:?}",
            x,
            y,
            x,
            y,
            x.cmp(&y)
        );
    }

    #[test]
    fn parsing_sanity() {
        assert_eq!(Ok(34), "0034".parse::<u32>())
    }

    #[test]
    fn test_eq() {
        assert!(
            Requirement::from_str("=1.0.0")
                .unwrap()
                .matches(&Versioning::new("1.0.0").unwrap())
        );
        assert!(
            Requirement::from_str("=1.1.0")
                .unwrap()
                .matches(&Versioning::new("1.1.0").unwrap())
        );
        assert!(
            Requirement::from_str("=0.9.0")
                .unwrap()
                .matches(&Versioning::new("0.9.0").unwrap())
        );
        assert!(
            Requirement::from_str("=6.0.pre134")
                .unwrap()
                .matches(&Versioning::new("6.0.pre134").unwrap())
        );
        assert!(
            Requirement::from_str("=6.0.166")
                .unwrap()
                .matches(&Versioning::new("6.0.166").unwrap())
        );
    }

    #[test]
    fn test_wild() {
        let wild = Requirement::from_str("*").unwrap();
        assert!(wild.matches(&Versioning::new("1.0.0").unwrap()));
        assert!(wild.matches(&Versioning::new("1.1.0").unwrap()));
        assert!(wild.matches(&Versioning::new("0.9.0").unwrap()));
        assert!(wild.matches(&Versioning::new("6.0.pre134").unwrap()));
        assert!(wild.matches(&Versioning::new("6.0.166").unwrap()));
    }

    #[test]
    fn test_gt() {
        let gt = Requirement::from_str(">1.1.1").unwrap();

        assert!(!gt.matches(&Versioning::new("1.1.1").unwrap()));
        assert!(gt.matches(&Versioning::new("2.2.2").unwrap()));
        assert!(gt.matches(&Versioning::new("2.0.0").unwrap()));
        assert!(gt.matches(&Versioning::new("1.2.0").unwrap()));
        assert!(gt.matches(&Versioning::new("1.1.2").unwrap()));
        assert!(!gt.matches(&Versioning::new("0.9.9").unwrap()));
        assert!(!gt.matches(&Versioning::new("0.1.1").unwrap()));
        assert!(!gt.matches(&Versioning::new("1.0.0").unwrap()));
        assert!(!gt.matches(&Versioning::new("1.1.0").unwrap()));
    }

    #[test]
    fn test_lt() {
        let lt = Requirement::from_str("<1.1.1").unwrap();
        assert!(!lt.matches(&Versioning::new("1.1.1").unwrap()));
        assert!(!lt.matches(&Versioning::new("2.2.2").unwrap()));
        assert!(!lt.matches(&Versioning::new("2.0.0").unwrap()));
        assert!(!lt.matches(&Versioning::new("1.2.0").unwrap()));
        assert!(!lt.matches(&Versioning::new("1.1.2").unwrap()));
        assert!(lt.matches(&Versioning::new("0.9.9").unwrap()));
        assert!(lt.matches(&Versioning::new("0.1.1").unwrap()));
        assert!(lt.matches(&Versioning::new("1.0.0").unwrap()));
        assert!(lt.matches(&Versioning::new("1.1.0").unwrap()));
    }

    #[test]
    fn test_gte() {
        let gte = Requirement::from_str(">=1.1.1").unwrap();
        assert!(gte.matches(&Versioning::new("1.1.1").unwrap()));
        assert!(gte.matches(&Versioning::new("2.2.2").unwrap()));
        assert!(gte.matches(&Versioning::new("2.0.0").unwrap()));
        assert!(gte.matches(&Versioning::new("1.2.0").unwrap()));
        assert!(gte.matches(&Versioning::new("1.1.2").unwrap()));
        assert!(!gte.matches(&Versioning::new("0.9.9").unwrap()));
        assert!(!gte.matches(&Versioning::new("0.1.1").unwrap()));
        assert!(!gte.matches(&Versioning::new("1.0.0").unwrap()));
        assert!(!gte.matches(&Versioning::new("1.1.0").unwrap()));
    }

    #[test]
    fn test_lte() {
        let lte = Requirement::from_str("<=1.1.1").unwrap();
        assert!(lte.matches(&Versioning::new("1.1.1").unwrap()));
        assert!(!lte.matches(&Versioning::new("2.2.2").unwrap()));
        assert!(!lte.matches(&Versioning::new("2.0.0").unwrap()));
        assert!(!lte.matches(&Versioning::new("1.2.0").unwrap()));
        assert!(!lte.matches(&Versioning::new("1.1.2").unwrap()));
        assert!(lte.matches(&Versioning::new("0.9.9").unwrap()));
        assert!(lte.matches(&Versioning::new("0.1.1").unwrap()));
        assert!(lte.matches(&Versioning::new("1.0.0").unwrap()));
        assert!(lte.matches(&Versioning::new("1.1.0").unwrap()));
    }

    #[test]
    fn test_tilde() {
        let tilde = Requirement::from_str("~1.1.1").unwrap();
        assert!(tilde.matches(&Versioning::new("1.1.1").unwrap()));
        assert!(tilde.matches(&Versioning::new("1.1.2").unwrap()));
        assert!(tilde.matches(&Versioning::new("1.1.3").unwrap()));
        assert!(!tilde.matches(&Versioning::new("1.2.0").unwrap()));
        assert!(!tilde.matches(&Versioning::new("2.0.0").unwrap()));
        assert!(!tilde.matches(&Versioning::new("2.2.2").unwrap()));
        assert!(!tilde.matches(&Versioning::new("0.9.9").unwrap()));
        assert!(!tilde.matches(&Versioning::new("0.1.1").unwrap()));
        assert!(!tilde.matches(&Versioning::new("1.0.0").unwrap()));
    }

    #[test]
    fn test_caret() {
        let caret = Requirement::from_str("^1.1.1").unwrap();
        assert!(caret.matches(&Versioning::new("1.1.1").unwrap()));
        assert!(caret.matches(&Versioning::new("1.1.2").unwrap()));
        assert!(caret.matches(&Versioning::new("1.1.3").unwrap()));
        assert!(caret.matches(&Versioning::new("1.2.0").unwrap()));
        assert!(!caret.matches(&Versioning::new("2.0.0").unwrap()));
        assert!(!caret.matches(&Versioning::new("2.2.2").unwrap()));
        assert!(!caret.matches(&Versioning::new("0.9.9").unwrap()));
        assert!(!caret.matches(&Versioning::new("0.1.1").unwrap()));
        assert!(!caret.matches(&Versioning::new("1.0.0").unwrap()));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_deserialize() {
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct DeserializableVersioning {
            #[serde(deserialize_with = "Versioning::deserialize_pretty")]
            version: Versioning,
        }

        let deserializable: DeserializableVersioning =
            serde_json::from_str(r#"{"version": "1.2.3"}"#).unwrap();

        assert_eq!(deserializable.version, Versioning::new("1.2.3").unwrap());

        #[derive(Deserialize)]
        struct DeserializableRequirement {
            #[serde(deserialize_with = "Requirement::deserialize")]
            version: Requirement,
        }

        let deserializable: DeserializableRequirement =
            serde_json::from_str(r#"{"version": ">=1.2.3"}"#).unwrap();

        assert_eq!(
            deserializable.version,
            Requirement {
                op: Op::GreaterEq,
                version: Some(Versioning::new("1.2.3").unwrap())
            }
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_serialize() {
        use serde::Serialize;

        #[derive(Serialize)]
        struct SerializableRequirement {
            #[serde(serialize_with = "Requirement::serialize")]
            req: Requirement,
        }

        let test_object = SerializableRequirement {
            req: Requirement::from_str(">=1.2.3").unwrap(),
        };

        let string = serde_json::to_string(&test_object).unwrap();

        assert_eq!(string, "{\"req\":\">=1.2.3\"}");
    }
}
