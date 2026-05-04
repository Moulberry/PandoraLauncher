#![doc = include_str!("lib.md")]

pub mod config;
pub mod de;
mod error;
pub mod ser;
#[cfg(test)]
mod test;

pub use crate::config::SerdeXml;
pub use crate::de::{from_reader, from_str, Deserializer};
pub use crate::error::Error;
pub use crate::ser::{to_string, to_writer, Serializer};

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
