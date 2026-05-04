use crate::as_impl::impl_as;
use crate::traits::{IntoOwned, ToBorrowed, ToOwned};
use core::borrow::{Borrow, BorrowMut};
use core::cmp::Ordering;
use core::fmt::{Debug, Display, Formatter};
use core::hash::{Hash, Hasher};
use core::ops::{Deref, DerefMut};

/// Transparent wrapper for [`Copy`]able types to support all [traits](crate::traits) (by copying).
///
/// ```rust
/// # use ownable::{AsCopy, IntoOwned, ToBorrowed, ToOwned};
/// # use std::borrow::Cow;
/// # use std::collections::HashMap;
/// # use std::net::Ipv4Addr;
/// // Simple struct which uses `Ipv4Addr` with the help of `AsCopy`.
/// // As an alternative it's possible to impl the ownable traits by hand (for own types, not `Ipv4Addr`).
/// #[derive(IntoOwned, ToBorrowed, ToOwned)]
/// struct MyMap<'a>(HashMap<AsCopy<Ipv4Addr>, Cow<'a, str>>);
/// ```
///
/// All trait impls work on the inner value as if there is no layer in between
/// (e.g. `Display` does not add a `AsCopy` to the output).
#[repr(transparent)]
pub struct AsCopy<T: Copy>(pub T);

impl<T: Copy> ToBorrowed<'_> for AsCopy<T> {
    #[inline(always)]
    fn to_borrowed(&self) -> Self {
        AsCopy(self.0)
    }
}

impl<T: Copy> ToOwned for AsCopy<T> {
    type Owned = AsCopy<T>;

    #[inline(always)]
    fn to_owned(&self) -> Self::Owned {
        AsCopy(self.0)
    }
}

impl<T: Copy> IntoOwned for AsCopy<T> {
    type Owned = AsCopy<T>;

    #[inline(always)]
    fn into_owned(self) -> Self::Owned {
        AsCopy(self.0)
    }
}

impl_as!(AsCopy, Copy);
