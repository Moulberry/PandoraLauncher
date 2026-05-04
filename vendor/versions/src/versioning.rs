//! Types and logic for handling combinde [`Versioning`]s.

use std::cmp::Ordering;

use crate::{Error, Mess, SemVer, Version};
use nom::IResult;
use nom::combinator::map;
use nom::{Parser, branch::alt};
use std::str::FromStr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

/// A top-level Versioning type which acts as a wrapper for the more specific
/// types.
///
/// # Examples
///
/// ```
/// use versions::Versioning;
///
/// let a = Versioning::new("1.2.3-1").unwrap();   // SemVer.
/// let b = Versioning::new("1.2.3r1").unwrap();   // Not SemVer but good enough.
/// let c = Versioning::new("000.007-1").unwrap(); // Garbage.
///
/// assert!(a.is_ideal());
/// assert!(b.is_general());
/// assert!(c.is_complex());
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Versioning {
    /// Follows good parsing and comparison rules.
    Ideal(SemVer),
    /// A little more permissive than [`SemVer`].
    General(Version),
    /// Hope that you need not venture here.
    Complex(Mess),
}

impl Versioning {
    /// Create a `Versioning` by attempting to parse the input first as
    /// [`SemVer`], then as a [`Version`], and finally as a [`Mess`].
    pub fn new<S>(s: S) -> Option<Versioning>
    where
        S: AsRef<str>,
    {
        let str = s.as_ref();

        SemVer::new(str)
            .map(Versioning::Ideal)
            .or_else(|| Version::new(str).map(Versioning::General))
            .or_else(|| Mess::new(str).map(Versioning::Complex))
    }

    /// The raw `nom` parser for [`Versioning`]. Feel free to use this in
    /// combination with other general `nom` parsers.
    pub fn parse(i: &str) -> IResult<&str, Versioning> {
        alt((
            map(SemVer::parse, Versioning::Ideal),
            map(Version::parse, Versioning::General),
            map(Mess::parse, Versioning::Complex),
        ))
        .parse(i)
    }

    /// A short-hand for detecting an inner [`SemVer`].
    pub fn is_ideal(&self) -> bool {
        matches!(self, Versioning::Ideal(_))
    }

    /// A short-hand for detecting an inner [`Version`].
    pub fn is_general(&self) -> bool {
        matches!(self, Versioning::General(_))
    }

    /// A short-hand for detecting an inner [`Mess`].
    pub fn is_complex(&self) -> bool {
        matches!(self, Versioning::Complex(_))
    }

    /// Try to extract a position from the `Versioning` as a nice integer, as if it
    /// were a [`SemVer`].
    ///
    /// ```
    /// use versions::Versioning;
    ///
    /// let semver = Versioning::new("1.2.3-r1+git123").unwrap();
    /// assert!(semver.is_ideal());
    /// assert_eq!(Some(1), semver.nth(0));
    /// assert_eq!(Some(2), semver.nth(1));
    /// assert_eq!(Some(3), semver.nth(2));
    ///
    /// let version = Versioning::new("1:2.a.4.5.6.7-r1").unwrap();
    /// assert!(version.is_general());
    /// assert_eq!(Some(2), version.nth(0));
    /// assert_eq!(None, version.nth(1));
    /// assert_eq!(Some(4), version.nth(2));
    ///
    /// let mess = Versioning::new("1.6a.0+2014+m872b87e73dfb-1").unwrap();
    /// assert!(mess.is_complex());
    /// assert_eq!(Some(1), mess.nth(0));
    /// assert_eq!(None, mess.nth(1));
    /// assert_eq!(Some(0), mess.nth(2));
    /// ```
    pub fn nth(&self, n: usize) -> Option<u32> {
        match self {
            Versioning::Ideal(s) if n == 0 => Some(s.major),
            Versioning::Ideal(s) if n == 1 => Some(s.minor),
            Versioning::Ideal(s) if n == 2 => Some(s.patch),
            Versioning::Ideal(_) => None,
            Versioning::General(v) => v.nth(n),
            Versioning::Complex(m) => m.nth(n),
        }
    }

    pub(crate) fn matches_tilde(&self, other: &Versioning) -> bool {
        match (self, other) {
            (Versioning::Ideal(a), Versioning::Ideal(b)) => a.matches_tilde(b),
            (Versioning::General(a), Versioning::General(b)) => a.matches_tilde(b),
            // Complex can't be tilde-equal because they're not semantic.
            (Versioning::Complex(_), Versioning::Complex(_)) => false,
            // Any other combination cannot be compared.
            (_, _) => false,
        }
    }

    pub(crate) fn matches_caret(&self, other: &Versioning) -> bool {
        match (self, other) {
            (Versioning::Ideal(v1), Versioning::Ideal(v2)) => v1.matches_caret(v2),
            (Versioning::General(v1), Versioning::General(v2)) => v1.matches_caret(v2),
            // Complex can't be caret-equal because they're not semantic
            (Versioning::Complex(_), Versioning::Complex(_)) => false,
            // Any other combination cannot be compared.
            (_, _) => false,
        }
    }

    #[cfg(feature = "serde")]
    /// Function suitable for use as a custom serde deserializer for
    /// `Versioning` where `Versioning` is the type of a field in a struct.
    ///
    /// ```rust
    /// use versions::Versioning;
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct Foo {
    ///    #[serde(deserialize_with = "Versioning::deserialize_pretty")]
    ///    version: Versioning,
    ///    // ...
    /// }
    ///
    /// let foo: Foo = serde_json::from_str(r#"{"version": "1.0.0"}"#).unwrap();
    /// ```
    pub fn deserialize_pretty<'de, D>(deserializer: D) -> Result<Versioning, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;

        Versioning::new(&s)
            .ok_or_else(|| Error::IllegalVersioning(s))
            .map_err(D::Error::custom)
    }
}

impl PartialOrd for Versioning {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Versioning {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            // Obvious comparisons when the types are the same.
            (Versioning::Ideal(a), Versioning::Ideal(b)) => a.cmp(b),
            (Versioning::General(a), Versioning::General(b)) => a.cmp(b),
            (Versioning::Complex(a), Versioning::Complex(b)) => a.cmp(b),
            // SemVer and Version can compare nicely.
            (Versioning::Ideal(a), Versioning::General(b)) => a.cmp_version(b),
            (Versioning::General(a), Versioning::Ideal(b)) => b.cmp_version(a).reverse(),
            // If we're lucky, the `Mess` is well-formed enough to pull
            // SemVer-like values out of its initial positions. Otherwise we
            // need to downcast the `SemVer` into a `Mess` and hope for the
            // best.
            (Versioning::Ideal(a), Versioning::Complex(b)) => a.cmp_mess(b),
            (Versioning::Complex(a), Versioning::Ideal(b)) => b.cmp_mess(a).reverse(),
            // Same as above - we might get lucky, we might not.
            // The lucky fate means no extra allocations.
            (Versioning::General(a), Versioning::Complex(b)) => a.cmp_mess(b),
            (Versioning::Complex(a), Versioning::General(b)) => b.cmp_mess(a).reverse(),
        }
    }
}

impl std::fmt::Display for Versioning {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Versioning::Ideal(s) => write!(f, "{}", s),
            Versioning::General(v) => write!(f, "{}", v),
            Versioning::Complex(m) => write!(f, "{}", m),
        }
    }
}

impl FromStr for Versioning {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Versioning::new(s).ok_or_else(|| Error::IllegalVersioning(s.to_string()))
    }
}

impl TryFrom<&str> for Versioning {
    type Error = Error;

    /// ```
    /// use versions::Versioning;
    ///
    /// let orig = "1.2.3";
    /// let prsd: Versioning = orig.try_into().unwrap();
    /// assert_eq!(orig, prsd.to_string());
    /// ```
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Versioning::from_str(value)
    }
}

impl Default for Versioning {
    fn default() -> Self {
        Self::Ideal(SemVer::default())
    }
}
