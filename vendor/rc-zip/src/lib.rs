#![warn(missing_docs)]

//! rc-zip is a [sans-io](https://sans-io.readthedocs.io/how-to-sans-io.html) library for reading zip files.
//!
//! It's made up of a bunch of types representing the various parts of a zip
//! file, winnow parsers that can turn byte buffers into those types, and
//! state machines that can use those parsers to read zip files from a stream.
//!
//! This crate is low-level, you may be interested in either of those higher
//! level wrappers:
//!
//!   * [rc-zip-sync](https://crates.io/crates/rc-zip-sync) for using std I/O traits
//!   * [rc-zip-tokio](https://crates.io/crates/rc-zip-tokio) for using tokio I/O traits

pub mod encoding;
pub mod error;
pub mod fsm;
pub mod parse;

#[cfg(feature = "corpus")]
#[deprecated(since = "5.3.7", note = "Please use the `rc-zip-corpus` crate instead")]
pub mod corpus;

// re-exports
pub use error::{Error, Result};
pub use parse::{Archive, Entry, EntryKind};

// dependencies re-exports
pub use chrono;
