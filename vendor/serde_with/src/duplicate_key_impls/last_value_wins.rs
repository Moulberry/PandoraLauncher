use crate::prelude::*;
#[cfg(any(
    feature = "std",
    feature = "hashbrown_0_14",
    feature = "hashbrown_0_15",
    feature = "hashbrown_0_16",
    feature = "hashbrown_0_17",
    feature = "indexmap_1",
    feature = "indexmap_2"
))]
use crate::utils::size_hint_cautious;

pub trait DuplicateInsertsLastWinsSet<T> {
    fn new(size_hint: Option<usize>) -> Self;

    /// Insert or replace the existing value
    fn replace(&mut self, value: T);
}

#[cfg(feature = "std")]
impl<T, S> DuplicateInsertsLastWinsSet<T> for HashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn new(size_hint: Option<usize>) -> Self {
        Self::with_capacity_and_hasher(size_hint_cautious::<T>(size_hint), S::default())
    }

    #[inline]
    fn replace(&mut self, value: T) {
        // Hashset already fulfils the contract
        self.replace(value);
    }
}

#[cfg(feature = "hashbrown_0_14")]
impl<T, S> DuplicateInsertsLastWinsSet<T> for hashbrown_0_14::HashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn new(size_hint: Option<usize>) -> Self {
        Self::with_capacity_and_hasher(size_hint_cautious::<T>(size_hint), S::default())
    }

    #[inline]
    fn replace(&mut self, value: T) {
        // Hashset already fulfils the contract
        self.replace(value);
    }
}

#[cfg(feature = "hashbrown_0_15")]
impl<T, S> DuplicateInsertsLastWinsSet<T> for hashbrown_0_15::HashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn new(size_hint: Option<usize>) -> Self {
        Self::with_capacity_and_hasher(size_hint_cautious::<T>(size_hint), S::default())
    }

    #[inline]
    fn replace(&mut self, value: T) {
        // Hashset already fulfils the contract
        self.replace(value);
    }
}

#[cfg(feature = "hashbrown_0_16")]
impl<T, S> DuplicateInsertsLastWinsSet<T> for hashbrown_0_16::HashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn new(size_hint: Option<usize>) -> Self {
        Self::with_capacity_and_hasher(size_hint_cautious::<T>(size_hint), S::default())
    }

    #[inline]
    fn replace(&mut self, value: T) {
        // Hashset already fulfils the contract
        self.replace(value);
    }
}

#[cfg(feature = "hashbrown_0_17")]
impl<T, S> DuplicateInsertsLastWinsSet<T> for hashbrown_0_17::HashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn new(size_hint: Option<usize>) -> Self {
        Self::with_capacity_and_hasher(size_hint_cautious::<T>(size_hint), S::default())
    }

    #[inline]
    fn replace(&mut self, value: T) {
        // Hashset already fulfils the contract
        self.replace(value);
    }
}

#[cfg(feature = "indexmap_1")]
impl<T, S> DuplicateInsertsLastWinsSet<T> for indexmap_1::IndexSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn new(size_hint: Option<usize>) -> Self {
        Self::with_capacity_and_hasher(size_hint_cautious::<T>(size_hint), S::default())
    }

    #[inline]
    fn replace(&mut self, value: T) {
        // Hashset already fulfils the contract
        self.replace(value);
    }
}

#[cfg(feature = "indexmap_2")]
impl<T, S> DuplicateInsertsLastWinsSet<T> for indexmap_2::IndexSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn new(size_hint: Option<usize>) -> Self {
        Self::with_capacity_and_hasher(size_hint_cautious::<T>(size_hint), S::default())
    }

    #[inline]
    fn replace(&mut self, value: T) {
        // Hashset already fulfils the contract
        self.replace(value);
    }
}

impl<T> DuplicateInsertsLastWinsSet<T> for BTreeSet<T>
where
    T: Ord,
{
    #[inline]
    fn new(_size_hint: Option<usize>) -> Self {
        Self::new()
    }

    #[inline]
    fn replace(&mut self, value: T) {
        // BTreeSet already fulfils the contract
        self.replace(value);
    }
}
