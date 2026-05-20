use super::Ustr;
use byteorder::{ByteOrder, NativeEndian};
use std::{
    collections::{HashMap, HashSet},
    hash::{BuildHasherDefault, Hasher},
};

/// A standard `HashMap` using `Ustr` as the key type with a custom `Hasher`
/// that just uses the precomputed hash for speed instead of calculating it.
pub type UstrMap<V> = HashMap<Ustr, V, BuildHasherDefault<IdentityHasher>>;

/// A standard `HashSet` using `Ustr` as the key type with a custom `Hasher`
/// that just uses the precomputed hash for speed instead of calculating it.
pub type UstrSet = HashSet<Ustr, BuildHasherDefault<IdentityHasher>>;

/// The worst hasher in the world -- the identity hasher.
#[doc(hidden)]
#[derive(Default)]
pub struct IdentityHasher {
    hash: u64,
}

impl Hasher for IdentityHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        if bytes.len() == 8 {
            self.hash = NativeEndian::read_u64(bytes);
        }
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

#[test]
fn test_hashing() {
    let _t = super::TEST_LOCK.lock();
    use crate::ustr as u;

    use std::hash::Hash;
    let u1 = u("the quick brown fox");
    let u2 = u("jumped over the lazy dog");

    let mut hasher = IdentityHasher::default();
    u1.hash(&mut hasher);
    assert_eq!(hasher.finish(), u1.precomputed_hash());

    let mut hasher = IdentityHasher::default();
    u2.hash(&mut hasher);
    assert_eq!(hasher.finish(), u2.precomputed_hash());

    let mut hm = UstrMap::<u32>::default();
    hm.insert(u1, 17);
    hm.insert(u2, 42);

    assert_eq!(hm.get(&u1), Some(&17));
    assert_eq!(hm.get(&u2), Some(&42));
}
