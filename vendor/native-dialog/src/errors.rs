use std::ffi::OsString;

use thiserror::Error;

pub type Result<T = ()> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("system error or I/O failure")]
    Io(#[from] std::io::Error),

    #[error("invalid utf-8 string")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("cannot find implementation (kdialog/zenity/yad)")]
    MissingDep,

    #[error("subprocess killed by signal: {0:?}")]
    Killed(OsString),

    #[error("other errors reported by implementation: {0}")]
    Other(String),
}
