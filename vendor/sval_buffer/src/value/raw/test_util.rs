/*!
Test-only support for decoding a buffer back into inspectable values.
*/

use crate::{BinaryBuf, TextBuf};

#[cfg(feature = "alloc")]
use crate::std::vec::Vec;
#[cfg(not(feature = "alloc"))]
use alloc::vec::Vec;

use super::{
    read::{skip_part, str_from_utf8_unchecked, Reader},
    BinaryHeader, EnumHeader, Kind, MapKeyHeader, MapValueHeader, Parts, RawStorage, RecordHeader,
    RecordTupleHeader, RecordTupleValueHeader, RecordValueHeader, SeqHeader, SeqValueHeader,
    TagHintPart, TagPart, TaggedHeader, TextHeader, TupleHeader, TupleValueHeader, TAG_BORROWED,
};

impl<'sval, S: RawStorage> Parts<'sval, S> {
    pub(crate) fn decode(&self) -> Vec<ValueKind<'_>> {
        // SAFETY: `Parts` maintains a valid sequence of parts
        unsafe { decode(self.buf.as_slice()) }
    }

    #[cfg(feature = "alloc")]
    pub(crate) fn is_owned(&self) -> bool {
        self.owned
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ValueKind<'sval> {
    Null,
    Bool(bool),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    F32(f32),
    F64(f64),
    Text(TextBuf<'sval>),
    Binary(BinaryBuf<'sval>),
    Map {
        num_parts: usize,
        num_entries: usize,
    },
    MapKey {
        num_parts: usize,
    },
    MapValue {
        num_parts: usize,
    },
    Seq {
        num_parts: usize,
        num_entries: usize,
    },
    SeqValue {
        num_parts: usize,
    },
    Tag {
        tag: Option<sval::Tag>,
        label: Option<sval::Label<'static>>,
        index: Option<sval::Index>,
    },
    TagHint {
        tag: sval::Tag,
    },
    Enum {
        num_parts: usize,
        tag: Option<sval::Tag>,
        label: Option<sval::Label<'static>>,
        index: Option<sval::Index>,
    },
    Tagged {
        num_parts: usize,
        tag: Option<sval::Tag>,
        label: Option<sval::Label<'static>>,
        index: Option<sval::Index>,
    },
    Record {
        num_parts: usize,
        tag: Option<sval::Tag>,
        label: Option<sval::Label<'static>>,
        index: Option<sval::Index>,
        num_entries: Option<usize>,
    },
    RecordValue {
        num_parts: usize,
        tag: Option<sval::Tag>,
        label: sval::Label<'static>,
    },
    Tuple {
        num_parts: usize,
        tag: Option<sval::Tag>,
        label: Option<sval::Label<'static>>,
        index: Option<sval::Index>,
        num_entries: Option<usize>,
    },
    TupleValue {
        num_parts: usize,
        tag: Option<sval::Tag>,
        index: sval::Index,
    },
    RecordTuple {
        num_parts: usize,
        tag: Option<sval::Tag>,
        label: Option<sval::Label<'static>>,
        index: Option<sval::Index>,
        num_entries: Option<usize>,
    },
    RecordTupleValue {
        num_parts: usize,
        tag: Option<sval::Tag>,
        label: sval::Label<'static>,
        index: sval::Index,
    },
}

// SAFETY: The caller must ensure `bytes` encodes a valid sequence of parts
unsafe fn decode(bytes: &[u8]) -> Vec<ValueKind<'_>> {
    let mut out = Vec::new();
    let mut pos = 0;

    while pos < bytes.len() {
        let mut r = Reader { bytes, pos };

        // SAFETY: `pos` is a part boundary, upheld by this function's contract
        out.push(unsafe { clone_kind(&mut r) });

        pos = skip_part(bytes, pos);
    }

    out
}

// SAFETY: The caller must ensure `r` points to the tag byte of a valid part
unsafe fn clone_kind<'a>(r: &mut Reader<'a>) -> ValueKind<'a> {
    let tag = r.read_pod::<u8>();

    // SAFETY: A valid payload of the kind's type follows the tag byte;
    // plain-data payloads are read safely, native ones rely on that contract
    match Kind::from_tag_byte(tag) {
        Kind::Null => ValueKind::Null,
        Kind::Bool => ValueKind::Bool(r.read_bool()),
        Kind::U8 => ValueKind::U8(r.read_pod()),
        Kind::U16 => ValueKind::U16(r.read_pod()),
        Kind::U32 => ValueKind::U32(r.read_pod()),
        Kind::U64 => ValueKind::U64(r.read_pod()),
        Kind::U128 => ValueKind::U128(r.read_pod()),
        Kind::I8 => ValueKind::I8(r.read_pod()),
        Kind::I16 => ValueKind::I16(r.read_pod()),
        Kind::I32 => ValueKind::I32(r.read_pod()),
        Kind::I64 => ValueKind::I64(r.read_pod()),
        Kind::I128 => ValueKind::I128(r.read_pod()),
        Kind::F32 => ValueKind::F32(r.read_pod()),
        Kind::F64 => ValueKind::F64(r.read_pod()),
        Kind::Text => {
            if tag & TAG_BORROWED != 0 {
                ValueKind::Text(TextBuf::from(unsafe { r.read_native::<&'a str>() }))
            } else {
                let h = r.read_pod::<TextHeader>();

                // SAFETY: Inline text bytes are only ever copied out of
                // `str` fragments, so they're valid UTF-8
                let v = unsafe { str_from_utf8_unchecked(&r.bytes[r.pos..r.pos + h.len as usize]) };
                r.pos += h.len as usize;

                // Reconstruct inline data as a computed fragment
                #[cfg(feature = "alloc")]
                {
                    let mut buf = TextBuf::new();
                    buf.push_fragment_computed(v).unwrap();

                    ValueKind::Text(buf)
                }
                #[cfg(not(feature = "alloc"))]
                {
                    ValueKind::Text(TextBuf::from(v))
                }
            }
        }
        Kind::Binary => {
            if tag & TAG_BORROWED != 0 {
                ValueKind::Binary(BinaryBuf::from(unsafe { r.read_native::<&'a [u8]>() }))
            } else {
                let h = r.read_pod::<BinaryHeader>();

                let v = &r.bytes[r.pos..r.pos + h.len as usize];
                r.pos += h.len as usize;

                // Reconstruct inline data as a computed fragment
                #[cfg(feature = "alloc")]
                {
                    let mut buf = BinaryBuf::new();
                    buf.push_fragment_computed(v).unwrap();

                    ValueKind::Binary(buf)
                }
                #[cfg(not(feature = "alloc"))]
                {
                    ValueKind::Binary(BinaryBuf::from(v))
                }
            }
        }
        Kind::Map => {
            let h = r.read_pod::<SeqHeader>();
            ValueKind::Map {
                num_parts: count_parts(&r.bytes[r.pos..r.pos + h.len as usize]),
                num_entries: h.num_entries as usize,
            }
        }
        Kind::MapKey => {
            let h = r.read_pod::<MapKeyHeader>();
            ValueKind::MapKey {
                num_parts: count_parts(&r.bytes[r.pos..r.pos + h.len as usize]),
            }
        }
        Kind::MapValue => {
            let h = r.read_pod::<MapValueHeader>();
            ValueKind::MapValue {
                num_parts: count_parts(&r.bytes[r.pos..r.pos + h.len as usize]),
            }
        }
        Kind::Seq => {
            let h = r.read_pod::<SeqHeader>();
            ValueKind::Seq {
                num_parts: count_parts(&r.bytes[r.pos..r.pos + h.len as usize]),
                num_entries: h.num_entries as usize,
            }
        }
        Kind::SeqValue => {
            let h = r.read_pod::<SeqValueHeader>();
            ValueKind::SeqValue {
                num_parts: count_parts(&r.bytes[r.pos..r.pos + h.len as usize]),
            }
        }
        Kind::Tag => {
            let h = unsafe { r.read_native_non_copy::<TagPart>() };
            ValueKind::Tag {
                tag: h.tag.clone(),
                label: h.label.clone(),
                index: h.index.clone(),
            }
        }
        Kind::TagHint => ValueKind::TagHint {
            tag: unsafe { r.read_native::<TagHintPart>().tag },
        },
        Kind::Enum => {
            let h = unsafe { r.read_native_non_copy::<EnumHeader>() };
            ValueKind::Enum {
                num_parts: count_parts(&r.bytes[r.pos..r.pos + h.len as usize]),
                tag: h.tag.clone(),
                label: h.label.clone(),
                index: h.index.clone(),
            }
        }
        Kind::Tagged => {
            let h = unsafe { r.read_native_non_copy::<TaggedHeader>() };
            ValueKind::Tagged {
                num_parts: count_parts(&r.bytes[r.pos..r.pos + h.len as usize]),
                tag: h.tag.clone(),
                label: h.label.clone(),
                index: h.index.clone(),
            }
        }
        Kind::Record => {
            let h = unsafe { r.read_native_non_copy::<RecordHeader>() };
            ValueKind::Record {
                num_parts: count_parts(&r.bytes[r.pos..r.pos + h.len as usize]),
                tag: h.tag.clone(),
                label: h.label.clone(),
                index: h.index.clone(),
                num_entries: Some(h.num_entries as usize),
            }
        }
        Kind::RecordValue => {
            let h = unsafe { r.read_native_non_copy::<RecordValueHeader>() };
            ValueKind::RecordValue {
                num_parts: count_parts(&r.bytes[r.pos..r.pos + h.len as usize]),
                tag: h.tag.clone(),
                label: h.label.clone(),
            }
        }
        Kind::Tuple => {
            let h = unsafe { r.read_native_non_copy::<TupleHeader>() };
            ValueKind::Tuple {
                num_parts: count_parts(&r.bytes[r.pos..r.pos + h.len as usize]),
                tag: h.tag.clone(),
                label: h.label.clone(),
                index: h.index.clone(),
                num_entries: Some(h.num_entries as usize),
            }
        }
        Kind::TupleValue => {
            let h = unsafe { r.read_native::<TupleValueHeader>() };
            ValueKind::TupleValue {
                num_parts: count_parts(&r.bytes[r.pos..r.pos + h.len as usize]),
                tag: h.tag,
                index: h.index,
            }
        }
        Kind::RecordTuple => {
            let h = unsafe { r.read_native_non_copy::<RecordTupleHeader>() };
            ValueKind::RecordTuple {
                num_parts: count_parts(&r.bytes[r.pos..r.pos + h.len as usize]),
                tag: h.tag.clone(),
                label: h.label.clone(),
                index: h.index.clone(),
                num_entries: Some(h.num_entries as usize),
            }
        }
        Kind::RecordTupleValue => {
            let h = unsafe { r.read_native_non_copy::<RecordTupleValueHeader>() };
            ValueKind::RecordTupleValue {
                num_parts: count_parts(&r.bytes[r.pos..r.pos + h.len as usize]),
                tag: h.tag.clone(),
                label: h.label.clone(),
                index: h.index.clone(),
            }
        }
    }
}

// Count the parts in `bytes`; only meaningful when `bytes` encodes a valid
// sequence of parts
fn count_parts(bytes: &[u8]) -> usize {
    let mut pos = 0;
    let mut n = 0;

    while pos < bytes.len() {
        pos = skip_part(bytes, pos);
        n += 1;
    }

    n
}
