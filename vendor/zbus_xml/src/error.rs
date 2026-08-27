use std::{borrow::Cow, convert::Infallible, error, fmt, io, num::NonZeroUsize, sync::Arc};
use zbus_names::Error as NamesError;
use zvariant::Error as VariantError;

/// The error type for `zbus_xml`.
///
/// The various errors that can be reported by this crate.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Error {
    /// A zvariant error, e.g. an invalid signature.
    Variant(VariantError),
    /// A D-Bus name error.
    Name(NamesError),
    /// An XML parsing error.
    Xml(XmlError),
    /// An I/O error.
    Io(Arc<io::Error>),
    /// An XML error from quick_xml
    #[deprecated(
        since = "5.2.0",
        note = "This variant is no longer returned from any of our API. \
                Match on `Error::Xml` instead."
    )]
    #[allow(deprecated)]
    QuickXml(DeError),
    /// An XML serialization error from quick_xml
    #[deprecated(
        since = "5.2.0",
        note = "This variant is no longer returned from any of our API. \
                Match on `Error::Xml` or `Error::Io` instead."
    )]
    #[allow(deprecated)]
    QuickXmlSer(SeError),
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Variant(s), Self::Variant(o)) => s == o,
            (Self::Name(s), Self::Name(o)) => s == o,
            (Self::Xml(s), Self::Xml(o)) => s == o,
            (_, _) => false,
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::Variant(e) => Some(e),
            Error::Name(e) => Some(e),
            Error::Xml(e) => Some(e),
            Error::Io(e) => Some(e),
            #[allow(deprecated)]
            Error::QuickXml(e) => Some(e),
            #[allow(deprecated)]
            Error::QuickXmlSer(e) => Some(e),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Variant(e) => write!(f, "{e}"),
            Error::Name(e) => write!(f, "{e}"),
            Error::Xml(e) => write!(f, "XML error: {e}"),
            Error::Io(e) => write!(f, "I/O error: {e}"),
            #[allow(deprecated)]
            Error::QuickXml(e) => write!(f, "XML error: {e}"),
            #[allow(deprecated)]
            Error::QuickXmlSer(e) => write!(f, "XML serialization error: {e}"),
        }
    }
}

impl From<VariantError> for Error {
    fn from(val: VariantError) -> Self {
        Error::Variant(val)
    }
}

impl From<NamesError> for Error {
    fn from(val: NamesError) -> Self {
        Error::Name(val)
    }
}

impl From<XmlError> for Error {
    fn from(val: XmlError) -> Self {
        Error::Xml(val)
    }
}

impl From<io::Error> for Error {
    fn from(val: io::Error) -> Self {
        Error::Io(Arc::new(val))
    }
}

impl From<Infallible> for Error {
    fn from(i: Infallible) -> Self {
        match i {}
    }
}

/// An error encountered while parsing an XML document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmlError {
    message: String,
    position: usize,
}

impl XmlError {
    pub(crate) fn new(message: impl Into<String>, position: usize) -> Self {
        Self {
            message: message.into(),
            position,
        }
    }

    /// A message describing the error.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The byte offset in the document at which the error was encountered.
    pub fn position(&self) -> usize {
        self.position
    }
}

impl fmt::Display for XmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (at byte offset {})", self.message, self.position)
    }
}

impl error::Error for XmlError {}

/// A copy of `quick_xml::de::DeError`, kept for backwards compatibility.
///
/// This crate no longer uses quick-xml, so this error is never returned. It only exists so that
/// code matching on [`Error::QuickXml`] keeps compiling. The `InvalidXml` variant carries the
/// error message as a string instead of the quick-xml error type it used to wrap, so `source()`
/// returns `None` for it.
#[doc(hidden)]
#[deprecated(
    since = "5.2.0",
    note = "This error is no longer returned from any of our API. \
            Match on `Error::Xml` instead."
)]
#[derive(Clone, Debug)]
pub enum DeError {
    /// Serde custom error.
    Custom(String),
    /// XML parsing error.
    InvalidXml(String),
    /// `MapAccess::next_value[_seed]` was called before `MapAccess::next_key[_seed]`.
    KeyNotRead,
    /// Deserializer encountered a start tag with an unexpected name.
    UnexpectedStart(Vec<u8>),
    /// The reader produced an EOF event when it wasn't expecting one.
    UnexpectedEof,
    /// Too many events were skipped while deserializing a sequence.
    TooManyEvents(NonZeroUsize),
}

#[allow(deprecated)]
impl fmt::Display for DeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Custom(s) => f.write_str(s),
            Self::InvalidXml(e) => f.write_str(e),
            Self::KeyNotRead => f.write_str(
                "invalid `Deserialize` implementation: `MapAccess::next_value[_seed]` was called \
                 before `MapAccess::next_key[_seed]`",
            ),
            Self::UnexpectedStart(e) => {
                write!(
                    f,
                    "unexpected `Event::Start({})`",
                    String::from_utf8_lossy(e)
                )
            }
            Self::UnexpectedEof => f.write_str("unexpected `Event::Eof`"),
            Self::TooManyEvents(s) => write!(f, "deserializer buffered {s} events, limit exceeded"),
        }
    }
}

#[allow(deprecated)]
impl error::Error for DeError {}

/// A copy of `quick_xml::se::SeError`, kept for backwards compatibility.
///
/// This crate no longer uses quick-xml, so this error is never returned. It only exists so that
/// code matching on [`Error::QuickXmlSer`] keeps compiling.
#[doc(hidden)]
#[deprecated(
    since = "5.2.0",
    note = "This error is no longer returned from any of our API. \
            Match on `Error::Xml` or `Error::Io` instead."
)]
#[derive(Clone, Debug)]
pub enum SeError {
    /// Serde custom error.
    Custom(String),
    /// XML document cannot be written to the underlying source.
    Io(Arc<io::Error>),
    /// Some value could not be formatted.
    Fmt(std::fmt::Error),
    /// Serialized type cannot be represented in XML.
    Unsupported(Cow<'static, str>),
    /// Some value could not be turned to UTF-8.
    NonEncodable(std::str::Utf8Error),
}

#[allow(deprecated)]
impl fmt::Display for SeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Custom(s) => f.write_str(s),
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Fmt(e) => write!(f, "formatting error: {e}"),
            Self::Unsupported(s) => write!(f, "unsupported value: {s}"),
            Self::NonEncodable(e) => write!(f, "malformed UTF-8: {e}"),
        }
    }
}

#[allow(deprecated)]
impl error::Error for SeError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

/// Alias for a `Result` with the error type `zbus_xml::Error`.
pub type Result<T> = std::result::Result<T, Error>;
