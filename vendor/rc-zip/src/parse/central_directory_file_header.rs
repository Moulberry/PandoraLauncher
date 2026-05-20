use std::borrow::Cow;

use ownable::{IntoOwned, ToOwned};
use tracing::trace;
use winnow::{
    binary::{le_u16, le_u32},
    prelude::PResult,
    token::{literal, take},
    Parser, Partial,
};

use crate::{
    encoding::detect_utf8,
    encoding::Encoding,
    error::{Error, FormatError},
    parse::{
        zero_datetime, Entry, ExtraField, ExtraFieldSettings, HostSystem, Mode, MsdosMode,
        MsdosTimestamp, UnixMode, Version,
    },
};

use super::Method;

/// 4.3.12 Central directory structure: File header
#[derive(IntoOwned, ToOwned)]
pub struct CentralDirectoryFileHeader<'a> {
    /// version made by
    pub creator_version: Version,

    /// version needed to extract
    pub reader_version: Version,

    /// general purpose bit flag
    pub flags: u16,

    /// compression method
    pub method: Method,

    /// last mod file datetime
    pub modified: MsdosTimestamp,

    /// crc32 hash
    pub crc32: u32,

    /// compressed size
    pub compressed_size: u32,

    /// uncompressed size
    pub uncompressed_size: u32,

    /// disk number start
    pub disk_nbr_start: u16,

    /// internal file attributes
    pub internal_attrs: u16,

    /// external file attributes
    pub external_attrs: u32,

    /// relative offset of local header
    pub header_offset: u32,

    /// name field
    pub name: Cow<'a, [u8]>,

    /// extra field
    pub extra: Cow<'a, [u8]>,

    /// comment field
    pub comment: Cow<'a, [u8]>,
}

impl<'a> CentralDirectoryFileHeader<'a> {
    const SIGNATURE: &'static str = "PK\x01\x02";

    /// Parser for the central directory file header
    pub fn parser(i: &mut Partial<&'a [u8]>) -> PResult<Self> {
        _ = literal(Self::SIGNATURE).parse_next(i)?;
        let creator_version = Version::parser.parse_next(i)?;
        let reader_version = Version::parser.parse_next(i)?;
        let flags = le_u16.parse_next(i)?;
        let method = Method::parser.parse_next(i)?;
        let modified = MsdosTimestamp::parser.parse_next(i)?;
        let crc32 = le_u32.parse_next(i)?;
        let compressed_size = le_u32.parse_next(i)?;
        let uncompressed_size = le_u32.parse_next(i)?;
        let name_len = le_u16.parse_next(i)?;
        let extra_len = le_u16.parse_next(i)?;
        let comment_len = le_u16.parse_next(i)?;
        let disk_nbr_start = le_u16.parse_next(i)?;
        let internal_attrs = le_u16.parse_next(i)?;
        let external_attrs = le_u32.parse_next(i)?;
        let header_offset = le_u32.parse_next(i)?;

        let name = take(name_len).parse_next(i)?;
        let extra = take(extra_len).parse_next(i)?;
        let comment = take(comment_len).parse_next(i)?;

        Ok(Self {
            creator_version,
            reader_version,
            flags,
            method,
            modified,
            crc32,
            compressed_size,
            uncompressed_size,
            disk_nbr_start,
            internal_attrs,
            external_attrs,
            header_offset,
            name: Cow::Borrowed(name),
            extra: Cow::Borrowed(extra),
            comment: Cow::Borrowed(comment),
        })
    }
}

impl CentralDirectoryFileHeader<'_> {
    /// Returns true if the name or comment is not valid UTF-8
    pub fn is_non_utf8(&self) -> bool {
        let (valid1, require1) = detect_utf8(&self.name[..]);
        let (valid2, require2) = detect_utf8(&self.comment[..]);
        if !valid1 || !valid2 {
            // definitely not utf-8
            return true;
        }

        if !require1 && !require2 {
            // name and comment only use single-byte runes that overlap with UTF-8
            return false;
        }

        // Might be UTF-8, might be some other encoding; preserve existing flag.
        // Some ZIP writers use UTF-8 encoding without setting the UTF-8 flag.
        // Since it is impossible to always distinguish valid UTF-8 from some
        // other encoding (e.g., GBK or Shift-JIS), we trust the flag.
        self.flags & 0x800 == 0
    }

    /// Converts the directory header into a entry: this involves
    /// parsing the extra fields and converting the timestamps.
    pub fn as_entry(&self, encoding: Encoding, global_offset: u64) -> Result<Entry, Error> {
        let mut entry = Entry {
            name: encoding.decode(&self.name[..])?,
            method: self.method,
            comment: encoding.decode(&self.comment[..])?,
            modified: self.modified.to_datetime().unwrap_or_else(zero_datetime),
            created: None,
            accessed: None,
            header_offset: (self.header_offset as u64)
                .checked_add(global_offset)
                .ok_or(FormatError::InvalidHeaderOffset)?,
            reader_version: self.reader_version,
            flags: self.flags,
            uid: None,
            gid: None,
            crc32: self.crc32,
            compressed_size: self.compressed_size as _,
            uncompressed_size: self.uncompressed_size as _,
            mode: Mode(0),
        };

        entry.mode = match self.creator_version.host_system {
            HostSystem::Unix | HostSystem::Osx => UnixMode(self.external_attrs >> 16).into(),
            HostSystem::WindowsNtfs | HostSystem::Vfat | HostSystem::MsDos => {
                MsdosMode(self.external_attrs).into()
            }
            _ => Mode(0),
        };
        if entry.name.ends_with('/') {
            // believe it or not, this is straight from the APPNOTE
            entry.mode |= Mode::DIR
        };

        let settings = ExtraFieldSettings {
            uncompressed_size_u32: self.uncompressed_size,
            compressed_size_u32: self.compressed_size,
            header_offset_u32: self.header_offset,
        };

        let mut slice = Partial::new(&self.extra[..]);
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
