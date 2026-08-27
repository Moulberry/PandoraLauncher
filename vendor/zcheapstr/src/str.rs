#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use alloc::{
    borrow::{Cow, ToOwned},
    string::{String, ToString},
    sync::Arc,
};
use core::{
    cmp::Ordering,
    hash::{Hash, Hasher},
};

/// A string wrapper.
///
///
/// This is similar to the [`Cow`] type, but it:
///
/// * is specialized for strings.
/// * treats `&'static str` as a separate type. This allows you to avoid allocations and copying
///   when turning a `CheapStr` instance created from a `&'static str` into an owned version in
///   generic code that doesn't/can't assume the inner lifetime of the source `CheapStr` instance.
/// * stores owned strings in an [`Arc`], so `Clone` never copies or allocates: it either copies a
///   reference or increments a reference count.
/// * is immutable. Consequently, unlike [`Cow`], it is *not* a copy-on-write type: there is no way
///   to get a mutable reference to the underlying string.
///
/// API is provided to convert from, and to a [`&str`] and [`String`].
///
/// [`&str`]: https://doc.rust-lang.org/std/str/index.html
/// [`String`]: https://doc.rust-lang.org/std/string/struct.String.html
#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CheapStr<'a>(#[cfg_attr(feature = "serde", serde(borrow))] Inner<'a>);

#[derive(Eq, Clone)]
enum Inner<'a> {
    Static(&'static str),
    Borrowed(&'a str),
    Owned(Arc<str>),
}

impl Default for Inner<'_> {
    fn default() -> Self {
        Self::Static("")
    }
}

impl<'a> PartialEq for Inner<'a> {
    fn eq(&self, other: &Inner<'a>) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<'a> Ord for Inner<'a> {
    fn cmp(&self, other: &Inner<'a>) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl<'a> PartialOrd for Inner<'a> {
    fn partial_cmp(&self, other: &Inner<'a>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Hash for Inner<'_> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.as_str().hash(h)
    }
}

impl Inner<'_> {
    /// The underlying string.
    pub fn as_str(&self) -> &str {
        match self {
            Inner::Static(s) => s,
            Inner::Borrowed(s) => s,
            Inner::Owned(s) => s,
        }
    }
}

#[cfg(feature = "serde")]
impl Serialize for Inner<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de: 'a, 'a> Deserialize<'de> for Inner<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        <&'a str>::deserialize(deserializer).map(Inner::Borrowed)
    }
}

impl CheapStr<'_> {
    /// An owned string without allocations
    pub const fn from_static(s: &'static str) -> Self {
        CheapStr(Inner::Static(s))
    }

    /// This is faster than `Clone::clone` when `self` contains owned data.
    pub fn as_ref(&self) -> CheapStr<'_> {
        match &self.0 {
            Inner::Static(s) => CheapStr(Inner::Static(s)),
            Inner::Borrowed(s) => CheapStr(Inner::Borrowed(s)),
            Inner::Owned(s) => CheapStr(Inner::Borrowed(s)),
        }
    }

    /// The underlying string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Creates an owned clone of `self`.
    pub fn to_owned(&self) -> CheapStr<'static> {
        self.clone().into_owned()
    }

    /// Creates an owned clone of `self`.
    pub fn into_owned(self) -> CheapStr<'static> {
        match self.0 {
            Inner::Static(s) => CheapStr(Inner::Static(s)),
            Inner::Borrowed(s) => CheapStr(Inner::Owned(s.to_owned().into())),
            Inner::Owned(s) => CheapStr(Inner::Owned(s)),
        }
    }
}

impl<'a> From<&'a str> for CheapStr<'a> {
    fn from(value: &'a str) -> Self {
        Self(Inner::Borrowed(value))
    }
}

impl<'a> From<&'a String> for CheapStr<'a> {
    fn from(value: &'a String) -> Self {
        Self(Inner::Borrowed(value))
    }
}

impl From<String> for CheapStr<'_> {
    fn from(value: String) -> Self {
        Self(Inner::Owned(value.into()))
    }
}

impl From<Arc<str>> for CheapStr<'_> {
    fn from(value: Arc<str>) -> Self {
        Self(Inner::Owned(value))
    }
}

impl<'a> From<Cow<'a, str>> for CheapStr<'a> {
    fn from(value: Cow<'a, str>) -> Self {
        match value {
            Cow::Owned(value) => value.into(),
            Cow::Borrowed(value) => value.into(),
        }
    }
}

impl<'a> From<CheapStr<'a>> for String {
    fn from(value: CheapStr<'a>) -> String {
        match value.0 {
            Inner::Static(s) => s.into(),
            Inner::Borrowed(s) => s.into(),
            Inner::Owned(s) => s.to_string(),
        }
    }
}

impl<'a> From<&'a CheapStr<'_>> for &'a str {
    fn from(value: &'a CheapStr<'_>) -> &'a str {
        value.as_str()
    }
}

impl core::ops::Deref for CheapStr<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl PartialEq<str> for CheapStr<'_> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for CheapStr<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl core::fmt::Debug for CheapStr<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl core::fmt::Display for CheapStr<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self.as_str(), f)
    }
}

#[cfg(test)]
mod tests {
    use super::CheapStr;
    use alloc::string::{String, ToString};

    #[test]
    fn from_string() {
        let string = String::from("value");
        let v = CheapStr::from(&string);
        assert_eq!(v.as_str(), "value");
    }

    #[test]
    fn test_ordering() {
        let first = CheapStr::from("a".to_string());
        let second = CheapStr::from_static("b");
        assert!(first < second);
    }
}

#[cfg(all(test, feature = "serde"))]
mod serde_tests {
    use super::CheapStr;
    use alloc::string::String;

    #[test]
    fn serde_round_trip() {
        let s = CheapStr::from("hello");
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"hello\"");
        let deserialized: CheapStr<'_> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, s);

        // Owned data serializes the same way.
        let owned = CheapStr::from(String::from("hello"));
        assert_eq!(serde_json::to_string(&owned).unwrap(), json);
    }

    #[test]
    fn serde_non_borrowable_input_errors() {
        // `CheapStr` only supports borrowed deserialization: input that cannot be handed out as a
        // borrowed `&str` (here because of the escape sequence) is an error, not an allocation.
        serde_json::from_str::<CheapStr<'_>>("\"a\\nb\"").unwrap_err();
    }

    #[test]
    fn serde_borrowed_deserialization() {
        let json = String::from("\"borrowed\"");
        let s: CheapStr<'_> = serde_json::from_str(&json).unwrap();
        assert_eq!(s.as_str(), "borrowed");
        // The deserialized `CheapStr` borrows from the JSON input instead of allocating.
        assert!(core::ptr::eq(s.as_str().as_ptr(), json[1..].as_ptr()));
    }
}
