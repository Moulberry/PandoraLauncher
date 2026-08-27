/*!
A raw binary encoding for `Value`s.

The encoding is an optimized `Vec<Part>`, where `Part` is an enum of each
possible method callable on `Stream`. The reason we don't just do that is because
Rust's enums are sized to fit their largest member, so we waste a lot of space
using an array of fixed-sized variants.

The encoding looks like this:

```text
    +-----+----------------+
    | tag | payload struct |
    +-----+----------------+
```

Where the `tag` is a single byte that determines the size and shape of the payload
that follows:

```text
bit:    7          6         5        4 .. 0
    +----------+----------+--------+---------+
    |  OWNED   | BORROWED | unused |  KIND   |
    +----------+----------+--------+---------+
```

- `OWNED`: When set, the payload owns a non-copy value that needs to be dropped
  and cloned.
- `BORROWED`: When set, the payload borrows for `'sval`, so will need to be
  buffered inline  when converting into owned.
- `KIND`: An identifier for the variant.

What follows the tag byte depends on the kind. Primitives, tags, and tag
hints are a bare `#[repr(C)]` payload struct, written unaligned.

Containers store the length of their value as a field. This makes the buffer a
sort of skip-list that can be used to stream a flat sequence of tokens as a tree-like structure:

```text
    +-----+------------+----------------+~ ~ ~ ~ ~ ~ ~ ~ ~ ~+
    | tag | len: usize | rest of header |  body: len bytes  |
    +-----+------------+----------------+~ ~ ~ ~ ~ ~ ~ ~ ~ ~+
```

Borrowed text and binary (`BORROWED` set) store the reference itself as the payload:

```text
    +-----+--------------------------+
    | tag | &'sval str / &'sval [u8] |
    +-----+--------------------------+
```
*/

// NOTE: The safe/unsafe boundary between this `raw` module and `value` where it's used
// is still a bit unsatisfying. It's possible to misuse safe functions here to produce an
// unsound stream later. The `value` module itself guarantees its public API is UB-free,
// but this should still be better cleaned up

mod header;
mod parts;
mod read;
mod write;

#[cfg(feature = "alloc")]
mod owned;

#[cfg(not(feature = "alloc"))]
mod array_vec;

#[cfg(test)]
pub(crate) mod test_util;

pub(crate) use self::{
    header::{
        BinaryHeader, BorrowedBinary, BorrowedText, EnumHeader, MapHeader, MapKeyHeader,
        MapValueHeader, RecordHeader, RecordTupleHeader, RecordTupleValueHeader, RecordValueHeader,
        SeqHeader, SeqValueHeader, TagHintPart, TagPart, TaggedHeader, TextHeader, TupleHeader,
        TupleValueHeader,
    },
    parts::{into_value_parts, Parts, RawStorage, RawStorageMut, ValueBufParts, ValueParts},
    read::stream_parts,
    write::Encode,
};

#[cfg(feature = "alloc")]
pub(crate) use self::owned::make_owned;

#[cfg(not(feature = "alloc"))]
pub(crate) use self::array_vec::ArrayVec;

use crate::Error;

use zerocopy::{try_transmute, Immutable, KnownLayout, TryFromBytes};

const KIND_MASK: u8 = 0b0001_1111;

#[cfg(feature = "alloc")]
const TAG_OWNED: u8 = 0b1000_0000;
const TAG_BORROWED: u8 = 0b0100_0000;

// The number of bytes reserved for a value in no-std builds.
//
// This needs to be large enough to fit the largest single leaf part (a `Tag` with a tag, label, and index).
const PARTS_CAP: usize = 256;

// The maximum size of an encoded value
pub(crate) const MAX_PARTS_LEN: usize = (u32::MAX - 8) as usize;

#[derive(Clone, Copy, PartialEq, Eq, TryFromBytes, Immutable, KnownLayout)]
#[repr(u8)]
enum Kind {
    // is_primitive (0..=15)
    Null = 0,
    Bool = 1,
    U8 = 2,
    U16 = 3,
    U32 = 4,
    U64 = 5,
    U128 = 6,
    I8 = 7,
    I16 = 8,
    I32 = 9,
    I64 = 10,
    I128 = 11,
    F32 = 12,
    F64 = 13,
    Tag = 14,
    TagHint = 15,
    // is_container (16..=30)
    Text = 16,
    Binary = 17,
    MapKey = 18,
    MapValue = 19,
    SeqValue = 20,
    Enum = 21,
    Tagged = 22,
    RecordValue = 23,
    TupleValue = 24,
    RecordTupleValue = 25,
    // is_entry_tracking (26..=30)
    Map = 26,
    Seq = 27,
    Record = 28,
    Tuple = 29,
    RecordTuple = 30,
    // NOTE: See `KIND_MASK` and `Kind::is_*` before adding new variants
}

impl Kind {
    #[inline]
    fn from_tag_byte(b: u8) -> Kind {
        try_transmute!(b & KIND_MASK).expect("tag byte holds an invalid kind")
    }

    #[inline]
    fn to_tag_byte(&self, owned: bool, borrowed: bool) -> Result<u8, Error> {
        let mut tag = *self as u8;

        debug_assert!(tag <= KIND_MASK);

        #[cfg(feature = "alloc")]
        {
            if owned {
                tag |= TAG_OWNED;
            }
        }
        #[cfg(not(feature = "alloc"))]
        {
            // NOTE: An owned header is still possible if `sval`'s `alloc` feature is enabled,
            // even if this crate's is not. Without our own `alloc` there are no drop or
            // deep-clone walks, so an owned header would leak; refuse it instead
            if owned {
                return Err(Error::no_alloc("owned value part"));
            }
        }

        if borrowed {
            tag |= TAG_BORROWED;
        }

        Ok(tag)
    }

    #[inline]
    const fn is_container(&self) -> bool {
        let tag = *self as u8;

        tag >= 16
    }

    #[inline]
    const fn is_entry_tracking(&self) -> bool {
        let tag = *self as u8;

        tag >= 26
    }
}
