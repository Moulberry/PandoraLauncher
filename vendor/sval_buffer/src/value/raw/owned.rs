/*!
Allocator-backed storage and support for parts that own or borrow
allocations: dropping, deep-cloning, and converting into owned.
*/

use crate::{
    std::{boxed::Box, vec::Vec},
    Error,
};

use core::{
    marker::PhantomData,
    mem::{self, ManuallyDrop},
    ptr,
};
use zerocopy::IntoBytes;

use super::{
    header::payload_size,
    read::{read_native_at, read_pod_at, skip_part, Reader},
    EnumHeader, Kind, Parts, RawStorage, RawStorageMut, RecordHeader, RecordTupleHeader,
    RecordTupleValueHeader, RecordValueHeader, TagPart, TaggedHeader, TupleHeader, PARTS_CAP,
    TAG_BORROWED, TAG_OWNED,
};

impl RawStorage for Vec<u8> {
    #[inline]
    fn as_slice(&self) -> &[u8] {
        self
    }

    #[inline]
    fn as_mut_slice(&mut self) -> &mut [u8] {
        self
    }

    #[inline]
    fn from_vec(buf: Vec<u8>) -> Self {
        buf
    }
}

impl RawStorageMut for Vec<u8> {
    #[inline]
    fn reserve(&mut self, n: usize) -> Result<(), Error> {
        #[cold]
        #[inline(never)]
        fn grow(buf: &mut Vec<u8>, n: usize) -> Result<(), Error> {
            if super::MAX_PARTS_LEN - buf.len() < n {
                return Err(Error::invalid_value(
                    "value exceeds the maximum buffer size",
                ));
            }

            // Growth here is coarser than the default `Vec` strategy
            let grown = n
                .max(buf.capacity().saturating_mul(3))
                .max(PARTS_CAP)
                .min(super::MAX_PARTS_LEN - buf.len());

            Vec::reserve(buf, grown);

            Ok(())
        }

        if self.capacity() - self.len() < n {
            return grow(self, n);
        }

        Ok(())
    }

    #[inline]
    fn extend_from_slice(&mut self, v: &[u8]) -> Result<(), Error> {
        Vec::extend_from_slice(self, v);

        Ok(())
    }

    #[inline]
    fn capacity(&self) -> usize {
        Vec::capacity(self)
    }

    #[inline]
    fn as_mut_ptr(&mut self) -> *mut u8 {
        Vec::as_mut_ptr(self)
    }

    #[inline]
    unsafe fn advance_len(&mut self, n: usize) {
        // SAFETY: Upheld by this function's contract
        unsafe { self.set_len(self.len() + n) }
    }

    #[inline]
    fn clear(&mut self) {
        Vec::clear(self)
    }
}

impl RawStorage for Box<[u8]> {
    #[inline]
    fn as_slice(&self) -> &[u8] {
        self
    }

    #[inline]
    fn as_mut_slice(&mut self) -> &mut [u8] {
        self
    }

    #[inline]
    fn from_vec(buf: Vec<u8>) -> Self {
        buf.into_boxed_slice()
    }
}

impl<'sval> Parts<'sval, Vec<u8>> {
    pub(super) fn into_boxed(mut self) -> Parts<'sval, Box<[u8]>> {
        // Move any potentially owned allocations out of `self`, so drop
        // becomes a no-op
        let buf = mem::take(&mut self.buf);

        Parts {
            buf: buf.into_boxed_slice(),
            _marker: PhantomData,
            owned: self.owned,
            borrowed: self.borrowed,
        }
    }
}

impl<'sval, S: RawStorage> Drop for Parts<'sval, S> {
    fn drop(&mut self) {
        if self.owned {
            // SAFETY: `Parts` maintains a valid sequence of parts, and this
            // is the buffer's final use
            unsafe {
                drop_all(self.buf.as_slice());
            }
        }
    }
}

impl<'sval, S: RawStorage + Clone> Clone for Parts<'sval, S> {
    fn clone(&self) -> Self {
        // NOTE: If cloning in place fails we'll leak, but won't double-free
        let mut buf = self.buf.clone();

        if self.owned {
            // SAFETY: The byte copy is a valid sequence of parts whose
            // owned payloads still alias `self`; cloning them in place
            // makes the copy independent
            unsafe {
                clone_in_place(buf.as_mut_slice());
            }
        }

        Parts {
            buf,
            _marker: PhantomData,
            owned: self.owned,
            borrowed: self.borrowed,
        }
    }
}

pub(crate) fn make_owned<S: RawStorage>(
    mut parts: Parts<'_, S>,
) -> Result<Parts<'static, S>, Error> {
    if !parts.borrowed {
        // SAFETY: `Parts` doesn't carry data borrowed for `'sval`
        return Ok(unsafe { mem::transmute::<Parts<'_, S>, Parts<'static, S>>(parts) });
    }

    // SAFETY: `Parts` maintains a valid sequence of parts
    let size = unsafe { owned_size(parts.buf.as_slice()) };

    if size > super::MAX_PARTS_LEN {
        return Err(Error::invalid_value(
            "value exceeds the maximum buffer size",
        ));
    }

    // SAFETY: `Parts` maintains a valid sequence of parts, and the old
    // bytes are replaced below without dropping their payloads
    let mut buf = Vec::with_capacity(size);
    unsafe {
        rebuild_owned(parts.buf.as_slice(), &mut buf);
    }

    parts.buf = S::from_vec(buf);
    parts.borrowed = false;

    // SAFETY: `Parts` doesn't carry data borrowed for `'sval`
    Ok(unsafe { mem::transmute::<Parts<'_, S>, Parts<'static, S>>(parts) })
}

// SAFETY: The caller must ensure `bytes` encodes a valid sequence of parts,
// and must not use or drop their owned payloads again afterwards
pub(super) unsafe fn drop_all(bytes: &[u8]) {
    let mut pos = 0;

    while pos < bytes.len() {
        let tag = bytes[pos];

        if tag & TAG_OWNED != 0 {
            let mut r = Reader {
                bytes,
                pos: pos + 1,
            };

            // SAFETY: A valid payload of the matched kind is encoded at
            // `pos + 1`, upheld by this function's contract
            match Kind::from_tag_byte(tag) {
                Kind::Tag => unsafe {
                    ManuallyDrop::drop(&mut r.read_native_non_copy::<TagPart>())
                },
                Kind::Enum => unsafe {
                    ManuallyDrop::drop(&mut r.read_native_non_copy::<EnumHeader>())
                },
                Kind::Tagged => unsafe {
                    ManuallyDrop::drop(&mut r.read_native_non_copy::<TaggedHeader>())
                },
                Kind::Record => unsafe {
                    ManuallyDrop::drop(&mut r.read_native_non_copy::<RecordHeader>())
                },
                Kind::Tuple => unsafe {
                    ManuallyDrop::drop(&mut r.read_native_non_copy::<TupleHeader>())
                },
                Kind::RecordTuple => unsafe {
                    ManuallyDrop::drop(&mut r.read_native_non_copy::<RecordTupleHeader>())
                },
                Kind::RecordValue => unsafe {
                    ManuallyDrop::drop(&mut r.read_native_non_copy::<RecordValueHeader>())
                },
                Kind::RecordTupleValue => unsafe {
                    ManuallyDrop::drop(&mut r.read_native_non_copy::<RecordTupleValueHeader>())
                },
                // Only the kinds above ever set `TAG_OWNED`
                _ => unreachable!("unexpected owned part"),
            }
        }

        pos = skip_part(bytes, pos);
    }
}

// SAFETY: The caller must ensure `bytes` encodes a valid sequence of parts
// whose owned payloads alias allocations owned by another buffer
unsafe fn clone_in_place(bytes: &mut [u8]) {
    // SAFETY: The caller must ensure a valid `T` is encoded at `at`, and that
    // its payload aliases an allocation owned by another buffer
    unsafe fn clone_part_in_place<T: Clone>(bytes: &mut [u8], at: usize) {
        debug_assert!(
            at + mem::size_of::<T>() <= bytes.len(),
            "attempt to clone a part payload past the end of the buffer"
        );

        // SAFETY: Upheld by this function's contract
        unsafe {
            let ptr = bytes.as_mut_ptr().add(at) as *mut T;

            // The bitwise copy aliases an allocation owned by the buffer this one was
            // copied from, so it must not be dropped
            let aliased = ManuallyDrop::new(ptr::read_unaligned(ptr));
            ptr::write_unaligned(ptr, T::clone(&aliased));
        }
    }

    let mut pos = 0;

    while pos < bytes.len() {
        let tag = bytes[pos];

        if tag & TAG_OWNED != 0 {
            // SAFETY: A valid payload of the matched kind is encoded at
            // `pos + 1`, upheld by this function's contract
            unsafe {
                match Kind::from_tag_byte(tag) {
                    Kind::Tag => clone_part_in_place::<TagPart>(bytes, pos + 1),
                    Kind::Enum => clone_part_in_place::<EnumHeader>(bytes, pos + 1),
                    Kind::Tagged => clone_part_in_place::<TaggedHeader>(bytes, pos + 1),
                    Kind::Record => clone_part_in_place::<RecordHeader>(bytes, pos + 1),
                    Kind::Tuple => clone_part_in_place::<TupleHeader>(bytes, pos + 1),
                    Kind::RecordTuple => clone_part_in_place::<RecordTupleHeader>(bytes, pos + 1),
                    Kind::RecordValue => clone_part_in_place::<RecordValueHeader>(bytes, pos + 1),
                    Kind::RecordTupleValue => {
                        clone_part_in_place::<RecordTupleValueHeader>(bytes, pos + 1)
                    }
                    // Only the kinds above ever set `TAG_OWNED`
                    _ => unreachable!("unexpected owned part"),
                }
            }
        }

        pos = skip_part(bytes, pos);
    }
}

// SAFETY: The caller must ensure `bytes` encodes a valid sequence of parts
// and the part at `pos` is borrowed text or binary
#[inline]
unsafe fn borrowed_fragment(bytes: &[u8], pos: usize) -> &[u8] {
    debug_assert!(
        bytes[pos] & TAG_BORROWED != 0,
        "part at `pos` is not borrowed"
    );

    // SAFETY: A borrowed part's payload is a `&str` or `&[u8]`, which share
    // a layout; either way its data is a run of initialized bytes
    unsafe { read_native_at::<&[u8]>(bytes, pos + 1) }
}

// SAFETY: The caller must ensure `bytes` encodes a valid sequence of parts
unsafe fn owned_size(bytes: &[u8]) -> usize {
    let mut pos = 0;
    let mut size = 0usize;

    while pos < bytes.len() {
        let end = skip_part(bytes, pos);

        let part_size = if bytes[pos] & TAG_BORROWED != 0 {
            // SAFETY: `TAG_BORROWED` is only set on parts that hold borrowed values
            1 + mem::size_of::<u32>() + unsafe { borrowed_fragment(bytes, pos).len() }
        } else {
            end - pos
        };

        // Saturate rather than wrap on 32-bit targets, where the sum of
        // inlined fragments can exceed `usize`; a saturated total fails
        // `make_owned`'s size check instead of under-allocating
        size = size.saturating_add(part_size);

        pos = end;
    }

    size
}

// SAFETY: The caller must ensure `old` encodes a valid sequence of parts;
// their owned payloads are moved into `out`, so the caller must release
// `old` without dropping them
unsafe fn rebuild_owned(old: &[u8], out: &mut Vec<u8>) {
    let mut pos = 0;

    while pos < old.len() {
        let tag = old[pos];
        let kind = Kind::from_tag_byte(tag);

        match kind {
            Kind::Text | Kind::Binary if tag & TAG_BORROWED != 0 => {
                // Copy both borrowed fragments into `out` as inline fragments

                // SAFETY: `TAG_BORROWED` is only set on parts that hold borrowed values
                let fragment = unsafe { borrowed_fragment(old, pos) };

                // The borrowed fragment becomes an inline part
                out.push(tag & !TAG_BORROWED);
                // Fragments are capped when they're encoded, and
                // `make_owned` bounds the whole rebuilt size before
                // calling here, so the length always fits
                debug_assert!(fragment.len() <= u32::MAX as usize);
                out.extend_from_slice((fragment.len() as u32).as_bytes());
                out.extend_from_slice(fragment);

                pos += 1 + mem::size_of::<&str>();
            }
            Kind::Map
            | Kind::MapKey
            | Kind::MapValue
            | Kind::Seq
            | Kind::SeqValue
            | Kind::Enum
            | Kind::Tagged
            | Kind::Record
            | Kind::RecordValue
            | Kind::Tuple
            | Kind::TupleValue
            | Kind::RecordTuple
            | Kind::RecordTupleValue => {
                // Parts that require cloning

                let header_size = 1 + payload_size(kind);

                // A container's body length is the first field of its
                // header, immediately after the tag byte
                let body_len = read_pod_at::<u32>(old, pos + 1) as usize;

                // Copy the header verbatim, then rebuild the body and patch
                // the length with the body's rebuilt size
                let len_at = out.len() + 1;
                out.extend_from_slice(&old[pos..pos + header_size]);

                let body_at = out.len();

                // SAFETY: A container's body is itself a valid sequence of parts
                unsafe {
                    rebuild_owned(&old[pos + header_size..pos + header_size + body_len], out);
                }

                let len = (out.len() - body_at) as u32;
                len.write_to(&mut out[len_at..len_at + mem::size_of::<u32>()])
                    .expect("attempt to patch a length past the end of the buffer");

                pos += header_size + body_len;
            }
            _ => {
                // Everything else (including inline text and binary) is copied verbatim
                let end = skip_part(old, pos);
                out.extend_from_slice(&old[pos..end]);

                pos = end;
            }
        }
    }
}
