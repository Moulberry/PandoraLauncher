use std::borrow::Cow;
use std::sync::Arc;

/// A wrapper for `Cow<'a, str>` that is specifically designed for use with the `t!` macro.
///
/// This wrapper provides additional functionality or optimizations when handling strings in the `t!` macro.
pub struct CowStr<'a>(Cow<'a, str>);

impl<'a> CowStr<'a> {
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }

    pub fn into_inner(self) -> Cow<'a, str> {
        self.0
    }
}

macro_rules! impl_convert_from_numeric {
    ($typ:ty) => {
        impl<'a> From<$typ> for CowStr<'a> {
            fn from(val: $typ) -> Self {
                Self(Cow::from(format!("{}", val)))
            }
        }
    };
}

impl_convert_from_numeric!(i8);
impl_convert_from_numeric!(i16);
impl_convert_from_numeric!(i32);
impl_convert_from_numeric!(i64);
impl_convert_from_numeric!(i128);
impl_convert_from_numeric!(isize);

impl_convert_from_numeric!(u8);
impl_convert_from_numeric!(u16);
impl_convert_from_numeric!(u32);
impl_convert_from_numeric!(u64);
impl_convert_from_numeric!(u128);
impl_convert_from_numeric!(usize);

impl<'a> From<Arc<str>> for CowStr<'a> {
    #[inline]
    fn from(s: Arc<str>) -> Self {
        Self(Cow::Owned(s.to_string()))
    }
}

impl<'a> From<Box<str>> for CowStr<'a> {
    #[inline]
    fn from(s: Box<str>) -> Self {
        Self(Cow::Owned(s.to_string()))
    }
}

impl<'a> From<&'a str> for CowStr<'a> {
    #[inline]
    fn from(s: &'a str) -> Self {
        Self(Cow::Borrowed(s))
    }
}

impl<'a> From<&&'a str> for CowStr<'a> {
    #[inline]
    fn from(s: &&'a str) -> Self {
        Self(Cow::Borrowed(s))
    }
}

impl<'a> From<Arc<&'a str>> for CowStr<'a> {
    #[inline]
    fn from(s: Arc<&'a str>) -> Self {
        Self(Cow::Borrowed(*s))
    }
}

impl<'a> From<Box<&'a str>> for CowStr<'a> {
    #[inline]
    fn from(s: Box<&'a str>) -> Self {
        Self(Cow::Borrowed(*s))
    }
}

impl<'a> From<String> for CowStr<'a> {
    #[inline]
    fn from(s: String) -> Self {
        Self(Cow::from(s))
    }
}

impl<'a> From<&'a String> for CowStr<'a> {
    #[inline]
    fn from(s: &'a String) -> Self {
        Self(Cow::Borrowed(s))
    }
}

impl<'a> From<Arc<String>> for CowStr<'a> {
    #[inline]
    fn from(s: Arc<String>) -> Self {
        Self(Cow::Owned(s.to_string()))
    }
}

impl<'a> From<Box<String>> for CowStr<'a> {
    #[inline]
    fn from(s: Box<String>) -> Self {
        Self(Cow::from(*s))
    }
}
