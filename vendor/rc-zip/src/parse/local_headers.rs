use std::borrow::Cow;

use crate::{
    encoding::{detect_utf8, Encoding},
    error::{Error, FormatError, UnsupportedError},
    parse::{Method, MsdosTimestamp, Version},
};

use ownable::{IntoOwned, ToOwned};
use tracing::trace;
use winnow::{
    binary::{le_u16, le_u32, le_u64, le_u8},
    combinator::opt,
    error::{ContextError, ErrMode, ErrorKind, FromExternalError},
    seq,
    token::{literal, take},
    PResult, Parser, Partial,
};

use super::{zero_datetime, Entry, ExtraField, ExtraFieldSettings, Mode};

#[derive(Debug, ToOwned, IntoOwned)]
/// 4.3.7 Local file header
pub struct LocalFileHeader<'a> {
    /// version needed to extract
    pub reader_version: Version,

    /// general purpose bit flag
    pub flags: u16,

    /// compression method
    pub method: Method,

    /// last mod file datetime
    pub modified: MsdosTimestamp,

    /// crc-32
    pub crc32: u32,

    /// compressed size
    pub compressed_size: u32,

    /// uncompressed size
    pub uncompressed_size: u32,

    /// file name
    pub name: Cow<'a, [u8]>,

    /// extra field
    pub extra: Cow<'a, [u8]>,

    /// method-specific fields
    pub method_specific: MethodSpecific,
}

#[derive(Debug, ToOwned, IntoOwned)]
/// Method-specific properties following the local file header
pub enum MethodSpecific {
    /// No method-specific properties
    None,

    /// LZMA properties
    Lzma(LzmaProperties),
}

impl<'a> LocalFileHeader<'a> {
    /// The signature for a local file header
    pub const SIGNATURE: &'static str = "PK\x03\x04";

    /// Parser for the local file header
    pub fn parser(i: &mut Partial<&'a [u8]>) -> PResult<Self> {
        let _ = literal(Self::SIGNATURE).parse_next(i)?;

        let reader_version = Version::parser.parse_next(i)?;
        let flags = le_u16.parse_next(i)?;
        let method = le_u16.parse_next(i).map(Method::from)?;
        let modified = MsdosTimestamp::parser.parse_next(i)?;
        let crc32 = le_u32.parse_next(i)?;
        let compressed_size = le_u32.parse_next(i)?;
        let uncompressed_size = le_u32.parse_next(i)?;

        let name_len = le_u16.parse_next(i)?;
        let extra_len = le_u16.parse_next(i)?;

        let name = take(name_len).parse_next(i).map(Cow::Borrowed)?;
        let extra = take(extra_len).parse_next(i).map(Cow::Borrowed)?;

        let method_specific = match method {
            Method::Lzma => {
                let lzma_properties = LzmaProperties::parser.parse_next(i)?;
                if let Err(e) = lzma_properties.error_if_unsupported() {
                    return Err(ErrMode::Cut(ContextError::from_external_error(
                        i,
                        ErrorKind::Verify,
                        e,
                    )));
                }
                MethodSpecific::Lzma(lzma_properties)
            }
            _ => MethodSpecific::None,
        };

        Ok(Self {
            reader_version,
            flags,
            method,
            modified,
            crc32,
            compressed_size,
            uncompressed_size,
            name,
            extra,
            method_specific,
        })
    }

    /// Check for the presence of the bit flag that indicates a data descriptor
    /// is present after the file data.
    pub fn has_data_descriptor(&self) -> bool {
        // 4.3.9.1 This descriptor MUST exist if bit 3 of the general
        // purpose bit flag is set (see below).
        self.flags & 0b1000 != 0
    }

    /// Converts the local file header into an entry.
    pub fn as_entry(&self) -> Result<Entry, Error> {
        // see APPNOTE 4.4.4: Bit 11 is the language encoding flag (EFS)
        let has_utf8_flag = self.flags & 0x800 == 0;
        let encoding = if has_utf8_flag && detect_utf8(&self.name[..]).0 {
            Encoding::Utf8
        } else {
            Encoding::Cp437
        };
        let name = encoding.decode(&self.name[..])?;

        let mut entry = Entry {
            name,
            method: self.method,
            comment: Default::default(),
            modified: self.modified.to_datetime().unwrap_or_else(zero_datetime),
            created: None,
            accessed: None,
            header_offset: 0,
            reader_version: self.reader_version,
            flags: self.flags,
            uid: None,
            gid: None,
            crc32: self.crc32,
            compressed_size: self.compressed_size as _,
            uncompressed_size: self.uncompressed_size as _,
            mode: Mode(0),
        };

        if entry.name.ends_with('/') {
            // believe it or not, this is straight from the APPNOTE
            entry.mode |= Mode::DIR
        };

        let mut slice = Partial::new(&self.extra[..]);
        let settings = ExtraFieldSettings {
            compressed_size_u32: self.compressed_size,
            uncompressed_size_u32: self.uncompressed_size,
            header_offset_u32: 0,
        };

        while !slice.is_empty() {
            match ExtraField::mk_parser(settings).parse_next(&mut slice) {
                Ok(ef) => {
                    entry.set_extra_field(&ef);
                }
                Err(e) => {
                    trace!("extra field error: {:#?}", e);
                    return Err(FormatError::InvalidExtraField.into());
                }
            }
        }

        Ok(entry)
    }
}

/// 4.3.9  Data descriptor:
#[derive(Debug)]
pub struct DataDescriptorRecord {
    /// CRC32 checksum
    pub crc32: u32,
    /// Compressed size
    pub compressed_size: u64,
    /// Uncompressed size
    pub uncompressed_size: u64,
}

impl DataDescriptorRecord {
    const SIGNATURE: &'static str = "PK\x07\x08";

    /// Create a parser for the data descriptor record.
    pub fn mk_parser(is_zip64: bool) -> impl FnMut(&mut Partial<&'_ [u8]>) -> PResult<Self> {
        move |i| {
            // From appnote.txt:
            //
            // 4.3.9.3 Although not originally assigned a signature, the value
            // 0x08074b50 has commonly been adopted as a signature value for the
            // data descriptor record.  Implementers SHOULD be aware that ZIP files
            // MAY be encountered with or without this signature marking data
            // descriptors and SHOULD account for either case when reading ZIP files
            // to ensure compatibility.
            let _ = opt(literal(Self::SIGNATURE)).parse_next(i)?;

            if is_zip64 {
                seq! {Self {
                    crc32: le_u32,
                    compressed_size: le_u64,
                    uncompressed_size: le_u64,
                }}
                .parse_next(i)
            } else {
                seq! {Self {
                    crc32: le_u32,
                    compressed_size: le_u32.map(|x| x as u64),
                    uncompressed_size: le_u32.map(|x| x as u64),
                }}
                .parse_next(i)
            }
        }
    }
}

/// 5.8.5 LZMA Properties header
#[derive(Debug, ToOwned, IntoOwned)]
pub struct LzmaProperties {
    /// major version
    pub major: u8,
    /// minor version
    pub minor: u8,
    /// properties size
    pub properties_size: u16,
}

impl LzmaProperties {
    /// Parser for the LZMA properties header.
    pub fn parser(i: &mut Partial<&'_ [u8]>) -> PResult<Self> {
        // Note: the actual properties (5 bytes, contains dictionary size,
        // and various other settings) is not actually read, because lzma-rs
        // reads those properties itself.

        seq! {Self {
            major: le_u8,
            minor: le_u8,
            properties_size: le_u16,
        }}
        .parse_next(i)
    }

    /// Check if the LZMA version is supported.
    pub fn error_if_unsupported(&self) -> Result<(), Error> {
        if (self.major, self.minor) != (2, 0) {
            return Err(Error::Unsupported(
                UnsupportedError::LzmaVersionUnsupported {
                    minor: self.minor,
                    major: self.major,
                },
            ));
        }

        const LZMA_PROPERTIES_SIZE: u16 = 5;
        if self.properties_size != LZMA_PROPERTIES_SIZE {
            return Err(Error::Unsupported(
                UnsupportedError::LzmaPropertiesHeaderWrongSize {
                    expected: 5,
                    actual: self.properties_size,
                },
            ));
        }

        Ok(())
    }
}
