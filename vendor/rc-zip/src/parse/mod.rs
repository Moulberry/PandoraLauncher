//! Parsers and types for the various elements that make up a ZIP file.
//!
//! Contain winnow parsers for most elements that make up a ZIP file, like the
//! end-of-central-directory record, local file headers, and central directory
//! headers.
//!
//! All parsers here are based off of the PKWARE appnote.txt, which you can find
//! in the source repository.

mod archive;
pub use archive::*;

mod extra_field;
pub use extra_field::*;

mod mode;
pub use mode::*;

mod version;
pub use version::*;

mod date_time;
pub use date_time::*;

mod central_directory_file_header;
pub use central_directory_file_header::*;

mod eocd;
pub use eocd::*;

mod local_headers;
pub use local_headers::*;
