/*!
The payload structs that follow a tag byte in the encoded buffer.
*/

#[cfg(any(test, feature = "alloc"))]
use core::mem;

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

#[cfg(any(test, feature = "alloc"))]
use super::Kind;

#[repr(C)]
#[derive(Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub(crate) struct TextHeader {
    pub(crate) len: u32,
}

#[repr(C)]
#[derive(Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub(crate) struct BinaryHeader {
    pub(crate) len: u32,
}

#[repr(C)]
#[derive(Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub(crate) struct MapHeader {
    pub(crate) len: u32,
    pub(crate) num_entries: u32,
}

#[repr(C)]
#[derive(Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub(crate) struct MapKeyHeader {
    pub(crate) len: u32,
}

#[repr(C)]
#[derive(Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub(crate) struct MapValueHeader {
    pub(crate) len: u32,
}

#[repr(C)]
#[derive(Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub(crate) struct SeqHeader {
    pub(crate) len: u32,
    pub(crate) num_entries: u32,
}

#[repr(C)]
#[derive(Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub(crate) struct SeqValueHeader {
    pub(crate) len: u32,
}

#[repr(C)]
#[derive(Clone)]
pub(crate) struct TaggedHeader {
    pub(crate) len: u32,
    pub(crate) tag: Option<sval::Tag>,
    pub(crate) label: Option<sval::Label<'static>>,
    pub(crate) index: Option<sval::Index>,
}

#[repr(C)]
#[derive(Clone)]
pub(crate) struct EnumHeader {
    pub(crate) len: u32,
    pub(crate) tag: Option<sval::Tag>,
    pub(crate) label: Option<sval::Label<'static>>,
    pub(crate) index: Option<sval::Index>,
}

#[repr(C)]
#[derive(Clone)]
pub(crate) struct RecordHeader {
    pub(crate) len: u32,
    pub(crate) num_entries: u32,
    pub(crate) tag: Option<sval::Tag>,
    pub(crate) label: Option<sval::Label<'static>>,
    pub(crate) index: Option<sval::Index>,
}

#[repr(C)]
#[derive(Clone)]
pub(crate) struct TupleHeader {
    pub(crate) len: u32,
    pub(crate) num_entries: u32,
    pub(crate) tag: Option<sval::Tag>,
    pub(crate) label: Option<sval::Label<'static>>,
    pub(crate) index: Option<sval::Index>,
}

#[repr(C)]
#[derive(Clone)]
pub(crate) struct RecordTupleHeader {
    pub(crate) len: u32,
    pub(crate) num_entries: u32,
    pub(crate) tag: Option<sval::Tag>,
    pub(crate) label: Option<sval::Label<'static>>,
    pub(crate) index: Option<sval::Index>,
}

#[repr(C)]
#[derive(Clone)]
pub(crate) struct RecordValueHeader {
    pub(crate) len: u32,
    pub(crate) tag: Option<sval::Tag>,
    pub(crate) label: sval::Label<'static>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct TupleValueHeader {
    pub(crate) len: u32,
    pub(crate) tag: Option<sval::Tag>,
    pub(crate) index: sval::Index,
}

#[repr(C)]
#[derive(Clone)]
pub(crate) struct RecordTupleValueHeader {
    pub(crate) len: u32,
    pub(crate) tag: Option<sval::Tag>,
    pub(crate) label: sval::Label<'static>,
    pub(crate) index: sval::Index,
}

#[repr(C)]
#[derive(Clone)]
pub(crate) struct TagPart {
    pub(crate) tag: Option<sval::Tag>,
    pub(crate) label: Option<sval::Label<'static>>,
    pub(crate) index: Option<sval::Index>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct TagHintPart {
    pub(crate) tag: sval::Tag,
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct BorrowedText<'sval> {
    pub(crate) fragment: &'sval str,
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct BorrowedBinary<'sval> {
    pub(crate) fragment: &'sval [u8],
}

#[cfg(any(test, feature = "alloc"))]
#[inline]
pub(super) fn payload_size(kind: Kind) -> usize {
    match kind {
        Kind::Null => 0,
        Kind::Bool | Kind::U8 | Kind::I8 => mem::size_of::<u8>(),
        Kind::U16 | Kind::I16 => mem::size_of::<u16>(),
        Kind::U32 | Kind::I32 | Kind::F32 => mem::size_of::<u32>(),
        Kind::U64 | Kind::I64 | Kind::F64 => mem::size_of::<u64>(),
        Kind::U128 | Kind::I128 => mem::size_of::<u128>(),
        Kind::Map => mem::size_of::<MapHeader>(),
        Kind::Seq => mem::size_of::<SeqHeader>(),
        Kind::MapKey => mem::size_of::<MapKeyHeader>(),
        Kind::MapValue => mem::size_of::<MapValueHeader>(),
        Kind::SeqValue => mem::size_of::<SeqValueHeader>(),
        Kind::Tag => mem::size_of::<TagPart>(),
        Kind::TagHint => mem::size_of::<TagHintPart>(),
        Kind::Enum => mem::size_of::<EnumHeader>(),
        Kind::Tagged => mem::size_of::<TaggedHeader>(),
        Kind::Record => mem::size_of::<RecordHeader>(),
        Kind::Tuple => mem::size_of::<TupleHeader>(),
        Kind::RecordTuple => mem::size_of::<RecordTupleHeader>(),
        Kind::RecordValue => mem::size_of::<RecordValueHeader>(),
        Kind::TupleValue => mem::size_of::<TupleValueHeader>(),
        Kind::RecordTupleValue => mem::size_of::<RecordTupleValueHeader>(),
        // We'll never call `payload_size` for buffered parts
        Kind::Text | Kind::Binary => unreachable!("buffered parts are variable-size"),
    }
}
