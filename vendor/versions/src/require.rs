//! Constraints on version numbers.

use crate::{Error, Versioning};
use nom::IResult;
use nom::bytes::complete::tag;
use nom::combinator::map;
use nom::{Parser, branch::alt};
use std::str::FromStr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serializer, de::Error as _};

/// [`Versioning`] comparison operators used in a [`Requirement`]: `=`, `>`,
/// `>=`, `<`, `<=`, `~`, `^`, `*`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum Op {
    /// A matching `Versioning` exactly equals the requirement.
    Exact,
    /// A matching `Versioning` must be strictly greater than the requirement.
    Greater,
    /// A matching `Versioning` must be greater than or equal to the requirement.
    GreaterEq,
    /// A matching `Versioning` must be strictly less than the requirement.
    Less,
    /// A matching `Versioning` must be less than or equal to the requirement.
    LessEq,
    /// A matching `Versioning` may have a patch (or last component of the) version
    /// greater than or equal to the requirement.
    Tilde,
    /// A matching `Versioning` has its first non-zero component equal to the
    /// requirement, and all other components greater than or equal to the
    /// requirement.
    Caret,
    /// Any `Versioning` matches the requirement.
    Wildcard,
}

impl std::fmt::Display for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Op::Exact => write!(f, "="),
            Op::Greater => write!(f, ">"),
            Op::GreaterEq => write!(f, ">="),
            Op::Less => write!(f, "<"),
            Op::LessEq => write!(f, "<="),
            Op::Tilde => write!(f, "~"),
            Op::Caret => write!(f, "^"),
            Op::Wildcard => write!(f, "*"),
        }
    }
}

impl Op {
    fn parse(i: &str) -> IResult<&str, Op> {
        // FIXME Use `value` instead of `map`.
        alt((
            map(tag("="), |_| Op::Exact),
            map(tag(">="), |_| Op::GreaterEq),
            map(tag(">"), |_| Op::Greater),
            map(tag("<="), |_| Op::LessEq),
            map(tag("<"), |_| Op::Less),
            map(tag("~"), |_| Op::Tilde),
            map(tag("^"), |_| Op::Caret),
            map(tag("*"), |_| Op::Wildcard),
        ))
        .parse(i)
    }
}

/// A version requirement expression, like `^1.4.163`.
///
/// See also [`Op`] for all possibilities.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Requirement {
    /// The version requirement operation.
    pub op: Op,
    /// The version itself. `None` when `op` is `*`.
    pub version: Option<Versioning>,
}

impl Requirement {
    /// Parse a new `Requirement` from a string.
    pub fn new(s: &str) -> Option<Self> {
        match Requirement::parse(s) {
            Ok(("", r)) => Some(r),
            _ => None,
        }
    }

    /// Does this [`Requirement`] succeed on a tilde-match with another version?
    ///
    /// A tilde match is defined as a match where the major and minor versions
    /// are equal and the patch version is greater than or equal. For non-semver
    /// conformant `Versioning`s, this match extends the rule such that the last
    /// part of the version is greater than or equal.
    fn matches_tilde(&self, other: &Versioning) -> bool {
        self.version
            .as_ref()
            .is_some_and(|v| v.matches_tilde(other))
    }

    /// Does this [`Requirement`] succeed on a caret-matche with another version?
    ///
    /// A caret match is defined as a match where the first non-zero part of the
    /// version is equal and the remaining parts are greater than or equal.
    fn matches_caret(&self, other: &Versioning) -> bool {
        self.version
            .as_ref()
            .is_some_and(|v| v.matches_caret(other))
    }

    /// Check if a version matches a version constraint.
    ///
    /// ```rust
    /// use versions::{Requirement, Versioning};
    /// use std::str::FromStr;
    ///
    /// let gt = Requirement::from_str(">=1.0.0").unwrap();
    /// assert!(gt.matches(&Versioning::new("1.0.0").unwrap()));
    /// assert!(gt.matches(&Versioning::new("1.1.0").unwrap()));
    /// assert!(!gt.matches(&Versioning::new("0.9.0").unwrap()));
    ///
    /// let wild = Requirement::from_str("*").unwrap();
    /// assert!(wild.matches(&Versioning::new("1.0.0").unwrap()));
    /// assert!(wild.matches(&Versioning::new("1.1.0").unwrap()));
    /// assert!(wild.matches(&Versioning::new("0.9.0").unwrap()));
    ///
    /// let constraint_eq = Requirement::from_str("=1.0.0").unwrap();
    /// assert!(constraint_eq.matches(&Versioning::new("1.0.0").unwrap()));
    /// assert!(!constraint_eq.matches(&Versioning::new("1.1.0").unwrap()));
    /// assert!(!constraint_eq.matches(&Versioning::new("0.9.0").unwrap()));
    /// ```
    pub fn matches(&self, other: &Versioning) -> bool {
        if let Some(version) = &self.version {
            match self.op {
                Op::Exact => other == version,
                Op::Greater => other > version,
                Op::GreaterEq => other >= version,
                Op::Less => other < version,
                Op::LessEq => other <= version,
                Op::Tilde => self.matches_tilde(other),
                Op::Caret => self.matches_caret(other),
                Op::Wildcard => true,
            }
        } else {
            matches!(self.op, Op::Wildcard)
        }
    }

    /// The raw `nom` parser for [`Requirement`]. Feel free to use this in
    /// combination with other general `nom` parsers.
    pub fn parse(i: &str) -> IResult<&str, Requirement> {
        let (i, op) = Op::parse(i)?;

        let (i, req) = match op {
            Op::Wildcard => {
                let req = Requirement { op, version: None };
                (i, req)
            }
            _ => {
                let (i, vr) = Versioning::parse(i)?;

                let req = Requirement {
                    op,
                    version: Some(vr),
                };

                (i, req)
            }
        };

        Ok((i, req))
    }

    #[cfg(feature = "serde")]
    /// Function suitable for use as a serde deserializer for `Requirement` where
    /// `Requirement` is the type of a field in a struct.
    ///
    /// ```rust
    /// use versions::Requirement;
    /// use serde::Deserialize;
    /// use serde_json::from_str;
    ///
    /// #[derive(Deserialize)]
    /// struct Foo {
    ///    #[serde(deserialize_with = "Requirement::deserialize")]
    ///    requirement: Requirement,
    ///    // ...
    /// }
    ///
    /// let foo: Foo = from_str(r#"{"requirement": ">=1.0.0"}"#).unwrap();
    /// ```
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Requirement, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;

        s.parse().map_err(D::Error::custom)
    }

    #[cfg(feature = "serde")]
    /// Function suitable for use as a custom serde serializer for
    /// the `Requirment` type.
    ///
    /// ```rust
    /// use versions::Requirement;
    /// use serde::Serialize;
    /// use serde_json::to_string;
    ///
    /// #[derive(Serialize)]
    /// struct Foo {
    ///    #[serde(serialize_with = "Requirement::serialize")]
    ///    requirement: Requirement,
    ///    // ...
    /// }
    ///
    /// ```
    pub fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s: String = self.to_string();
        serializer.serialize_str(&s)
    }
}

impl FromStr for Requirement {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Requirement::new(s).ok_or_else(|| Error::IllegalOp(s.to_string()))
    }
}

impl Default for Requirement {
    fn default() -> Self {
        Requirement {
            op: Op::Wildcard,
            version: None,
        }
    }
}

impl std::fmt::Display for Requirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = self
            .version
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_default();
        write!(f, "{}{}", self.op, version,)
    }
}
