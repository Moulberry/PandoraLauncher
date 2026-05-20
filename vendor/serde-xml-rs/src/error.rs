pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unsupported operation {0}")]
    Unsupported(&'static str),
    #[error("Expected {expected} but got {but_got}")]
    Unexpected {
        expected: &'static str,
        but_got: String,
    },
    #[error(
        "In '{element_name}', attribute '{attribute_name}' comes after at least one element. All attributes must come before any elements."
    )]
    AttributesMustComeBeforeElements {
        element_name: String,
        attribute_name: &'static str,
    },
    #[error("Custom: {0}")]
    Custom(String),
    #[error("Reader: {0}")]
    Reader(#[from] xml::reader::Error),
    #[error("Writer: {0}")]
    Writer(#[from] xml::writer::Error),
    #[error("UTF-8: {0}")]
    FromUtf8(#[from] std::string::FromUtf8Error),
    #[error("Parse: {0}")]
    ParseBool(#[from] std::str::ParseBoolError),
    #[error("Parse int: {0}")]
    ParseInt(#[from] std::num::ParseIntError),
    #[error("Parse float: {0}")]
    ParseFloat(#[from] std::num::ParseFloatError),
}

impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Self::Custom(msg.to_string())
    }
}

impl serde::ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Self::Custom(msg.to_string())
    }
}
