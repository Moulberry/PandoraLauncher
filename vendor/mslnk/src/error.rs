use std::error::Error;
use std::fmt;
use std::io;
use std::num;
#[derive(Debug)]
pub struct MSLinkError {
    kind: String,
    message: String,
}
/// fix :doesn't satisfy `MSLinkError: std::error::Error`
impl Error for MSLinkError {}
impl fmt::Display for MSLinkError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.kind, self.message)
    }
}

// Implement std::convert::From for AppError; from io::Error
impl From<io::Error> for MSLinkError {
    fn from(error: io::Error) -> Self {
        MSLinkError {
            kind: String::from("io"),
            message: error.to_string(),
        }
    }
}

// Implement std::convert::From for AppError; from num::ParseIntError
impl From<num::ParseIntError> for MSLinkError {
    fn from(error: num::ParseIntError) -> Self {
        MSLinkError {
            kind: String::from("parse"),
            message: error.to_string(),
        }
    }
}
