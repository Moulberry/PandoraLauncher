//! which
//!
//! A Rust equivalent of Unix command `which(1)`.
//! # Example:
//!
//! To find which rustc executable binary is using:
//!
//! ```no_run
//! # #[cfg(feature = "real-sys")]
//! # {
//! use which::which;
//! use std::path::PathBuf;
//!
//! let result = which("rustc").unwrap();
//! assert_eq!(result, PathBuf::from("/usr/bin/rustc"));
//! # }
//! ```

mod checker;
mod error;
mod finder;
mod helper;
pub mod sys;
#[cfg(all(windows, feature = "real-sys"))]
mod win_ffi;

use std::fmt;
use std::path;

use std::ffi::{OsStr, OsString};

pub use crate::error::*;
use crate::finder::Finder;
use crate::sys::Sys;

/// Find an executable binary's path by name.
///
/// If given an absolute path, returns it if the file exists and is executable.
///
/// If given a relative path, returns an absolute path to the file if
/// it exists and is executable.
///
/// If given a string without path separators, looks for a file named
/// `binary_name` at each directory in `$PATH` and if it finds an executable
/// file there, returns it.
///
/// # Example
///
/// ```no_run
/// use which::which;
/// use std::path::PathBuf;
///
/// let result = which::which("rustc").unwrap();
/// assert_eq!(result, PathBuf::from("/usr/bin/rustc"));
///
/// ```
#[cfg(feature = "real-sys")]
pub fn which<T: AsRef<OsStr>>(binary_name: T) -> Result<path::PathBuf> {
    which_all(binary_name).and_then(|mut i| i.next().ok_or(Error::CannotFindBinaryPath))
}

/// Find an executable binary's path by name, ignoring `cwd`.
///
/// If given an absolute path, returns it if the file exists and is executable.
///
/// Does not resolve relative paths.
///
/// If given a string without path separators, looks for a file named
/// `binary_name` at each directory in `$PATH` and if it finds an executable
/// file there, returns it.
///
/// # Example
///
/// ```no_run
/// use which::which;
/// use std::path::PathBuf;
///
/// let result = which::which_global("rustc").unwrap();
/// assert_eq!(result, PathBuf::from("/usr/bin/rustc"));
///
/// ```
#[cfg(feature = "real-sys")]
pub fn which_global<T: AsRef<OsStr>>(binary_name: T) -> Result<path::PathBuf> {
    which_all_global(binary_name).and_then(|mut i| i.next().ok_or(Error::CannotFindBinaryPath))
}

/// Find all binaries with `binary_name` using `cwd` to resolve relative paths.
#[cfg(feature = "real-sys")]
pub fn which_all<T: AsRef<OsStr>>(binary_name: T) -> Result<impl Iterator<Item = path::PathBuf>> {
    let cwd = sys::RealSys.current_dir().ok();

    Finder::new(&sys::RealSys).find(binary_name, sys::RealSys.env_path(), cwd, Noop)
}

/// Find all binaries with `binary_name` ignoring `cwd`.
#[cfg(feature = "real-sys")]
pub fn which_all_global<T: AsRef<OsStr>>(
    binary_name: T,
) -> Result<impl Iterator<Item = path::PathBuf>> {
    Finder::new(&sys::RealSys).find(
        binary_name,
        sys::RealSys.env_path(),
        Option::<&Path>::None,
        Noop,
    )
}

/// Find all binaries matching a regular expression in a the system PATH.
///
/// Only available when feature `regex` is enabled.
///
/// # Arguments
///
/// * `regex` - A regular expression to match binaries with
///
/// # Examples
///
/// Find Python executables:
///
/// ```no_run
/// use regex::Regex;
/// use which::which;
/// use std::path::PathBuf;
///
/// let re = Regex::new(r"python\d$").unwrap();
/// let binaries: Vec<PathBuf> = which::which_re(re).unwrap().collect();
/// let python_paths = vec![PathBuf::from("/usr/bin/python2"), PathBuf::from("/usr/bin/python3")];
/// assert_eq!(binaries, python_paths);
/// ```
///
/// Find all cargo subcommand executables on the path:
///
/// ```
/// use which::which_re;
/// use regex::Regex;
///
/// which_re(Regex::new("^cargo-.*").unwrap()).unwrap()
///     .for_each(|pth| println!("{}", pth.to_string_lossy()));
/// ```
#[cfg(all(feature = "regex", feature = "real-sys"))]
pub fn which_re(
    regex: impl std::borrow::Borrow<Regex>,
) -> Result<impl Iterator<Item = path::PathBuf>> {
    which_re_in(regex, sys::RealSys.env_path())
}

/// Find `binary_name` in the path list `paths`, using `cwd` to resolve relative paths.
#[cfg(feature = "real-sys")]
pub fn which_in<T, U, V>(binary_name: T, paths: Option<U>, cwd: V) -> Result<path::PathBuf>
where
    T: AsRef<OsStr>,
    U: AsRef<OsStr>,
    V: AsRef<path::Path>,
{
    which_in_all(binary_name, paths, cwd)
        .and_then(|mut i| i.next().ok_or(Error::CannotFindBinaryPath))
}

/// Find all binaries matching a regular expression in a list of paths.
///
/// Only available when feature `regex` is enabled.
///
/// # Arguments
///
/// * `regex` - A regular expression to match binaries with
/// * `paths` - A string containing the paths to search
///   (separated in the same way as the PATH environment variable)
///
/// # Examples
///
/// ```no_run
/// use regex::Regex;
/// use which::which;
/// use std::path::PathBuf;
///
/// let re = Regex::new(r"python\d$").unwrap();
/// let paths = Some("/usr/bin:/usr/local/bin");
/// let binaries: Vec<PathBuf> = which::which_re_in(re, paths).unwrap().collect();
/// let python_paths = vec![PathBuf::from("/usr/bin/python2"), PathBuf::from("/usr/bin/python3")];
/// assert_eq!(binaries, python_paths);
/// ```
#[cfg(all(feature = "regex", feature = "real-sys"))]
pub fn which_re_in<T>(
    regex: impl std::borrow::Borrow<Regex>,
    paths: Option<T>,
) -> Result<impl Iterator<Item = path::PathBuf>>
where
    T: AsRef<OsStr>,
{
    Finder::new(&sys::RealSys).find_re(regex, paths, Noop)
}

/// Find all binaries with `binary_name` in the path list `paths`, using `cwd` to resolve relative paths.
#[cfg(feature = "real-sys")]
pub fn which_in_all<'a, T, U, V>(
    binary_name: T,
    paths: Option<U>,
    cwd: V,
) -> Result<impl Iterator<Item = path::PathBuf> + 'a>
where
    T: AsRef<OsStr>,
    U: AsRef<OsStr>,
    V: AsRef<path::Path> + 'a,
{
    Finder::new(&sys::RealSys).find(binary_name, paths, Some(cwd), Noop)
}

/// Find all binaries with `binary_name` in the path list `paths`, ignoring `cwd`.
#[cfg(feature = "real-sys")]
pub fn which_in_global<T, U>(
    binary_name: T,
    paths: Option<U>,
) -> Result<impl Iterator<Item = path::PathBuf>>
where
    T: AsRef<OsStr>,
    U: AsRef<OsStr>,
{
    Finder::new(&sys::RealSys).find(binary_name, paths, Option::<&Path>::None, Noop)
}

/// A wrapper containing all functionality in this crate.
pub struct WhichConfig<TSys: sys::Sys, F = Noop> {
    cwd: CwdOption,
    custom_path_list: Option<OsString>,
    binary_name: Option<OsString>,
    nonfatal_error_handler: F,
    #[cfg(feature = "regex")]
    regex: Option<Regex>,
    sys: TSys,
}

enum CwdOption {
    Unspecified,
    UseSysCwd,
    RefuseCwd,
    UseCustomCwd(path::PathBuf),
}

/// A handler for non-fatal errors which does nothing with them.
#[derive(Default, Debug, Clone)]
pub struct Noop;

/// Defines what should happen when a nonfatal error is encountered. A nonfatal error may represent a problem,
/// but it doesn't necessarily require `which` to stop its search.
///
/// This trait is implemented for any closure or function that takes a single argument which is a [`NonFatalError`].
/// You may also implement it for your own types.
pub trait NonFatalErrorHandler {
    fn handle(&mut self, e: NonFatalError);
}

impl NonFatalErrorHandler for Noop {
    fn handle(&mut self, _: NonFatalError) {
        // Do nothing
    }
}

impl<T> NonFatalErrorHandler for T
where
    T: FnMut(NonFatalError),
{
    fn handle(&mut self, e: NonFatalError) {
        (self)(e);
    }
}

#[cfg(feature = "real-sys")]
impl<F: Default> Default for WhichConfig<&sys::RealSys, F> {
    fn default() -> Self {
        Self {
            cwd: CwdOption::Unspecified,
            custom_path_list: None,
            binary_name: None,
            nonfatal_error_handler: F::default(),
            #[cfg(feature = "regex")]
            regex: None,
            sys: &sys::RealSys,
        }
    }
}

#[cfg(feature = "regex")]
type Regex = regex::Regex;

#[cfg(not(feature = "regex"))]
type Regex = ();

#[cfg(feature = "real-sys")]
impl WhichConfig<&sys::RealSys, Noop> {
    pub fn new() -> Self {
        Self::new_with_sys(&sys::RealSys)
    }
}

impl<TSys: Sys> WhichConfig<TSys, Noop> {
    /// Creates a new `WhichConfig` with the given `sys::Sys`.
    ///
    /// This is useful for providing all the system related
    /// functionality to this crate.
    pub fn new_with_sys(sys: TSys) -> Self {
        Self {
            cwd: CwdOption::Unspecified,
            custom_path_list: None,
            binary_name: None,
            nonfatal_error_handler: Noop,
            #[cfg(feature = "regex")]
            regex: None,
            sys,
        }
    }
}

impl<'a, TSys: Sys + 'a, F: NonFatalErrorHandler + 'a> WhichConfig<TSys, F> {
    /// Whether or not to use the current working directory. `true` by default.
    ///
    /// # Panics
    ///
    /// If regex was set previously, and you've just passed in `use_cwd: true`, this will panic.
    pub fn system_cwd(mut self, use_cwd: bool) -> Self {
        #[cfg(feature = "regex")]
        if self.regex.is_some() && use_cwd {
            panic!("which can't use regex and cwd at the same time!")
        }
        // Otherwise, keep custom cwd if specified.
        self.cwd = if use_cwd {
            CwdOption::UseSysCwd
        } else {
            CwdOption::RefuseCwd
        };
        self
    }

    /// Sets a custom path for resolving relative paths.
    ///
    /// # Panics
    ///
    /// If regex was set previously, this will panic.
    pub fn custom_cwd(mut self, cwd: path::PathBuf) -> Self {
        #[cfg(feature = "regex")]
        if self.regex.is_some() {
            panic!("which can't use regex and cwd at the same time!")
        }
        self.cwd = CwdOption::UseCustomCwd(cwd);
        self
    }

    /// Sets the path name regex to search for. You ***MUST*** call this, or [`Self::binary_name`] prior to searching.
    ///
    /// When `Regex` is disabled this function takes the unit type as a stand in. The parameter will change when
    /// `Regex` is enabled.
    ///
    /// # Panics
    ///
    /// If the `regex` feature wasn't turned on for this crate this will always panic. Additionally if a
    /// `cwd` (aka current working directory) or `binary_name` was set previously, this will panic, as those options
    /// are incompatible with `regex`.
    #[allow(unused_variables)]
    #[allow(unused_mut)]
    pub fn regex(mut self, regex: Regex) -> Self {
        #[cfg(not(feature = "regex"))]
        {
            panic!("which's regex feature was not enabled in your Cargo.toml!")
        }
        #[cfg(feature = "regex")]
        {
            if matches!(self.cwd, CwdOption::UseSysCwd)
                || matches!(self.cwd, CwdOption::UseCustomCwd(_))
            {
                panic!("which can't use regex and cwd at the same time!")
            }
            if self.binary_name.is_some() {
                panic!("which can't use `binary_name` and `regex` at the same time!");
            }
            self.regex = Some(regex);
            self
        }
    }

    /// Sets the path name to search for. You ***MUST*** call this, or [`Self::regex`] prior to searching.
    ///
    /// # Panics
    ///
    /// If a `regex` was set previously this will panic as this is not compatible with `regex`.
    pub fn binary_name(mut self, name: OsString) -> Self {
        #[cfg(feature = "regex")]
        if self.regex.is_some() {
            panic!("which can't use `binary_name` and `regex` at the same time!");
        }
        self.binary_name = Some(name);
        self
    }

    /// Uses the given string instead of the `PATH` env variable.
    pub fn custom_path_list(mut self, custom_path_list: OsString) -> Self {
        self.custom_path_list = Some(custom_path_list);
        self
    }

    /// Uses the `PATH` env variable. Enabled by default.
    pub fn system_path_list(mut self) -> Self {
        self.custom_path_list = None;
        self
    }

    /// Sets a closure that will receive non-fatal errors. You can also pass in other types
    /// that implement [`NonFatalErrorHandler`].
    ///
    /// # Example
    /// ```
    /// # #[cfg(feature = "real-sys")]
    /// # {
    /// # use which::WhichConfig;
    /// let mut nonfatal_errors = Vec::new();
    ///
    /// WhichConfig::new()
    ///     .binary_name("tar".into())
    ///     .nonfatal_error_handler(|e| nonfatal_errors.push(e))
    ///     .all_results()
    ///     .unwrap()
    ///     .collect::<Vec<_>>();
    ///
    /// if !nonfatal_errors.is_empty() {
    ///     println!("nonfatal errors encountered: {nonfatal_errors:?}");
    /// }
    /// # }
    /// ```
    ///
    /// You could also log it if you choose
    ///
    /// ```
    /// # #[cfg(feature = "real-sys")]
    /// # {
    /// # use which::WhichConfig;
    /// WhichConfig::new()
    ///     .binary_name("tar".into())
    ///     .nonfatal_error_handler(|e| eprintln!("{e}"))
    ///     .all_results()
    ///     .unwrap()
    ///     .collect::<Vec<_>>();
    /// # }
    /// ```
    pub fn nonfatal_error_handler<NewF>(self, handler: NewF) -> WhichConfig<TSys, NewF> {
        WhichConfig {
            custom_path_list: self.custom_path_list,
            cwd: self.cwd,
            binary_name: self.binary_name,
            nonfatal_error_handler: handler,
            #[cfg(feature = "regex")]
            regex: self.regex,
            sys: self.sys,
        }
    }

    /// Finishes configuring, runs the query and returns the first result.
    pub fn first_result(self) -> Result<path::PathBuf> {
        self.all_results()
            .and_then(|mut i| i.next().ok_or(Error::CannotFindBinaryPath))
    }

    /// Finishes configuring, runs the query and returns all results.
    pub fn all_results(self) -> Result<impl Iterator<Item = path::PathBuf> + 'a> {
        let paths = self.custom_path_list.or_else(|| self.sys.env_path());

        #[cfg(feature = "regex")]
        if let Some(regex) = self.regex {
            return Finder::new(self.sys)
                .find_re(regex, paths, self.nonfatal_error_handler)
                .map(|i| Box::new(i) as Box<dyn Iterator<Item = path::PathBuf> + 'a>);
        }

        let cwd = match self.cwd {
            CwdOption::RefuseCwd => None,
            CwdOption::UseCustomCwd(custom) => Some(custom),
            CwdOption::UseSysCwd | CwdOption::Unspecified => self.sys.current_dir().ok(),
        };

        Finder::new(self.sys)
            .find(
                self.binary_name.expect(
                    "binary_name not set! You must set binary_name or regex before searching!",
                ),
                paths,
                cwd,
                self.nonfatal_error_handler,
            )
            .map(|i| Box::new(i) as Box<dyn Iterator<Item = path::PathBuf> + 'a>)
    }
}

/// An owned, immutable wrapper around a `PathBuf` containing the path of an executable.
///
/// The constructed `PathBuf` is the output of `which` or `which_in`, but `which::Path` has the
/// advantage of being a type distinct from `std::path::Path` and `std::path::PathBuf`.
///
/// It can be beneficial to use `which::Path` instead of `std::path::Path` when you want the type
/// system to enforce the need for a path that exists and points to a binary that is executable.
///
/// Since `which::Path` implements `Deref` for `std::path::Path`, all methods on `&std::path::Path`
/// are also available to `&which::Path` values.
#[derive(Clone, PartialEq, Eq)]
pub struct Path {
    inner: path::PathBuf,
}

impl Path {
    /// Returns the path of an executable binary by name.
    ///
    /// This calls `which` and maps the result into a `Path`.
    #[cfg(feature = "real-sys")]
    pub fn new<T: AsRef<OsStr>>(binary_name: T) -> Result<Path> {
        which(binary_name).map(|inner| Path { inner })
    }

    /// Returns the paths of all executable binaries by a name.
    ///
    /// this calls `which_all` and maps the results into `Path`s.
    #[cfg(feature = "real-sys")]
    pub fn all<T: AsRef<OsStr>>(binary_name: T) -> Result<impl Iterator<Item = Path>> {
        which_all(binary_name).map(|inner| inner.map(|inner| Path { inner }))
    }

    /// Returns the path of an executable binary by name in the path list `paths` and using the
    /// current working directory `cwd` to resolve relative paths.
    ///
    /// This calls `which_in` and maps the result into a `Path`.
    #[cfg(feature = "real-sys")]
    pub fn new_in<T, U, V>(binary_name: T, paths: Option<U>, cwd: V) -> Result<Path>
    where
        T: AsRef<OsStr>,
        U: AsRef<OsStr>,
        V: AsRef<path::Path>,
    {
        which_in(binary_name, paths, cwd).map(|inner| Path { inner })
    }

    /// Returns all paths of an executable binary by name in the path list `paths` and using the
    /// current working directory `cwd` to resolve relative paths.
    ///
    /// This calls `which_in_all` and maps the results into a `Path`.
    #[cfg(feature = "real-sys")]
    pub fn all_in<'a, T, U, V>(
        binary_name: T,
        paths: Option<U>,
        cwd: V,
    ) -> Result<impl Iterator<Item = Path> + 'a>
    where
        T: AsRef<OsStr>,
        U: AsRef<OsStr>,
        V: AsRef<path::Path> + 'a,
    {
        which_in_all(binary_name, paths, cwd).map(|inner| inner.map(|inner| Path { inner }))
    }

    /// Returns a reference to a `std::path::Path`.
    pub fn as_path(&self) -> &path::Path {
        self.inner.as_path()
    }

    /// Consumes the `which::Path`, yielding its underlying `std::path::PathBuf`.
    pub fn into_path_buf(self) -> path::PathBuf {
        self.inner
    }
}

impl fmt::Debug for Path {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl std::ops::Deref for Path {
    type Target = path::Path;

    fn deref(&self) -> &path::Path {
        self.inner.deref()
    }
}

impl AsRef<path::Path> for Path {
    fn as_ref(&self) -> &path::Path {
        self.as_path()
    }
}

impl AsRef<OsStr> for Path {
    fn as_ref(&self) -> &OsStr {
        self.as_os_str()
    }
}

impl PartialEq<path::PathBuf> for Path {
    fn eq(&self, other: &path::PathBuf) -> bool {
        self.inner == *other
    }
}

impl PartialEq<Path> for path::PathBuf {
    fn eq(&self, other: &Path) -> bool {
        *self == other.inner
    }
}

/// An owned, immutable wrapper around a `PathBuf` containing the _canonical_ path of an
/// executable.
///
/// The constructed `PathBuf` is the result of `which` or `which_in` followed by
/// `Path::canonicalize`, but `CanonicalPath` has the advantage of being a type distinct from
/// `std::path::Path` and `std::path::PathBuf`.
///
/// It can be beneficial to use `CanonicalPath` instead of `std::path::Path` when you want the type
/// system to enforce the need for a path that exists, points to a binary that is executable, is
/// absolute, has all components normalized, and has all symbolic links resolved
///
/// Since `CanonicalPath` implements `Deref` for `std::path::Path`, all methods on
/// `&std::path::Path` are also available to `&CanonicalPath` values.
#[derive(Clone, PartialEq, Eq)]
pub struct CanonicalPath {
    inner: path::PathBuf,
}

impl CanonicalPath {
    /// Returns the canonical path of an executable binary by name.
    ///
    /// This calls `which` and `Path::canonicalize` and maps the result into a `CanonicalPath`.
    #[cfg(feature = "real-sys")]
    pub fn new<T: AsRef<OsStr>>(binary_name: T) -> Result<CanonicalPath> {
        which(binary_name)
            .and_then(|p| {
                sys::RealSys
                    .canonicalize(&p)
                    .map_err(|_| Error::CannotCanonicalize)
            })
            .map(|inner| CanonicalPath { inner })
    }

    /// Returns the canonical paths of an executable binary by name.
    ///
    /// This calls `which_all` and `Path::canonicalize` and maps the results into `CanonicalPath`s.
    #[cfg(feature = "real-sys")]
    pub fn all<T: AsRef<OsStr>>(
        binary_name: T,
    ) -> Result<impl Iterator<Item = Result<CanonicalPath>>> {
        which_all(binary_name).map(|inner| {
            inner.map(|inner| {
                sys::RealSys
                    .canonicalize(&inner)
                    .map_err(|_| Error::CannotCanonicalize)
                    .map(|inner| CanonicalPath { inner })
            })
        })
    }

    /// Returns the canonical path of an executable binary by name in the path list `paths` and
    /// using the current working directory `cwd` to resolve relative paths.
    ///
    /// This calls `which_in` and `Path::canonicalize` and maps the result into a `CanonicalPath`.
    #[cfg(feature = "real-sys")]
    pub fn new_in<T, U, V>(binary_name: T, paths: Option<U>, cwd: V) -> Result<CanonicalPath>
    where
        T: AsRef<OsStr>,
        U: AsRef<OsStr>,
        V: AsRef<path::Path>,
    {
        which_in(binary_name, paths, cwd)
            .and_then(|p| {
                sys::RealSys
                    .canonicalize(&p)
                    .map_err(|_| Error::CannotCanonicalize)
            })
            .map(|inner| CanonicalPath { inner })
    }

    /// Returns all of the canonical paths of an executable binary by name in the path list `paths` and
    /// using the current working directory `cwd` to resolve relative paths.
    ///
    /// This calls `which_in_all` and `Path::canonicalize` and maps the result into a `CanonicalPath`.
    #[cfg(feature = "real-sys")]
    pub fn all_in<'a, T, U, V>(
        binary_name: T,
        paths: Option<U>,
        cwd: V,
    ) -> Result<impl Iterator<Item = Result<CanonicalPath>> + 'a>
    where
        T: AsRef<OsStr>,
        U: AsRef<OsStr>,
        V: AsRef<path::Path> + 'a,
    {
        which_in_all(binary_name, paths, cwd).map(|inner| {
            inner.map(|inner| {
                sys::RealSys
                    .canonicalize(&inner)
                    .map_err(|_| Error::CannotCanonicalize)
                    .map(|inner| CanonicalPath { inner })
            })
        })
    }

    /// Returns a reference to a `std::path::Path`.
    pub fn as_path(&self) -> &path::Path {
        self.inner.as_path()
    }

    /// Consumes the `which::CanonicalPath`, yielding its underlying `std::path::PathBuf`.
    pub fn into_path_buf(self) -> path::PathBuf {
        self.inner
    }
}

impl fmt::Debug for CanonicalPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl std::ops::Deref for CanonicalPath {
    type Target = path::Path;

    fn deref(&self) -> &path::Path {
        self.inner.deref()
    }
}

impl AsRef<path::Path> for CanonicalPath {
    fn as_ref(&self) -> &path::Path {
        self.as_path()
    }
}

impl AsRef<OsStr> for CanonicalPath {
    fn as_ref(&self) -> &OsStr {
        self.as_os_str()
    }
}

impl PartialEq<path::PathBuf> for CanonicalPath {
    fn eq(&self, other: &path::PathBuf) -> bool {
        self.inner == *other
    }
}

impl PartialEq<CanonicalPath> for path::PathBuf {
    fn eq(&self, other: &CanonicalPath) -> bool {
        *self == other.inner
    }
}
