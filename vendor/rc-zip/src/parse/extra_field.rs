use std::borrow::Cow;

use ownable::{IntoOwned, ToOwned};
use winnow::{
    binary::{le_u16, le_u32, le_u64, le_u8, length_take},
    combinator::{opt, preceded, repeat_till},
    error::{ErrMode, ErrorKind, ParserError, StrContext},
    seq,
    token::{literal, take},
    PResult, Parser, Partial,
};

use crate::parse::NtfsTimestamp;

/// 4.4.28 extra field: (Variable)
pub(crate) struct ExtraFieldRecord<'a> {
    pub(crate) tag: u16,
    pub(crate) payload: &'a [u8],
}

impl<'a> ExtraFieldRecord<'a> {
    pub(crate) fn parser(i: &mut Partial<&'a [u8]>) -> PResult<Self> {
        seq! {Self {
            tag: le_u16,
            payload: length_take(le_u16),
        }}
        .parse_next(i)
    }
}

/// Useful because zip64 extended information extra field has fixed order *but*
/// optional fields. From the appnote:
///
/// If one of the size or offset fields in the Local or Central directory record
/// is too small to hold the required data, a Zip64 extended information record
/// is created. The order of the fields in the zip64 extended information record
/// is fixed, but the fields MUST only appear if the corresponding Local or
/// Central directory record field is set to 0xFFFF or 0xFFFFFFFF.
#[derive(Debug, Clone, Copy)]
pub struct ExtraFieldSettings {
    /// The uncompressed size field read from a local or central directory record
    /// If this is 0xFFFF_FFFF, then the zip64 extra field uncompressed size
    /// field will be present.
    pub uncompressed_size_u32: u32,

    /// The compressed size field read from a local or central directory record
    /// If this is 0xFFFF_FFFF, then the zip64 extra field compressed size
    /// field will be present.
    pub compressed_size_u32: u32,

    /// The header offset field read from a central directory record (or zero
    /// for local directory records). If this is 0xFFFF_FFFF, then the zip64
    /// extra field header offset field will be present.
    pub header_offset_u32: u32,
}

/// Information stored in the central directory header `extra` field
///
/// This typically contains timestamps, file sizes and offsets, file mode, uid/gid, etc.
///
/// See `extrafld.txt` in this crate's source distribution.
#[derive(Clone)]
pub enum ExtraField<'a> {
    /// Zip64 extended information extra field
    Zip64(ExtraZip64Field),
    /// Extended timestamp
    Timestamp(ExtraTimestampField),
    /// UNIX & Info-Zip UNIX
    Unix(ExtraUnixField<'a>),
    /// New UNIX extra field
    NewUnix(ExtraNewUnixField),
    /// NTFS (Win9x/WinNT FileTimes)
    Ntfs(ExtraNtfsField),
    /// Unknown extra field, with tag
    Unknown {
        /// tag of the extra field
        tag: u16,
    },
}

impl<'a> ExtraField<'a> {
    /// Make a parser for extra fields, given the settings for the zip64 extra
    /// field (which depend on whether the u32 values are 0xFFFF_FFFF or not)
    pub fn mk_parser(
        settings: ExtraFieldSettings,
    ) -> impl FnMut(&mut Partial<&'a [u8]>) -> PResult<Self> {
        move |i| {
            use ExtraField as EF;
            let rec = ExtraFieldRecord::parser.parse_next(i)?;
            let payload = &mut Partial::new(rec.payload);

            let variant = match rec.tag {
                ExtraZip64Field::TAG => opt(ExtraZip64Field::mk_parser(settings).map(EF::Zip64))
                    .context(StrContext::Label("zip64"))
                    .parse_next(payload)?,
                ExtraTimestampField::TAG => opt(ExtraTimestampField::parser.map(EF::Timestamp))
                    .context(StrContext::Label("timestamp"))
                    .parse_next(payload)?,
                ExtraNtfsField::TAG => {
                    opt(ExtraNtfsField::parser.map(EF::Ntfs)).parse_next(payload)?
                }
                ExtraUnixField::TAG => {
                    opt(ExtraUnixField::parser.map(EF::Unix)).parse_next(payload)?
                }
                ExtraNewUnixField::TAG => {
                    opt(ExtraNewUnixField::parser.map(EF::NewUnix)).parse_next(payload)?
                }
                _ => None,
            }
            .unwrap_or(EF::Unknown { tag: rec.tag });

            Ok(variant)
        }
    }
}

/// 4.5.3 -Zip64 Extended Information Extra Field (0x0001)
#[derive(Clone, Default)]
pub struct ExtraZip64Field {
    /// 64-bit uncompressed size
    pub uncompressed_size: u64,

    /// 64-bit compressed size
    pub compressed_size: u64,

    /// 64-bit header offset
    pub header_offset: u64,

    /// 32-bit disk start number
    pub disk_start: Option<u32>,
}

impl ExtraZip64Field {
    const TAG: u16 = 0x0001;

    pub(crate) fn mk_parser(
        settings: ExtraFieldSettings,
    ) -> impl FnMut(&mut Partial<&'_ [u8]>) -> PResult<Self> {
        move |i| {
            let uncompressed_size = if settings.uncompressed_size_u32 == 0xFFFF_FFFF {
                le_u64.parse_next(i)?
            } else {
                settings.uncompressed_size_u32 as u64
            };
            let compressed_size = if settings.compressed_size_u32 == 0xFFFF_FFFF {
                le_u64.parse_next(i)?
            } else {
                settings.compressed_size_u32 as u64
            };
            let header_offset = if settings.header_offset_u32 == 0xFFFF_FFFF {
                le_u64.parse_next(i)?
            } else {
                settings.header_offset_u32 as u64
            };
            let disk_start = opt(le_u32.complete_err()).parse_next(i)?;

            Ok(Self {
                uncompressed_size,
                compressed_size,
                header_offset,
                disk_start,
            })
        }
    }
}

/// Extended timestamp extra field
#[derive(Clone)]
pub struct ExtraTimestampField {
    /// number of seconds since epoch
    pub mtime: u32,
}

impl ExtraTimestampField {
    const TAG: u16 = 0x5455;

    fn parser(i: &mut Partial<&'_ [u8]>) -> PResult<Self> {
        preceded(
            // 1 byte of flags, if bit 0 is set, modification time is present
            le_u8.verify(|x| x & 0b1 != 0),
            seq! {Self { mtime: le_u32 }},
        )
        .parse_next(i)
    }
}

/// 4.5.7 -UNIX Extra Field (0x000d):
#[derive(Clone, ToOwned, IntoOwned)]
pub struct ExtraUnixField<'a> {
    /// file last access time
    pub atime: u32,
    /// file last modification time
    pub mtime: u32,
    /// file user id
    pub uid: u16,
    /// file group id
    pub gid: u16,
    /// variable length data field
    pub data: Cow<'a, [u8]>,
}

impl<'a> ExtraUnixField<'a> {
    const TAG: u16 = 0x000d;
    // FIXME(cosmic): add support after `ExtraUnixField`'s structure accounts for info-zip storing
    // less information than with `::TAG`. the smaller variant stores just atime and mtime while
    // the larger additionally store a `uid` and `gid` as well. the variant is determined based off
    // of `t_size`, with no additional variable data
    const _TAG_INFOZIP: u16 = 0x5855;

    fn parser(i: &mut Partial<&'a [u8]>) -> PResult<Self> {
        let t_size = le_u16.parse_next(i)?;

        // t_size includes the size of the atime .. gid fields, totalling 12 bytes.
        let t_size = t_size
            .checked_sub(12)
            .ok_or(ErrMode::from_error_kind(i, ErrorKind::Verify))?;

        seq! {Self {
            atime: le_u32,
            mtime: le_u32,
            uid: le_u16,
            gid: le_u16,
            data: take(t_size).map(Cow::Borrowed),
        }}
        .parse_next(i)
    }
}

/// Info-ZIP New Unix Extra Field:
/// ====================================
///
/// Currently stores Unix UIDs/GIDs up to 32 bits.
/// (Last Revision 20080509)
///
/// ```text
/// Value         Size        Description
/// -----         ----        -----------
/// 0x7875        Short       tag for this extra block type ("ux")
/// TSize         Short       total data size for this block
/// Version       1 byte      version of this extra field, currently 1
/// UIDSize       1 byte      Size of UID field
/// UID           Variable    UID for this entry
/// GIDSize       1 byte      Size of GID field
/// GID           Variable    GID for this entry
/// ```
#[derive(Clone)]
pub struct ExtraNewUnixField {
    /// file user id
    pub uid: u64,

    /// file group id
    pub gid: u64,
}

impl ExtraNewUnixField {
    const TAG: u16 = 0x7875;

    fn parser(i: &mut Partial<&'_ [u8]>) -> PResult<Self> {
        let _ = literal("\x01").parse_next(i)?;
        seq! {Self {
            uid: Self::parse_variable_length_integer,
            gid: Self::parse_variable_length_integer,
        }}
        .parse_next(i)
    }

    fn parse_variable_length_integer(i: &mut Partial<&'_ [u8]>) -> PResult<u64> {
        let slice = length_take(le_u8).parse_next(i)?;
        if let Some(u) = match slice.len() {
            1 => Some(le_u8.parse_peek(slice)?.1 as u64),
            2 => Some(le_u16.parse_peek(slice)?.1 as u64),
            4 => Some(le_u32.parse_peek(slice)?.1 as u64),
            8 => Some(le_u64.parse_peek(slice)?.1),
            _ => None,
        } {
            Ok(u)
        } else {
            Err(ErrMode::from_error_kind(i, ErrorKind::Alt))
        }
    }
}

/// 4.5.5 -NTFS Extra Field (0x000a):
#[derive(Clone)]
pub struct ExtraNtfsField {
    /// NTFS attributes
    pub attrs: Vec<NtfsAttr>,
}

impl ExtraNtfsField {
    const TAG: u16 = 0x000a;

    fn parser(i: &mut Partial<&'_ [u8]>) -> PResult<Self> {
        let _ = take(4_usize).parse_next(i)?; // reserved (unused)
        seq! {Self {
            // from the winnow docs:
            //   Parsers like repeat do not know when an eof is from insufficient
            //   data or the end of the stream, causing them to always report
            //   Incomplete.
            // using repeat_till with eof combinator to work around this:
            attrs: repeat_till(0.., NtfsAttr::parser, winnow::combinator::eof).map(|x| x.0),
        }}
        .parse_next(i)
    }
}

/// NTFS attribute for zip entries (mostly timestamps)
#[derive(Clone)]
pub enum NtfsAttr {
    /// NTFS attribute 1, which contains modified/accessed/created timestamps
    Attr1(NtfsAttr1),

    /// Unknown NTFS attribute
    Unknown {
        /// tag of the attribute
        tag: u16,
    },
}

impl NtfsAttr {
    fn parser(i: &mut Partial<&'_ [u8]>) -> PResult<Self> {
        let tag = le_u16.parse_next(i)?;
        let payload = length_take(le_u16).parse_next(i)?;

        match tag {
            0x0001 => NtfsAttr1::parser
                .parse_peek(Partial::new(payload))
                .map(|(_, attr)| NtfsAttr::Attr1(attr)),
            _ => Ok(NtfsAttr::Unknown { tag }),
        }
    }
}

/// NTFS attribute 1, which contains modified/accessed/created timestamps
#[derive(Clone)]
pub struct NtfsAttr1 {
    /// modified time
    pub mtime: NtfsTimestamp,

    /// accessed time
    pub atime: NtfsTimestamp,

    /// created time
    pub ctime: NtfsTimestamp,
}

impl NtfsAttr1 {
    fn parser(i: &mut Partial<&'_ [u8]>) -> PResult<Self> {
        seq! {Self {
            mtime: NtfsTimestamp::parser,
            atime: NtfsTimestamp::parser,
            ctime: NtfsTimestamp::parser,
        }}
        .parse_next(i)
    }
}
