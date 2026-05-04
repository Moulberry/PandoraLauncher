//! All error types used in this crate

use std::{error, fmt, io};

use crate::parse::Method;

use super::encoding;

/// An alias for `Result<T, rc_zip::Error>`
pub type Result<T> = std::result::Result<T, Error>;

/// Any zip-related error, from invalid archives to encoding problems.
#[derive(Debug)]
pub enum Error {
    /// Not a valid zip file, or a variant that is unsupported.
    Format(FormatError),

    /// Something is not supported by this crate
    Unsupported(UnsupportedError),

    /// Invalid UTF-8, Shift-JIS, or any problem encountered while decoding text in general.
    Encoding(encoding::DecodingError),

    /// I/O-related error
    IO(io::Error),

    /// Decompression-related error
    Decompression {
        /// The compression method that failed
        method: Method,
        /// Additional information
        msg: String,
    },

    /// Could not read as a zip because size could not be determined
    UnknownSize,
}

impl Error {
    /// Create a new error indicating that the given method is not supported.
    pub fn method_not_supported(method: Method) -> Self {
        Self::Unsupported(UnsupportedError::MethodNotSupported(method))
    }

    /// Create a new error indicating that the given method is not enabled.
    pub fn method_not_enabled(method: Method) -> Self {
        Self::Unsupported(UnsupportedError::MethodNotEnabled(method))
    }
}

impl From<FormatError> for Error {
    fn from(fmt_err: FormatError) -> Self {
        Self::Format(fmt_err)
    }
}

impl From<UnsupportedError> for Error {
    fn from(unsupported: UnsupportedError) -> Self {
        Self::Unsupported(unsupported)
    }
}

impl From<encoding::DecodingError> for Error {
    fn from(enc: encoding::DecodingError) -> Self {
        Self::Encoding(enc)
    }
}

impl From<io::Error> for Error {
    fn from(io: io::Error) -> Self {
        Self::IO(io)
    }
}

impl From<Error> for io::Error {
    fn from(e: Error) -> Self {
        match e {
            Error::IO(e) => e,
            e => io::Error::other(e),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format(format) => write!(f, "format: {format}"),
            Self::Unsupported(unsupported) => write!(f, "unsupported: {unsupported}"),
            Self::Encoding(enc) => write!(f, "encoding: {enc:?}"),
            Self::IO(io) => write!(f, "io: {io}"),
            Self::Decompression { method, msg } => {
                write!(f, "{method:?} decompression error: {msg}")
            }
            Self::UnknownSize => f.write_str("size must be known to open zip file"),
        }
    }
}

impl error::Error for Error {}

/// Some part of the zip format is not supported by this crate.
#[derive(Debug)]
pub enum UnsupportedError {
    /// The compression method is not supported.
    MethodNotSupported(Method),

    /// The compression method is supported, but not enabled in this build.
    MethodNotEnabled(Method),

    /// The zip file uses a version of LZMA that is not supported.
    LzmaVersionUnsupported {
        /// major version read from LZMA properties header, cf. appnote 5.8.8
        major: u8,
        /// minor version read from LZMA properties header, cf. appnote 5.8.8
        minor: u8,
    },

    /// The LZMA properties header is not the expected size.
    LzmaPropertiesHeaderWrongSize {
        /// expected size in bytes
        expected: u16,
        /// actual size in bytes, read from a u16, cf. appnote 5.8.8
        actual: u16,
    },
}

impl fmt::Display for UnsupportedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MethodNotSupported(m) => write!(f, "compression method not supported: {m:?}"),
            Self::MethodNotEnabled(m) => write!(
                f,
                "compression method supported, but not enabled in this build: {m:?}"
            ),
            Self::LzmaVersionUnsupported { major, minor } => {
                write!(f, "only LZMA2.0 is supported, found LZMA{major}.{minor}")
            }
            Self::LzmaPropertiesHeaderWrongSize { expected, actual } => {
                write!(f, "LZMA properties header wrong size: expected {expected} bytes, got {actual} bytes")
            }
        }
    }
}

impl error::Error for UnsupportedError {}

/// Specific zip format errors, mostly due to invalid zip archives but that could also stem from
/// implementation shortcomings.
#[derive(Debug)]
pub enum FormatError {
    /// The end of central directory record was not found.
    ///
    /// This usually indicates that the file being read is not a zip archive.
    DirectoryEndSignatureNotFound,

    /// The zip64 end of central directory record could not be parsed.
    ///
    /// This is only returned when a zip64 end of central directory *locator* was found,
    /// so the archive should be zip64, but isn't.
    Directory64EndRecordInvalid,

    /// Corrupted/partial zip file: the offset we found for the central directory
    /// points outside of the current file.
    DirectoryOffsetPointsOutsideFile,

    /// The central record is corrupted somewhat.
    ///
    /// This can happen when the end of central directory record advertises
    /// a certain number of files, but we weren't able to read the same number of central directory
    /// headers.
    InvalidCentralRecord {
        /// expected number of files
        expected: u16,
        /// actual number of files
        actual: u16,
    },

    /// An extra field (that we support) was not decoded correctly.
    ///
    /// This can indicate an invalid zip archive, or an implementation error in this crate.
    InvalidExtraField,

    /// The header offset of an entry is invalid.
    ///
    /// This can indicate an invalid zip archive, or an invalid user-provided global offset
    InvalidHeaderOffset,

    /// End of central directory record claims an impossible number of files.
    ///
    /// Each entry takes a minimum amount of size, so if the overall archive size is smaller than
    /// claimed_records_count * minimum_entry_size, we know it's not a valid zip file.
    ImpossibleNumberOfFiles {
        /// number of files claimed in the end of central directory record
        claimed_records_count: u64,
        /// total size of the zip file
        zip_size: u64,
    },

    /// The local file header (before the file data) could not be parsed correctly.
    InvalidLocalHeader,

    /// The data descriptor (after the file data) could not be parsed correctly.
    InvalidDataDescriptor,

    /// The uncompressed size didn't match
    WrongSize {
        /// expected size in bytes (from the local header, data descriptor, etc.)
        expected: u64,
        /// actual size in bytes (from decompressing the entry)
        actual: u64,
    },

    /// The CRC-32 checksum didn't match.
    WrongChecksum {
        /// expected checksum (from the data descriptor, etc.)
        expected: u32,
        /// actual checksum (from decompressing the entry)
        actual: u32,
    },
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectoryEndSignatureNotFound => {
                f.write_str("end of central directory record not found")
            }
            Self::Directory64EndRecordInvalid => {
                f.write_str("zip64 end of central directory record not found")
            }
            Self::DirectoryOffsetPointsOutsideFile => {
                f.write_str("directory offset points outside of file")
            }
            Self::InvalidCentralRecord { expected, actual } => {
                write!(
                    f,
                    "invalid central record: expected to read {expected} files, got {actual}"
                )
            }
            Self::InvalidExtraField => f.write_str("could not decode extra field"),
            Self::InvalidHeaderOffset => f.write_str("invalid header offset"),
            Self::ImpossibleNumberOfFiles {
                claimed_records_count,
                zip_size,
            } => {
                write!(
                    f,
                    "impossible number of files: claims to have {claimed_records_count}, but zip size is {zip_size}"
                )
            }
            Self::InvalidLocalHeader => f.write_str("invalid local file header"),
            Self::InvalidDataDescriptor => f.write_str("invalid data descriptor"),
            Self::WrongSize { expected, actual } => {
                write!(
                    f,
                    "uncompressed size didn't match: expected {expected}, got {actual}"
                )
            }
            Self::WrongChecksum { expected, actual } => {
                write!(
                    f,
                    "checksum didn't match: expected {expected:x?}, got {actual:x?}"
                )
            }
        }
    }
}

impl error::Error for FormatError {}
