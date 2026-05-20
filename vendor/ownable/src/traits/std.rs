use crate::traits::{IntoOwned, ToBorrowed, ToOwned};
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, Hash};

// HashSet

#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<'a, T, S> ToBorrowed<'a> for HashSet<T, S>
where
    T: ToBorrowed<'a> + Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn to_borrowed(&'a self) -> Self {
        self.iter().map(ToBorrowed::to_borrowed).collect()
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<T, S> ToOwned for HashSet<T, S>
where
    T: ToOwned,
    <T as ToOwned>::Owned: Eq + Hash,
    S: BuildHasher + Default,
{
    type Owned = HashSet<T::Owned, S>;

    #[inline]
    fn to_owned(&self) -> Self::Owned where {
        self.iter().map(ToOwned::to_owned).collect()
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<T, S> IntoOwned for HashSet<T, S>
where
    T: IntoOwned,
    <T as IntoOwned>::Owned: Eq + Hash,
    S: BuildHasher + Default,
{
    type Owned = HashSet<T::Owned, S>;

    #[inline]
    fn into_owned(self) -> Self::Owned {
        self.into_iter().map(IntoOwned::into_owned).collect()
    }
}

// HashMap

#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<'a, K, V, S> ToBorrowed<'a> for HashMap<K, V, S>
where
    K: ToBorrowed<'a> + Eq + Hash,
    V: ToBorrowed<'a>,
    S: BuildHasher + Default,
{
    #[inline]
    fn to_borrowed(&'a self) -> Self {
        self.iter()
            .map(|(k, v)| (ToBorrowed::to_borrowed(k), ToBorrowed::to_borrowed(v)))
            .collect()
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<K, V, S> ToOwned for HashMap<K, V, S>
where
    K: ToOwned,
    <K as ToOwned>::Owned: Eq + Hash,
    V: ToOwned,
    S: BuildHasher + Default,
{
    type Owned = HashMap<K::Owned, V::Owned, S>;

    #[inline]
    fn to_owned(&self) -> Self::Owned where {
        self.iter()
            .map(|(k, v)| (ToOwned::to_owned(k), ToOwned::to_owned(v)))
            .collect()
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<K, V, S> IntoOwned for HashMap<K, V, S>
where
    K: IntoOwned,
    <K as IntoOwned>::Owned: Eq + Hash,
    V: IntoOwned,
    S: BuildHasher + Default,
{
    type Owned = HashMap<K::Owned, V::Owned, S>;

    #[inline]
    fn into_owned(self) -> Self::Owned {
        self.into_iter()
            .map(|(k, v)| (IntoOwned::into_owned(k), IntoOwned::into_owned(v)))
            .collect()
    }
}
