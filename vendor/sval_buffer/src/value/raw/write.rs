/*!
Writing parts into the buffer: the `Encode` trait and the write cursor
that backs it.
*/

use crate::Error;

use core::{mem, ptr};
use zerocopy::{Immutable, IntoBytes};

use super::{
    BinaryHeader, BorrowedBinary, BorrowedText, EnumHeader, Kind, MapHeader, MapKeyHeader,
    MapValueHeader, Parts, RawStorageMut, RecordHeader, RecordTupleHeader, RecordTupleValueHeader,
    RecordValueHeader, SeqHeader, SeqValueHeader, TagHintPart, TagPart, TaggedHeader, TextHeader,
    TupleHeader, TupleValueHeader,
};

/**
A trait for data that can be written as bytes into a buffer.
*/
// SAFETY: Implementors must correctly report the ownership of their value
pub(crate) unsafe trait Encode<'sval>: Sized {
    /**
    Whether the value owns an allocation that needs to be cloned/dropped.
    */
    fn is_owned(&self) -> bool;

    /**
    Whether the value borrows an allocation for `'sval` that needs to be cloned.
    */
    fn is_borrowed(&self) -> bool;

    /**
    Whether the header tracks the count of its entries.
    */
    fn is_entry_tracking() -> bool;

    /**
    Write a tag byte followed by `self` into the buffer.
    */
    fn encode<S: RawStorageMut>(self, parts: &mut Parts<'sval, S>) -> Result<(), Error>;
}

macro_rules! impl_encode_pod {
    ($($ty:ty: $kind:expr,)*) => {
        $(
            // SAFETY: PODs don't ever need to be dropped
            unsafe impl<'sval> Encode<'sval> for $ty {
                #[inline]
                fn is_owned(&self) -> bool {
                    false
                }

                #[inline]
                fn is_borrowed(&self) -> bool {
                    false
                }

                fn is_entry_tracking() -> bool {
                    $kind.is_entry_tracking()
                }

                #[inline]
                fn encode<S: RawStorageMut>(
                    self,
                    parts: &mut Parts<'sval, S>,
                ) -> Result<(), Error> {
                    encode_pod(parts, $kind.to_tag_byte(self.is_owned(), self.is_borrowed())?, self)
                }
            }
        )*
    };
}

macro_rules! impl_encode_optional_labeled_container {
    ($($ty:ident: $kind:expr,)*) => {
        $(
            // SAFETY: `is_owned` reports if `label` owns an allocation
            unsafe impl<'sval> Encode<'sval> for $ty {
                #[inline]
                fn is_owned(&self) -> bool {
                    let $ty { label, .. } = self;

                    matches!(label, Some(label) if label.as_static_str().is_none())
                }

                #[inline]
                fn is_borrowed(&self) -> bool {
                    false
                }

                #[inline]
                fn is_entry_tracking() -> bool {
                    $kind.is_entry_tracking()
                }

                #[inline]
                fn encode<S: RawStorageMut>(
                    self,
                    parts: &mut Parts<'sval, S>,
                ) -> Result<(), Error> {
                    unsafe {
                        encode_native(parts, $kind.to_tag_byte(self.is_owned(), self.is_borrowed())?, self)
                    }
                }
            }
        )*
    };
}

macro_rules! impl_encode_labeled_container {
    ($($ty:ident: $kind:expr,)*) => {
        $(
            // SAFETY: `is_owned` reports if `label` owns an allocation
            unsafe impl<'sval> Encode<'sval> for $ty {
                #[inline]
                fn is_owned(&self) -> bool {
                    let $ty { label, .. } = self;

                    label.as_static_str().is_none()
                }

                #[inline]
                fn is_borrowed(&self) -> bool {
                    false
                }

                #[inline]
                fn is_entry_tracking() -> bool {
                    $kind.is_entry_tracking()
                }

                #[inline]
                fn encode<S: RawStorageMut>(
                    self,
                    parts: &mut Parts<'sval, S>,
                ) -> Result<(), Error> {
                    unsafe {
                        encode_native(parts, $kind.to_tag_byte(self.is_owned(), self.is_borrowed())?, self)
                    }
                }
            }
        )*
    };
}

macro_rules! impl_encode_unlabeled_container {
    ($($ty:ident: $kind:expr,)*) => {
        $(
            // SAFETY: no allocations are owned
            unsafe impl<'sval> Encode<'sval> for $ty {
                #[inline]
                fn is_owned(&self) -> bool {
                    false
                }

                #[inline]
                fn is_borrowed(&self) -> bool {
                    false
                }

                #[inline]
                fn is_entry_tracking() -> bool {
                    $kind.is_entry_tracking()
                }

                #[inline]
                fn encode<S: RawStorageMut>(
                    self,
                    parts: &mut Parts<'sval, S>,
                ) -> Result<(), Error> {
                    unsafe {
                        encode_native(parts, $kind.to_tag_byte(self.is_owned(), self.is_borrowed())?, self)
                    }
                }
            }
        )*
    };
}

macro_rules! impl_encode_borrowed_fragment {
    ($($ty:ident: $kind:expr,)*) => {
        $(
            // SAFETY: no allocations are owned
            unsafe impl<'sval> Encode<'sval> for $ty<'sval> {
                #[inline]
                fn is_owned(&self) -> bool {
                    false
                }

                #[inline]
                fn is_borrowed(&self) -> bool {
                    true
                }

                #[inline]
                fn is_entry_tracking() -> bool {
                    $kind.is_entry_tracking()
                }

                #[inline]
                fn encode<S: RawStorageMut>(
                    self,
                    parts: &mut Parts<'sval, S>,
                ) -> Result<(), Error> {
                    // A borrowed fragment is stored as a fat pointer, but
                    // converting it into owned inlines it into a buffer
                    // capped at `MAX_PARTS_LEN`; rejecting oversized
                    // fragments here means that conversion can't overflow
                    if self.fragment.len() > super::MAX_PARTS_LEN {
                        return Err(Error::invalid_value(
                            "borrowed fragment exceeds the maximum buffer size",
                        ));
                    }

                    unsafe {
                        encode_native(parts, $kind.to_tag_byte(self.is_owned(), self.is_borrowed())?, self.fragment)
                    }
                }
            }
        )*
    };
}

impl_encode_pod!(
    (): Kind::Null,
    bool: Kind::Bool,
    u8: Kind::U8,
    u16: Kind::U16,
    u32: Kind::U32,
    u64: Kind::U64,
    u128: Kind::U128,
    i8: Kind::I8,
    i16: Kind::I16,
    i32: Kind::I32,
    i64: Kind::I64,
    i128: Kind::I128,
    f32: Kind::F32,
    f64: Kind::F64,
    TextHeader: Kind::Text,
    BinaryHeader: Kind::Binary,
    SeqHeader: Kind::Seq,
    SeqValueHeader: Kind::SeqValue,
    MapHeader: Kind::Map,
    MapKeyHeader: Kind::MapKey,
    MapValueHeader: Kind::MapValue,
);

impl_encode_optional_labeled_container!(
    TaggedHeader: Kind::Tagged,
    EnumHeader: Kind::Enum,
    TagPart: Kind::Tag,
    TupleHeader: Kind::Tuple,
    RecordHeader: Kind::Record,
    RecordTupleHeader: Kind::RecordTuple,
);

impl_encode_labeled_container!(
    RecordTupleValueHeader: Kind::RecordTupleValue,
    RecordValueHeader: Kind::RecordValue,
);

impl_encode_unlabeled_container!(
    TagHintPart: Kind::TagHint,
    TupleValueHeader: Kind::TupleValue,
);

impl_encode_borrowed_fragment!(
    BorrowedText: Kind::Text,
    BorrowedBinary: Kind::Binary,
);

#[inline]
fn encode_pod<T: IntoBytes + Immutable, S: RawStorageMut>(
    parts: &mut Parts<'_, S>,
    tag: u8,
    payload: T,
) -> Result<(), Error> {
    let size = 1 + mem::size_of::<T>();
    parts.reserve(size)?;

    // SAFETY: We reserved `1 + size_of::<T>()` bytes, exactly what's written
    unsafe {
        let mut w = parts.writer();
        w.write_byte(tag);
        w.write_pod(payload);
        parts.commit(size);
    }

    parts.update_ownership_tag(tag);

    Ok(())
}

// SAFETY: The caller must ensure `T` is valid to write
#[inline]
unsafe fn encode_native<T, S: RawStorageMut>(
    parts: &mut Parts<'_, S>,
    tag: u8,
    payload: T,
) -> Result<(), Error> {
    let size = 1 + mem::size_of::<T>();
    parts.reserve(size)?;

    // SAFETY: We reserved `1 + size_of::<T>()` bytes, exactly what's written
    unsafe {
        let mut w = parts.writer();
        w.write_byte(tag);
        w.write_native(payload);
        parts.commit(size);
    }

    parts.update_ownership_tag(tag);

    Ok(())
}

impl<'sval, S: RawStorageMut> Parts<'sval, S> {
    // SAFETY: The caller must ensure `len_at` is the offset of the length
    // field of a text/binary part whose payload ends the buffer, and that
    // `bytes` is valid UTF8 when that part is text
    #[inline(never)]
    pub(crate) unsafe fn push_raw_bytes(
        &mut self,
        len_at: usize,
        bytes: &[u8],
    ) -> Result<(), Error> {
        debug_assert!(
            matches!(
                Kind::from_tag_byte(self.buf.as_slice()[len_at - 1]),
                Kind::Text | Kind::Binary
            ),
            "`len_at` is not an inline text/binary part's length field"
        );

        self.reserve(bytes.len())?;
        self.buf.extend_from_slice(bytes)?;

        let len = self.len() - (len_at + mem::size_of::<u32>());
        self.patch_u32(len_at, len as u32);

        Ok(())
    }
}

impl<'sval, S: RawStorageMut> Parts<'sval, S> {
    #[inline]
    fn writer(&mut self) -> Writer {
        let len = self.buf.len();

        #[cfg(debug_assertions)]
        let cap = self.capacity();

        let ptr = self.buf.as_mut_ptr();

        Writer {
            // SAFETY: The pointer is one-past-the-end of the initialized bytes
            ptr: unsafe { ptr.add(len) },
            // SAFETY: The pointer is one-past-the-end of the reserved capacity
            #[cfg(debug_assertions)]
            end: unsafe { ptr.add(cap) },
        }
    }

    // SAFETY: The caller must ensure `n` bytes past the end were written
    // through a writer
    #[inline]
    unsafe fn commit(&mut self, n: usize) {
        debug_assert!(
            self.buf.len() + n <= self.capacity(),
            "attempt to commit bytes past the reserved capacity"
        );

        // SAFETY: Upheld by this function's contract
        unsafe {
            self.buf.advance_len(n);
        }
    }

    #[inline]
    fn update_ownership_tag(&mut self, tag: u8) {
        #[cfg(feature = "alloc")]
        {
            self.owned |= tag & super::TAG_OWNED != 0;
            self.borrowed |= tag & super::TAG_BORROWED != 0;
        }
        #[cfg(not(feature = "alloc"))]
        {
            let _ = tag;
        }
    }
}

/**
A cursor for writing a part into pre-reserved buffer space.
*/
struct Writer {
    ptr: *mut u8,
    // One-past-the-end of the buffer's reserved capacity
    #[cfg(debug_assertions)]
    end: *mut u8,
}

impl Writer {
    #[inline]
    fn assert_reserved(&self, n: usize) {
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                n <= self.end as usize - self.ptr as usize,
                "attempt to write past the reserved capacity"
            );
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = n;
        }
    }

    // SAFETY: The caller must ensure the buffer has reserved capacity for `b`
    #[inline]
    unsafe fn write_byte(&mut self, b: u8) {
        self.assert_reserved(1);

        // SAFETY: Upheld by this function's contract
        unsafe {
            self.ptr.write(b);
            self.ptr = self.ptr.add(1);
        }
    }

    // SAFETY: The caller must ensure the buffer has reserved capacity for `v`
    #[inline]
    unsafe fn write_pod<T: IntoBytes + Immutable>(&mut self, v: T) {
        let bytes = v.as_bytes();

        self.assert_reserved(bytes.len());

        // SAFETY: Upheld by this function's contract
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), self.ptr, bytes.len());
            self.ptr = self.ptr.add(bytes.len());
        }
    }

    // SAFETY: The caller must ensure the buffer has reserved capacity for `v`
    #[inline]
    unsafe fn write_native<T>(&mut self, v: T) {
        self.assert_reserved(mem::size_of::<T>());

        // SAFETY: Upheld by this function's contract
        unsafe {
            ptr::write_unaligned(self.ptr as *mut T, v);
            self.ptr = self.ptr.add(mem::size_of::<T>());
        }
    }
}
