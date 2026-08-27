//! Idiomatic inotify wrapper for the Rust programming language
//!
//! # About
//!
//! [inotify-rs] is an idiomatic wrapper around the Linux kernel's [inotify] API
//! for the Rust programming language. It can be used for monitoring changes to
//! files or directories.
//!
//! The [`Inotify`] struct is the main entry point into the API.
//! The [`EventStream`] struct is designed to be used with async streams.
//!
//! # Examples
//!
//! If you just want to synchronously retrieve events
//! ```
//! use inotify::{
//!     Inotify,
//!     WatchMask,
//! };
//!
//! let mut inotify = Inotify::init()
//!     .expect("Error while initializing inotify instance");
//!
//! # // Create a temporary file, so `Watches::add` won't return an error.
//! # use std::fs::File;
//! # let mut test_file = File::create("/tmp/inotify-rs-test-file")
//! #     .expect("Failed to create test file");
//! #
//! // Watch for modify and close events.
//! inotify
//!     .watches()
//!     .add(
//!         "/tmp/inotify-rs-test-file",
//!         WatchMask::MODIFY | WatchMask::CLOSE,
//!     )
//!     .expect("Failed to add file watch");
//!
//! # // Modify file, so the following `read_events_blocking` won't block.
//! # use std::io::Write;
//! # write!(&mut test_file, "something\n")
//! #     .expect("Failed to write something to test file");
//! #
//! // Read events that were added with `Watches::add` above.
//! let mut buffer = [0; 1024];
//! let events = inotify.read_events_blocking(&mut buffer)
//!     .expect("Error while reading events");
//!
//! for event in events {
//!     // Handle event
//! }
//! ```
//! When you want to read events asynchronously, you need to convert it to [`EventStream`].
//! The transform function is [`Inotify::into_event_stream`]
//! ```
//! # async fn stream_events() {
//! # use inotify::StreamExt;
//! #
//! # let mut inotify = inotify::Inotify::init()
//! #     .expect("Error while initializing inotify instance");
//! #
//! let mut buffer = [0; 1024];
//! let mut stream = inotify.into_event_stream(&mut buffer)
//!     .expect("Error converting to stream");
//!
//! // Read events from async stream
//! while let Some(event_or_error) = stream.next().await {
//!     println!("event: {:?}", event_or_error.expect("Stream error"));
//! }
//! # }
//! ```
//! # Attention: inotify gotchas
//!
//! inotify (as in, the Linux API, not this wrapper) has many edge cases, making
//! it hard to use correctly. This can lead to weird and hard to find bugs in
//! applications that are based on it. inotify-rs does its best to fix these
//! issues, but sometimes this would require an amount of runtime overhead that
//! is just unacceptable for a low-level wrapper such as this.
//!
//! We've documented any issues that inotify-rs has inherited from inotify, as
//! far as we are aware of them. Please watch out for any further warnings
//! throughout this documentation. If you want to be on the safe side, in case
//! we have missed something, please read the [inotify man pages] carefully.
//!
//! [inotify-rs]: https://crates.io/crates/inotify
//! [inotify]: https://en.wikipedia.org/wiki/Inotify
//! [inotify man pages]: https://man7.org/linux/man-pages/man7/inotify.7.html
//!
//! # `Vec` as buffers
//!
//! Using a `Vec::with_capacity(4096)` as a buffer is problematic, since this buffer has reserved
//! capacity but length `0`. Use `vec![0u8; 4096]` instead.

#![deny(missing_docs)]
#![deny(warnings)]
#![deny(missing_debug_implementations)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[macro_use]
extern crate bitflags;

mod events;
mod fd_guard;
mod inotify;
mod util;
mod watches;

#[cfg(feature = "stream")]
mod stream;

pub use crate::events::{
    Event, EventAuxiliaryFlags, EventKind, EventMask, EventMaskParseError, EventOwned, Events,
    ParsedEventMask,
};
pub use crate::inotify::Inotify;
pub use crate::util::{get_absolute_path_buffer_size, get_buffer_size};
pub use crate::watches::{WatchDescriptor, WatchMask, Watches};

#[cfg(feature = "stream")]
#[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
pub use self::stream::EventStream;
#[cfg(feature = "stream")]
#[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
pub use futures_util::{Stream, StreamExt};
