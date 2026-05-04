use crate::as_impl::impl_as;
use crate::traits::{IntoOwned, ToBorrowed, ToOwned};
use alloc::string::String;
use core::borrow::{Borrow, BorrowMut};
use core::cmp::Ordering;
use core::fmt::{Debug, Display, Formatter};
use core::hash::{Hash, Hasher};
use core::ops::{Deref, DerefMut};

/// Transparent wrapper for [`Clone`]able types to support all [traits](crate::traits) (by cloning).
///
/// ```rust
/// # use ownable::{AsClone, IntoOwned, ToBorrowed, ToOwned};
/// # use std::borrow::Cow;
/// # use std::collections::HashMap;
/// // Simple struct which uses a `String` with the help of `AsClone`.
/// // As an alternative it's possible to impl the ownable traits by hand (for own types, not `String`).
/// #[derive(IntoOwned, ToBorrowed, ToOwned)]
/// struct MyMap<'a>(HashMap<AsClone<String>, Cow<'a, str>>);
/// ```
///
/// All trait impls work on the inner value as if there is no layer in between
/// (e.g. `Display` does not add a `AsClone` to the output).
#[repr(transparent)]
pub struct AsClone<T: Clone>(pub T);

impl<T: Clone> ToBorrowed<'_> for AsClone<T> {
    #[inline(always)]
    fn to_borrowed(&self) -> Self {
        AsClone(self.0.clone())
    }
}

impl<T: Clone> ToOwned for AsClone<T> {
    type Owned = AsClone<T>;

    #[inline(always)]
    fn to_owned(&self) -> Self::Owned {
        AsClone(self.0.clone())
    }
}

impl<T: Clone> IntoOwned for AsClone<T> {
    type Owned = AsClone<T>;

    #[inline(always)]
    fn into_owned(self) -> Self::Owned {
        AsClone(self.0)
    }
}

impl_as!(AsClone, Clone);

// Clone specific

impl From<&str> for AsClone<String> {
    #[inline]
    fn from(value: &str) -> Self {
        AsClone(value.into())
    }
}
