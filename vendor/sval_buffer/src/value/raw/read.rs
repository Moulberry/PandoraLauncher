/*!
Reading parts back out of the buffer: raw payload reads, part skipping,
and streaming a buffer through an `sval::Stream`.
*/

use core::{
    mem::{self, ManuallyDrop},
    ptr, str,
};
use zerocopy::{try_transmute, FromBytes};

use super::{
    BinaryHeader, EnumHeader, Kind, MapHeader, MapKeyHeader, MapValueHeader, Parts, RawStorage,
    RecordHeader, RecordTupleHeader, RecordTupleValueHeader, RecordValueHeader, SeqHeader,
    SeqValueHeader, TagHintPart, TagPart, TaggedHeader, TextHeader, TupleHeader, TupleValueHeader,
    TAG_BORROWED,
};

#[cfg(any(test, feature = "alloc"))]
use super::header::payload_size;

#[inline]
pub(super) fn read_pod_at<T: FromBytes>(bytes: &[u8], at: usize) -> T {
    // Slicing to the exact size lets the size check inside
    // `read_from_bytes` fold away, leaving a single bounds check
    T::read_from_bytes(&bytes[at..at + mem::size_of::<T>()])
        .expect("attempt to read a part payload past the end of the buffer")
}

// SAFETY: The caller must ensure a valid `T` is encoded at `at`
#[inline]
pub(super) unsafe fn read_native_at<T>(bytes: &[u8], at: usize) -> T {
    debug_assert!(
        at + mem::size_of::<T>() <= bytes.len(),
        "attempt to read a part payload past the end of the buffer"
    );

    ptr::read_unaligned(bytes.as_ptr().add(at) as *const T)
}

// SAFETY: The caller must ensure `bytes` is valid UTF-8
#[inline]
pub(super) unsafe fn str_from_utf8_unchecked(bytes: &[u8]) -> &str {
    debug_assert!(
        str::from_utf8(bytes).is_ok(),
        "inline text is not valid UTF-8"
    );

    str::from_utf8_unchecked(bytes)
}

#[cfg(any(test, feature = "alloc"))]
#[inline]
pub(super) fn skip_part(bytes: &[u8], pos: usize) -> usize {
    let tag = bytes[pos];
    let kind = Kind::from_tag_byte(tag);

    let end = match kind {
        Kind::Text | Kind::Binary => {
            if tag & TAG_BORROWED != 0 {
                debug_assert_eq!(mem::size_of::<&str>(), mem::size_of::<&[u8]>());

                // Borrowed text/binary keeps a fat pointer
                pos + 1 + mem::size_of::<&str>()
            } else {
                debug_assert_eq!(mem::size_of::<u32>(), mem::size_of::<TextHeader>());
                debug_assert_eq!(mem::size_of::<u32>(), mem::size_of::<BinaryHeader>());

                // An inline part is a length followed by that many
                // bytes of data
                let len = read_pod_at::<u32>(bytes, pos + 1) as usize;

                pos + 1 + mem::size_of::<u32>() + len
            }
        }
        _ => pos + 1 + payload_size(kind),
    };

    debug_assert!(end <= bytes.len(), "malformed part");

    end
}

/**
A cursor for reading encoded parts out of a buffer.
*/
pub(super) struct Reader<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) pos: usize,
}

impl<'a> Reader<'a> {
    #[inline]
    pub(super) fn read_pod<T: FromBytes>(&mut self) -> T {
        let v = read_pod_at::<T>(self.bytes, self.pos);

        self.pos += mem::size_of::<T>();
        v
    }

    #[inline]
    pub(super) fn read_bool(&mut self) -> bool {
        try_transmute!(self.read_pod::<u8>()).expect("part payload holds an invalid bool")
    }

    // SAFETY: The caller must ensure `self.bytes` encodes a valid `T` at
    // `self.pos`
    #[inline]
    pub(super) unsafe fn read_native<T: Copy>(&mut self) -> T {
        // SAFETY: Upheld by this function's contract
        let v = unsafe { read_native_at::<T>(self.bytes, self.pos) };

        self.pos += mem::size_of::<T>();
        v
    }

    // SAFETY: The caller must ensure `self.bytes` encodes a valid `T` at
    // `self.pos`; the read value aliases the buffer's copy, so the caller
    // must not drop it
    #[inline]
    pub(super) unsafe fn read_native_non_copy<T>(&mut self) -> ManuallyDrop<T> {
        // SAFETY: Upheld by this function's contract
        let v = ManuallyDrop::new(unsafe { read_native_at::<T>(self.bytes, self.pos) });

        self.pos += mem::size_of::<T>();
        v
    }
}

pub(crate) fn stream_parts<'sval, PS: RawStorage, S: sval::Stream<'sval> + ?Sized>(
    parts: &Parts<'sval, PS>,
    stream: &mut S,
) -> sval::Result {
    let bytes = parts.buf.as_slice();

    // If the buffer is empty then stream null
    if bytes.is_empty() {
        return stream.null();
    }

    // SAFETY: `Parts` maintains a valid sequence of parts
    unsafe { stream_all::<S>(bytes, stream) }
}

// SAFETY: The caller must ensure `bytes` encodes a valid sequence of parts
unsafe fn stream_all<'sval, S: sval::Stream<'sval> + ?Sized>(
    bytes: &[u8],
    stream: &mut S,
) -> sval::Result {
    let mut pos = 0;

    while pos < bytes.len() {
        // SAFETY: `pos` is a part boundary, upheld by this function's contract
        pos = unsafe { stream_part::<S>(bytes, pos, stream)? };
    }

    Ok(())
}

// SAFETY: The caller must ensure `bytes` encodes a valid sequence of parts
// and `pos` points to a tag byte
unsafe fn stream_part<'sval, S: sval::Stream<'sval> + ?Sized>(
    bytes: &[u8],
    pos: usize,
    stream: &mut S,
) -> Result<usize, sval::Error> {
    let mut r = Reader { bytes, pos };

    // Stream a container's body, sitting in the next `len` bytes.
    macro_rules! body {
        ($len:expr, $begin:expr, $end:expr) => {{
            let len = $len;
            let start = r.pos;

            $begin;
            // SAFETY: A container's body is itself a valid sequence of parts
            unsafe {
                stream_all::<S>(&bytes[start..start + len], stream)?;
            }
            $end;

            r.pos = start + len;
        }};
    }

    let tag = r.read_pod::<u8>();

    // SAFETY: A valid payload of the kind's type follows the tag byte;
    // plain-data payloads are read safely, native ones rely on that contract
    match Kind::from_tag_byte(tag) {
        Kind::Null => stream.null()?,
        Kind::Bool => stream.bool(r.read_bool())?,
        Kind::U8 => stream.u8(r.read_pod())?,
        Kind::U16 => stream.u16(r.read_pod())?,
        Kind::U32 => stream.u32(r.read_pod())?,
        Kind::U64 => stream.u64(r.read_pod())?,
        Kind::U128 => stream.u128(r.read_pod())?,
        Kind::I8 => stream.i8(r.read_pod())?,
        Kind::I16 => stream.i16(r.read_pod())?,
        Kind::I32 => stream.i32(r.read_pod())?,
        Kind::I64 => stream.i64(r.read_pod())?,
        Kind::I128 => stream.i128(r.read_pod())?,
        Kind::F32 => stream.f32(r.read_pod())?,
        Kind::F64 => stream.f64(r.read_pod())?,
        Kind::Text => {
            if tag & TAG_BORROWED != 0 {
                let v = unsafe { r.read_native::<&'sval str>() };

                stream.text_begin(Some(v.len()))?;
                stream.text_fragment(v)?;
                stream.text_end()?;
            } else {
                let h = r.read_pod::<TextHeader>();

                // SAFETY: Inline text bytes are only ever copied out of
                // `str` fragments, so they're valid UTF-8
                let v = unsafe { str_from_utf8_unchecked(&bytes[r.pos..r.pos + h.len as usize]) };
                r.pos += h.len as usize;

                stream.text_begin(Some(h.len as usize))?;
                stream.text_fragment_computed(v)?;
                stream.text_end()?;
            }
        }
        Kind::Binary => {
            if tag & TAG_BORROWED != 0 {
                let v = unsafe { r.read_native::<&'sval [u8]>() };

                stream.binary_begin(Some(v.len()))?;
                stream.binary_fragment(v)?;
                stream.binary_end()?;
            } else {
                let h = r.read_pod::<BinaryHeader>();

                let v = &bytes[r.pos..r.pos + h.len as usize];
                r.pos += h.len as usize;

                stream.binary_begin(Some(h.len as usize))?;
                stream.binary_fragment_computed(v)?;
                stream.binary_end()?;
            }
        }
        Kind::Map => {
            let h = r.read_pod::<MapHeader>();
            body!(
                h.len as usize,
                stream.map_begin(Some(h.num_entries as usize))?,
                stream.map_end()?
            );
        }
        Kind::MapKey => {
            let h = r.read_pod::<MapKeyHeader>();
            body!(
                h.len as usize,
                stream.map_key_begin()?,
                stream.map_key_end()?
            );
        }
        Kind::MapValue => {
            let h = r.read_pod::<MapValueHeader>();
            body!(
                h.len as usize,
                stream.map_value_begin()?,
                stream.map_value_end()?
            );
        }
        Kind::Seq => {
            let h = r.read_pod::<SeqHeader>();
            body!(
                h.len as usize,
                stream.seq_begin(Some(h.num_entries as usize))?,
                stream.seq_end()?
            );
        }
        Kind::SeqValue => {
            let h = r.read_pod::<SeqValueHeader>();
            body!(
                h.len as usize,
                stream.seq_value_begin()?,
                stream.seq_value_end()?
            );
        }
        Kind::Tag => {
            let h = unsafe { r.read_native_non_copy::<TagPart>() };
            stream.tag(h.tag.as_ref(), h.label.as_ref(), h.index.as_ref())?;
        }
        Kind::TagHint => {
            let h = unsafe { r.read_native::<TagHintPart>() };
            stream.tag_hint(&h.tag)?;
        }
        Kind::Enum => {
            let h = unsafe { r.read_native_non_copy::<EnumHeader>() };
            body!(
                h.len as usize,
                stream.enum_begin(h.tag.as_ref(), h.label.as_ref(), h.index.as_ref())?,
                stream.enum_end(h.tag.as_ref(), h.label.as_ref(), h.index.as_ref())?
            );
        }
        Kind::Tagged => {
            let h = unsafe { r.read_native_non_copy::<TaggedHeader>() };
            body!(
                h.len as usize,
                stream.tagged_begin(h.tag.as_ref(), h.label.as_ref(), h.index.as_ref())?,
                stream.tagged_end(h.tag.as_ref(), h.label.as_ref(), h.index.as_ref())?
            );
        }
        Kind::Record => {
            let h = unsafe { r.read_native_non_copy::<RecordHeader>() };
            body!(
                h.len as usize,
                stream.record_begin(
                    h.tag.as_ref(),
                    h.label.as_ref(),
                    h.index.as_ref(),
                    Some(h.num_entries as usize),
                )?,
                stream.record_end(h.tag.as_ref(), h.label.as_ref(), h.index.as_ref())?
            );
        }
        Kind::RecordValue => {
            let h = unsafe { r.read_native_non_copy::<RecordValueHeader>() };
            body!(
                h.len as usize,
                stream.record_value_begin(h.tag.as_ref(), &h.label)?,
                stream.record_value_end(h.tag.as_ref(), &h.label)?
            );
        }
        Kind::Tuple => {
            let h = unsafe { r.read_native_non_copy::<TupleHeader>() };
            body!(
                h.len as usize,
                stream.tuple_begin(
                    h.tag.as_ref(),
                    h.label.as_ref(),
                    h.index.as_ref(),
                    Some(h.num_entries as usize),
                )?,
                stream.tuple_end(h.tag.as_ref(), h.label.as_ref(), h.index.as_ref())?
            );
        }
        Kind::TupleValue => {
            let h = unsafe { r.read_native::<TupleValueHeader>() };
            body!(
                h.len as usize,
                stream.tuple_value_begin(h.tag.as_ref(), &h.index)?,
                stream.tuple_value_end(h.tag.as_ref(), &h.index)?
            );
        }
        Kind::RecordTuple => {
            let h = unsafe { r.read_native_non_copy::<RecordTupleHeader>() };
            body!(
                h.len as usize,
                stream.record_tuple_begin(
                    h.tag.as_ref(),
                    h.label.as_ref(),
                    h.index.as_ref(),
                    Some(h.num_entries as usize),
                )?,
                stream.record_tuple_end(h.tag.as_ref(), h.label.as_ref(), h.index.as_ref())?
            );
        }
        Kind::RecordTupleValue => {
            let h = unsafe { r.read_native_non_copy::<RecordTupleValueHeader>() };
            body!(
                h.len as usize,
                stream.record_tuple_value_begin(h.tag.as_ref(), &h.label, &h.index)?,
                stream.record_tuple_value_end(h.tag.as_ref(), &h.label, &h.index)?
            );
        }
    }

    Ok(r.pos)
}
