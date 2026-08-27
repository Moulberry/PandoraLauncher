/*!
The backing byte buffer for encoded values and the storage it writes into.
*/

use crate::{std::fmt, Error};

use core::marker::PhantomData;
use core::mem;
use zerocopy::IntoBytes;

use super::Kind;

#[cfg(feature = "alloc")]
use crate::std::{boxed::Box, vec::Vec};

#[cfg(feature = "alloc")]
use super::owned::drop_all;

#[cfg(not(feature = "alloc"))]
use super::{ArrayVec, PARTS_CAP};

/**
The backing byte buffer for an encoded value.
*/
pub(crate) struct Parts<'sval, S: RawStorage> {
    // NOTE: The extra `bool`s could be encoded as the first byte in `S` to save space
    pub(super) buf: S,
    pub(super) _marker: PhantomData<&'sval [u8]>,
    // Whether any part in the buffer owns an allocation that needs dropping.
    // Borrowed text and static labels are common, so this is usually `false`
    // and lets us skip the drop walk entirely.
    #[cfg(feature = "alloc")]
    pub(super) owned: bool,
    // Whether any part in the buffer borrows text or binary data for `'sval`.
    // When it's `false`, `into_owned` has nothing to convert and can skip its
    // walk entirely.
    #[cfg(feature = "alloc")]
    pub(super) borrowed: bool,
}

impl<'sval, S: RawStorage + Default> Default for Parts<'sval, S> {
    fn default() -> Self {
        Parts {
            buf: Default::default(),
            _marker: PhantomData,
            #[cfg(feature = "alloc")]
            owned: false,
            #[cfg(feature = "alloc")]
            borrowed: false,
        }
    }
}

impl<'sval, S: RawStorage> Parts<'sval, S> {
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.buf.len()
    }

    /**
    Whether any part in the buffer borrows data for `'sval`.
    */
    #[cfg(feature = "alloc")]
    #[inline]
    pub(crate) fn is_borrowed(&self) -> bool {
        self.borrowed
    }

    // The offset the `len` field of the next encoded part will occupy
    #[inline]
    pub(crate) fn next_len_at(&self) -> usize {
        self.len() + 1
    }

    #[inline]
    fn as_mut_slice(&mut self) -> &mut [u8] {
        self.buf.as_mut_slice()
    }

    #[inline]
    pub(super) fn patch_u32(&mut self, at: usize, v: u32) {
        let sz = mem::size_of::<u32>();
        v.write_to(&mut self.as_mut_slice()[at..at + sz])
            .expect("attempt to patch a value past the end of the buffer");
    }

    // Always inlined: called once per container end, the hottest patch path
    #[inline(always)]
    pub(crate) fn patch_container_end(
        &mut self,
        len_at: usize,
        len: usize,
        num_entries: Option<u32>,
    ) {
        if !Kind::from_tag_byte(self.buf.as_slice()[len_at - 1]).is_container() {
            return;
        }

        // The buffer is capped at `MAX_PARTS_LEN`, so lengths always fit
        debug_assert!(len <= u32::MAX as usize);

        // The byte length of the container is always the first field
        self.patch_u32(len_at, len as u32);

        if let Some(num_entries) = num_entries {
            // The number of entries is always the second field
            self.patch_u32(len_at + mem::size_of::<u32>(), num_entries);
        }
    }
}

impl<'sval, S: RawStorageMut + Default> Parts<'sval, S> {
    pub(crate) fn new() -> Self {
        Default::default()
    }
}

impl<'sval, S: RawStorageMut> Parts<'sval, S> {
    #[inline]
    pub(super) fn reserve(&mut self, n: usize) -> Result<(), Error> {
        self.buf.reserve(n)
    }

    #[inline]
    pub(crate) fn capacity(&self) -> usize {
        self.buf.capacity()
    }

    pub(crate) fn clear(&mut self) {
        #[cfg(feature = "alloc")]
        {
            // NOTE: Owned values are rejected when the `alloc` feature is disabled,
            // even when `sval` itself may still produce them

            // Move out of the current buffer so we can clear it
            // If dropping fails we'll leave behind an empty `Parts<S>`, not
            // one with partially dropped values in it
            let buf = mem::take(&mut self.buf);
            let owned = self.owned;

            self.owned = false;
            self.borrowed = false;

            if owned {
                // SAFETY: `Parts` maintains a valid sequence of parts, and
                // `owned` is already reset, so their payloads only drop once
                unsafe {
                    drop_all(buf.as_slice());
                }
            }

            // If we successfully walked the buffer then restore it
            self.buf = buf;
        }

        self.buf.clear();
    }
}

#[cfg(not(feature = "alloc"))]
impl<'sval, S: RawStorage + Clone> Clone for Parts<'sval, S> {
    fn clone(&self) -> Self {
        // In no-std builds nothing in the buffer owns an allocation, so a
        // raw byte copy is a correct deep clone
        Parts {
            buf: self.buf.clone(),
            _marker: PhantomData,
        }
    }
}

impl<'sval, S: RawStorage> fmt::Debug for Parts<'sval, S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[cfg(test)]
        {
            f.debug_list().entries(self.decode()).finish()
        }
        #[cfg(not(test))]
        {
            // The raw bytes can't be printed: encoded part payloads have
            // uninitialized holes (padding and inactive enum variant bytes)
            // that must not be read as values
            f.debug_struct("Parts").field("len", &self.len()).finish()
        }
    }
}

/**
The backing byte storage for a buffer of parts.
*/
pub(crate) trait RawStorage {
    fn as_slice(&self) -> &[u8];

    fn as_mut_slice(&mut self) -> &mut [u8];

    #[inline]
    fn len(&self) -> usize {
        self.as_slice().len()
    }

    #[cfg(feature = "alloc")]
    fn from_vec(buf: Vec<u8>) -> Self
    where
        Self: Sized;
}

/**
Mutable backing byte storage.
*/
pub(crate) trait RawStorageMut: RawStorage + Default {
    fn reserve(&mut self, n: usize) -> Result<(), Error>;

    fn extend_from_slice(&mut self, v: &[u8]) -> Result<(), Error>;

    fn capacity(&self) -> usize;

    fn as_mut_ptr(&mut self) -> *mut u8;

    // SAFETY: The caller must ensure `n` bytes past the end were written
    // through a writer
    unsafe fn advance_len(&mut self, n: usize);

    fn clear(&mut self);
}

#[cfg(feature = "alloc")]
pub(super) type ValueBufStore = Vec<u8>;
#[cfg(not(feature = "alloc"))]
pub(super) type ValueBufStore = ArrayVec<u8, PARTS_CAP>;

#[cfg(feature = "alloc")]
pub(super) type ValueStore = Box<[u8]>;
#[cfg(not(feature = "alloc"))]
pub(super) type ValueStore = ArrayVec<u8, PARTS_CAP>;

pub(crate) type ValueBufParts<'sval> = Parts<'sval, ValueBufStore>;
pub(crate) type ValueParts<'sval> = Parts<'sval, ValueStore>;

// Convert a `ValueBuf`'s parts into the compact storage used by `Value`.
#[cfg(feature = "alloc")]
pub(crate) fn into_value_parts(parts: ValueBufParts) -> ValueParts {
    parts.into_boxed()
}
#[cfg(not(feature = "alloc"))]
pub(crate) fn into_value_parts(parts: ValueBufParts) -> ValueParts {
    parts
}
