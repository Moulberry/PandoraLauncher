use std::{hash::Hash, ops::Deref, str::FromStr};

use serde::{de::Error, Deserialize, Serialize};

/// Newtype struct around `fluent_uri::Uri<String>` with serialization implementations that use `as_str()` and 'from_str()' respectively.
#[derive(Debug, Clone)]
pub struct Uri(fluent_uri::Uri<String>);

impl Serialize for Uri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.as_str().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Uri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let string = String::deserialize(deserializer)?;
        fluent_uri::Uri::<String>::parse_from(string)
            .map(Uri)
            .map_err(|(_, error)| Error::custom(error.to_string()))
    }
}

impl Ord for Uri {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl PartialOrd for Uri {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl FromStr for Uri {
    type Err = fluent_uri::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // TOUCH-UP:
        // Use upstream `FromStr` implementation if and when
        // https://github.com/yescallop/fluent-uri-rs/pull/10
        // gets merged.
        // fluent_uri::Uri::from_str(s).map(Self)
        fluent_uri::Uri::parse(s).map(|uri| Self(uri.to_owned()))
    }
}

impl Deref for Uri {
    type Target = fluent_uri::Uri<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/*
    TOUCH-UP: `PartialEq`, `Eq` and `Hash` could all be derived
    if and when the respective implementations get merged upstream:
    https://github.com/yescallop/fluent-uri-rs/pull/9
*/
impl PartialEq for Uri {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for Uri {}

impl Hash for Uri {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}
