#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/fern/0.7.1")]
//! Efficient, configurable logging in Rust.
//!
//! # fern 0.4.4, 0.5.\*, 0.6.\* security warning - `colored` feature + global allocator
//!
//! One of our downstream dependencies, [atty](https://docs.rs/atty/), through
//! [colored](https://docs.rs/colored/), has an unsoundness issue:
//! <https://rustsec.org/advisories/RUSTSEC-2021-0145.html>.
//!
//! This shows up in one situation: if you're using `colored` 0.1.0 (the crate, or our
//! feature), and a custom global allocator.
//!
//! Upgrade to `fern` 0.7.0, and `colored` 0.2.0 if you depend on it directly, to fix this issue.
//!
//! # Depending on fern
//!
//! Ensure you require both fern and log in your project's `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! log = "0.4"
//! fern = "0.7"
//! ```
//!
//! # Example setup
//!
//! With fern, all logger configuration is done via builder-like methods on
//! instances of the [`Dispatch`] structure.
//!
//! Here's an example logger which formats messages, and sends everything Debug
//! and above to both stdout and an output.log file:
//!
//! ```no_run
//! use log::{debug, error, info, trace, warn};
//! use std::time::SystemTime;
//!
//! fn setup_logger() -> Result<(), fern::InitError> {
//!     fern::Dispatch::new()
//!         .format(|out, message, record| {
//!             out.finish(format_args!(
//!                 "[{} {} {}] {}",
//!                 humantime::format_rfc3339_seconds(SystemTime::now()),
//!                 record.level(),
//!                 record.target(),
//!                 message
//!             ))
//!         })
//!         .level(log::LevelFilter::Debug)
//!         .chain(std::io::stdout())
//!         .chain(fern::log_file("output.log")?)
//!         .apply()?;
//!     Ok(())
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     setup_logger()?;
//!
//!     info!("Hello, world!");
//!     warn!("Warning!");
//!     debug!("Now exiting.");
//!
//!     Ok(())
//! }
//! ```
//!
//! Let's unwrap this:
//!
//!
//! ```
//! fern::Dispatch::new()
//! # ;
//! ```
//!
//! [`Dispatch::new`] creates an empty configuration.
//!
//! ```
//! # fern::Dispatch::new()
//! .format(|out, message, record| {
//!     out.finish(format_args!(
//!         "..."
//!     ))
//! })
//! # ;
//! ```
//!
//! This incantation sets the `Dispatch` format! The closure taking in
//! `out, message, record` will be called once for each message going through
//! the dispatch, and the formatted log message will be used for any downstream
//! consumers.
//!
//! Do any work you want in this closure, and then call `out.finish` at the end.
//! The callback-style result passing with `out.finish(format_args!())` lets us
//! format without any intermediate string allocation.
//!
//! [`format_args!`] has the same format as [`println!`], just returning a
//! not-yet-written result we can use internally.
//!
//! ```
//! std::time::SystemTime::now()
//! # ;
//! ```
//!
//! [`std::time::SystemTime::now`] retrieves the current time.
//!
//! ```
//! humantime::format_rfc3339_seconds(
//!     // ...
//!     # std::time::SystemTime::now()
//! )
//! # ;
//! ```
//!
//! [`humantime::format_rfc3339_seconds`] formats the current time into an
//! RFC3339 timestamp, with second-precision.
//!
//! RFC3339 looks like `2018-02-14T00:28:07Z`, always using UTC, ignoring system
//! timezone.
//!
//! `humantime` is a nice light dependency, but only offers this one format.
//! For more custom time formatting, I recommend
//! [`jiff`](https://docs.rs/jiff/).
//!
//! Now, back to the [`Dispatch`] methods:
//!
//! ```
//! # fern::Dispatch::new()
//! .level(log::LevelFilter::Debug)
//! # ;
//! ```
//!
//! Sets the minimum logging level for all modules, if not overwritten with
//! [`Dispatch::level_for`], to [`Level::Debug`][log::Level::Debug].
//!
//! ```
//! # fern::Dispatch::new()
//! .chain(std::io::stdout())
//! # ;
//! ```
//!
//! Adds a child to the logger. With this, all messages which pass the filters
//! will be sent to stdout.
//!
//! [`Dispatch::chain`] accepts [`Stdout`], [`Stderr`], [`File`] and other
//! [`Dispatch`] instances.
//!
//! ```
//! # fern::Dispatch::new()
//! .chain(fern::log_file("output.log")?)
//! # ; <Result<(), Box<dyn std::error::Error>>>::Ok(())
//! ```
//!
//! Adds a second child sending messages to the file "output.log".
//!
//! See [`log_file`].
//!
//! ```
//! # fern::Dispatch::new()
//! // ...
//! .apply()
//! # ;
//! ```
//!
//! Consumes the configuration and instantiates it as the current runtime global
//! logger.
//!
//! This will fail if and only if `.apply()` or equivalent form another crate
//! has already been used this runtime.
//!
//! Since the binary crate is the only one ever setting up logging, and it's
//! usually done near the start of `main`, the [`Dispatch::apply`] result can be
//! reasonably unwrapped: it's a bug if any crate is calling this method more
//! than once.
//!
//! ---
//!
//! The final output will look like:
//!
//! ```text
//! [2023-03-18T20:12:50Z INFO cmd_program] Hello, world!
//! [2023-03-18T20:12:50Z WARN cmd_program] Warning!
//! [2023-03-18T20:12:50Z DEBUG cmd_program] Now exiting.
//! ```
//!
//! # Logging
//!
//! Once the logger has been set, it will pick up all logging calls from your
//! crate and all libraries you depend on.
//!
//! ```rust
//! # use log::{debug, error, info, trace, warn};
//!
//! # fn setup_logger() -> Result<(), fern::InitError> {
//! fern::Dispatch::new()
//!     // ...
//!     .apply()?;
//! # Ok(())
//! # }
//!
//! # fn main() {
//! # setup_logger().ok(); // we're ok with this not succeeding.
//! trace!("Trace message");
//! debug!("Debug message");
//! info!("Info message");
//! warn!("Warning message");
//! error!("Error message");
//! # }
//! ```
//!
//! # More
//!
//! The [`Dispatch`] documentation has example usages of each method, and the
//! [full example program] might be useful for using fern in a larger
//! application context.
//!
//! See the [colors] module for examples using ANSI terminal coloring.
//!
//! See the [syslog] module for examples outputting to the unix syslog, or the
//! [syslog full example program] for a more realistic sample.
//!
//! See the [meta] module for information on getting logging-within-logging
//! working correctly.
//!
//! [`Stdout`]: std::io::Stdout
//! [`Stderr`]: std::io::Stderr
//! [`File`]: std::fs::File
//! [full example program]: https://github.com/daboross/fern/tree/fern-0.7.0/examples/cmd-program.rs
//! [syslog full example program]: https://github.com/daboross/fern/tree/fern-0.7.0/examples/syslog.rs
//! [`humantime::format_rfc3339_seconds`]: https://docs.rs/humantime/2/humantime/fn.format_rfc3339_seconds.html
use std::{
    convert::AsRef,
    fmt,
    fs::{File, OpenOptions},
    io,
    path::Path,
};

#[cfg(all(not(windows), any(feature = "syslog-4", feature = "syslog-6")))]
use std::collections::HashMap;

#[cfg(all(not(windows), feature = "syslog-7"))]
use std::collections::BTreeMap;

pub use crate::{
    builders::{Dispatch, Output, Panic},
    errors::InitError,
    log_impl::FormatCallback,
};

mod builders;
mod errors;
mod log_impl;

#[cfg(feature = "colored")]
pub mod colors;
#[cfg(all(
    feature = "syslog-3",
    feature = "syslog-4",
    // disable on windows when running doctests, as the code itself only runs on
    // linux. enable on windows otherwise because it's a documentation-only
    // module.
    any(not(windows), not(doctest))
))]
pub mod syslog;

pub mod meta;

/// A type alias for a log formatter.
///
/// As of fern `0.5`, the passed `fmt::Arguments` will always be the same as
/// the given `log::Record`'s `.args()`.
pub type Formatter = dyn Fn(FormatCallback, &fmt::Arguments, &log::Record) + Sync + Send + 'static;

/// A type alias for a log filter. Returning true means the record should
/// succeed - false means it should fail.
pub type Filter = dyn Fn(&log::Metadata) -> bool + Send + Sync + 'static;

#[cfg(feature = "date-based")]
pub use crate::builders::DateBased;

#[cfg(all(not(windows), feature = "syslog-4"))]
type Syslog4Rfc3164Logger = syslog4::Logger<syslog4::LoggerBackend, String, syslog4::Formatter3164>;

#[cfg(all(not(windows), feature = "syslog-4"))]
type Syslog4Rfc5424Logger = syslog4::Logger<
    syslog4::LoggerBackend,
    (i32, HashMap<String, HashMap<String, String>>, String),
    syslog4::Formatter5424,
>;

#[cfg(all(not(windows), feature = "syslog-6"))]
type Syslog6Rfc3164Logger = syslog6::Logger<syslog6::LoggerBackend, syslog6::Formatter3164>;

#[cfg(all(not(windows), feature = "syslog-6"))]
type Syslog6Rfc5424Logger = syslog6::Logger<syslog6::LoggerBackend, syslog6::Formatter5424>;

#[cfg(all(not(windows), feature = "syslog-7"))]
type Syslog7Rfc3164Logger = syslog7::Logger<syslog7::LoggerBackend, syslog7::Formatter3164>;

#[cfg(all(not(windows), feature = "syslog-7"))]
type Syslog7Rfc5424Logger = syslog7::Logger<syslog7::LoggerBackend, syslog7::Formatter5424>;

#[cfg(all(not(windows), feature = "syslog-4"))]
type Syslog4TransformFn =
    dyn Fn(&log::Record) -> (i32, HashMap<String, HashMap<String, String>>, String) + Send + Sync;

#[cfg(all(not(windows), feature = "syslog-6"))]
type Syslog6TransformFn =
    dyn Fn(&log::Record) -> (u32, HashMap<String, HashMap<String, String>>, String) + Send + Sync;

#[cfg(all(not(windows), feature = "syslog-7"))]
type Syslog7TransformFn =
    dyn Fn(&log::Record) -> (u32, BTreeMap<String, BTreeMap<String, String>>, String) + Send + Sync;

/// Convenience method for opening a log file with common options.
///
/// Equivalent to:
///
/// ```no_run
/// std::fs::OpenOptions::new()
///     .write(true)
///     .create(true)
///     .append(true)
///     .open("filename")
/// # ;
/// ```
///
/// See [`OpenOptions`] for more information.
///
/// [`OpenOptions`]: https://doc.rust-lang.org/std/fs/struct.OpenOptions.html
#[inline]
pub fn log_file<P: AsRef<Path>>(path: P) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

/// Convenience method for opening a re-openable log file with common options.
///
/// The file opening is equivalent to:
///
/// ```no_run
/// std::fs::OpenOptions::new()
///     .write(true)
///     .create(true)
///     .append(true)
///     .open("filename")
/// # ;
/// ```
///
/// See [`OpenOptions`] for more information.
///
/// [`OpenOptions`]: https://doc.rust-lang.org/std/fs/struct.OpenOptions.html
///
/// This function is not available on Windows, and it requires the `reopen-03`
/// feature to be enabled.
#[cfg(all(not(windows), feature = "reopen-03"))]
#[inline]
pub fn log_reopen(path: &Path, signal: Option<libc::c_int>) -> io::Result<reopen03::Reopen<File>> {
    let p = path.to_owned();
    let r = reopen03::Reopen::new(Box::new(move || log_file(&p)))?;

    if let Some(s) = signal {
        r.handle().register_signal(s)?;
    }
    Ok(r)
}

/// Convenience method for opening a re-openable log file with common options.
///
/// The file opening is equivalent to:
///
/// ```no_run
/// std::fs::OpenOptions::new()
///     .write(true)
///     .create(true)
///     .append(true)
///     .open("filename")
/// # ;
/// ```
///
/// See [`OpenOptions`] for more information.
///
/// [`OpenOptions`]: https://doc.rust-lang.org/std/fs/struct.OpenOptions.html
///
/// This function requires the `reopen-1` feature to be enabled.
#[cfg(all(not(windows), feature = "reopen-1"))]
#[inline]
pub fn log_reopen1<S: IntoIterator<Item = libc::c_int>>(
    path: &Path,
    signals: S,
) -> io::Result<reopen1::Reopen<File>> {
    let p = path.to_owned();
    let r = reopen1::Reopen::new(Box::new(move || log_file(&p)))?;

    for s in signals {
        r.handle().register_signal(s)?;
    }
    Ok(r)
}
