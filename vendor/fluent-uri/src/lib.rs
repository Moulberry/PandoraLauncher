#![warn(missing_debug_implementations, missing_docs, rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(not(feature = "std"), no_std)]

//! A generic URI parser that strictly adheres to IETF [RFC 3986].
//!
//! [RFC 3986]: https://datatracker.ietf.org/doc/html/rfc3986/
//!
//! See the documentation of [`Uri`] for more details.
//!
//! # Feature flags
//!
//! All features except `std` are disabled by default. Note that the last two features
//! each alter the enum [`HostData`] in a backward incompatible way that could make it
//! impossible for two crates that depend on different features of `fluent-uri` to
//! be used together.
//!
//! - `std`: Enables `std` support. This includes [`Error`] implementations
//!   and `Ip{v4, v6}Addr` support in [`HostData`].
//!
//! - `ipv_future`: Enables the parsing of [IPvFuture] literal addresses,
//!   which fails with [`InvalidIpLiteral`] when disabled.
//!
//!     Only enable this feature when you have a compelling reason to do so, such as
//!     that you have to deal with an existing system where the IPvFuture format is
//!     in use.
//!
//! - `rfc6874bis`: Enables the parsing of IPv6 zone identifiers,
//!   such as in `https://[fe80::abcd%en1]`.
//!
//!     This feature is based on the homonymous [draft] and is thus subject to change.
//!
//! [`Error`]: std::error::Error
//! [IPvFuture]: https://datatracker.ietf.org/doc/html/rfc3986/#section-3.2.2
//! [`InvalidIpLiteral`]: ParseErrorKind::InvalidIpLiteral
//! [draft]: https://datatracker.ietf.org/doc/html/draft-ietf-6man-rfc6874bis-05

extern crate alloc;

/// Utilities for percent-encoding.
pub mod enc;

mod fmt;

mod view;
pub use view::*;

mod parser;

use crate::enc::{EStr, Split};
use alloc::{string::String, vec::Vec};
use core::{iter::Iterator, marker::PhantomData, mem::ManuallyDrop, ptr::NonNull, slice, str};

#[cfg(feature = "std")]
use std::net::{Ipv4Addr, Ipv6Addr};

mod internal;
use internal::*;

/// Detailed cause of a [`ParseError`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// Invalid percent-encoded octet that is either non-hexadecimal or incomplete.
    ///
    /// The error index points to the percent character "%" of the octet.
    InvalidOctet,
    /// Unexpected character that is not allowed by the URI syntax.
    ///
    /// The error index points to the character.
    UnexpectedChar,
    /// Invalid IP literal address.
    ///
    /// The error index points to the preceding left square bracket "[".
    InvalidIpLiteral,
}

/// An error occurred when parsing URI references.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseError {
    index: u32,
    kind: ParseErrorKind,
}

impl ParseError {
    /// Returns the index where the error occurred in the input string.
    #[inline]
    pub fn index(&self) -> usize {
        self.index as usize
    }

    /// Returns the detailed cause of the error.
    #[inline]
    pub fn kind(&self) -> ParseErrorKind {
        self.kind
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}

type Result<T, E = ParseError> = core::result::Result<T, E>;

#[cold]
fn len_overflow() -> ! {
    panic!("input length exceeds i32::MAX");
}

/// A [URI reference] defined in RFC 3986.
///
/// [URI reference]: https://datatracker.ietf.org/doc/html/rfc3986/#section-4.1
///
/// # Variants
///
/// There are three variants of `Uri` in total:
///
/// - `Uri<&str>`: borrowed; immutable.
/// - `Uri<&mut [u8]>`: borrowed; in-place mutable.
/// - `Uri<String>`: owned; immutable.
///
/// Lifetimes are correctly handled in a way that `Uri<&'a str>` and `Uri<&'a mut [u8]>`
/// both output references with lifetime `'a` where appropriate. This allows you to drop
/// a temporary `Uri` while keeping the output references:
///
/// ```
/// use fluent_uri::Uri;
///
/// let mut bytes = *b"foo:bar";
///
/// let uri = Uri::parse(&bytes)?;
/// let path = uri.path();
/// drop(uri);
/// assert_eq!(path.as_str(), "bar");
///
/// let mut uri = Uri::parse_mut(&mut bytes)?;
/// let path = uri.take_path();
/// drop(uri);
/// assert_eq!(path.as_str(), "bar");
/// # Ok::<_, fluent_uri::ParseError>(())
/// ```
///
/// # Examples
///
/// Create and convert between `Uri<&str>` and `Uri<String>`:
///
/// ```
/// use fluent_uri::Uri;
///
/// let uri_str = "http://example.com/";
///
/// // Create a `Uri<&str>` from a string slice.
/// let uri_a: Uri<&str> = Uri::parse(uri_str)?;
///
/// // Create a `Uri<String>` from an owned string.
/// let uri_b: Uri<String> = Uri::parse_from(uri_str.to_owned()).map_err(|e| e.1)?;
///
/// // Convert a `Uri<&str>` to a `Uri<String>`.
/// let uri_c: Uri<String> = uri_a.to_owned();
///
/// // Borrow a `Uri<String>` as a `Uri<&str>`.
/// let uri_d: &Uri<&str> = uri_b.borrow();
/// # Ok::<_, fluent_uri::ParseError>(())
/// ```
///
/// Decode and extract query parameters in-place from a URI reference:
///
/// ```
/// use fluent_uri::{ParseError, Uri};
/// use std::collections::HashMap;
///
/// fn decode_and_extract_query(
///     bytes: &mut [u8],
/// ) -> Result<(Uri<&mut [u8]>, HashMap<&str, &str>), ParseError> {
///     let mut uri = Uri::parse_mut(bytes)?;
///     let map = if let Some(query) = uri.take_query() {
///         query
///             .split_view('&')
///             .flat_map(|pair| pair.split_once_view('='))
///             .map(|(k, v)| (k.decode_in_place(), v.decode_in_place()))
///             .flat_map(|(k, v)| k.into_str().ok().zip(v.into_str().ok()))
///             .collect()
///     } else {
///         HashMap::new()
///     };
///     Ok((uri, map))
/// }
///
/// let mut bytes = *b"?lang=Rust&mascot=Ferris%20the%20crab";
/// let (uri, query) = decode_and_extract_query(&mut bytes)?;
///
/// assert_eq!(query["lang"], "Rust");
/// assert_eq!(query["mascot"], "Ferris the crab");
///
/// // The query is taken from the `Uri`.
/// assert!(uri.query().is_none());
/// // In-place decoding is like this if you're interested:
/// assert_eq!(&bytes, b"?lang=Rust&mascot=Ferris the crabcrab");
/// # Ok::<_, fluent_uri::ParseError>(())
/// ```
// TODO: Create a mutable copy of an immutable `Uri` in a buffer:
#[repr(C)]
pub struct Uri<T: Storage> {
    ptr: T::Ptr,
    data: Data,
    _marker: PhantomData<T>,
}

impl<'a> Uri<&'a str> {
    /// Parses a URI reference from a byte sequence into a `Uri<&str>`.
    ///
    /// This function validates the input strictly except that UTF-8 validation is not
    /// performed on a percent-encoded registered name (see [Section 3.2.2, RFC 3986][1]).
    /// Care should be taken when dealing with such cases.
    ///
    /// [1]: https://datatracker.ietf.org/doc/html/rfc3986/#section-3.2.2
    ///
    /// # Panics
    ///
    /// Panics if the input length is greater than [`i32::MAX`].
    pub fn parse<S: AsRef<[u8]> + ?Sized>(s: &S) -> Result<Uri<&str>> {
        let bytes = s.as_ref();
        if bytes.len() > i32::MAX as usize {
            len_overflow();
        }
        // SAFETY: We're using the correct pointer, length, capacity, and generics.
        unsafe { parser::parse(bytes.as_ptr() as *mut _, bytes.len() as u32, 0) }
    }

    /// Duplicates this `Uri<&str>`.
    #[inline]
    pub fn dup(&self) -> Uri<&'a str> {
        Uri {
            data: self.data.clone(),
            ..*self
        }
    }

    /// Creates a new `Uri<String>` by cloning the contents of this `Uri<&str>`.
    #[inline]
    pub fn to_owned(&self) -> Uri<String> {
        let len = self.len();
        // We're allocating manually because there is no guarantee that
        // `String::to_owned` gives the exact capacity of `self.len`.
        let mut vec = ManuallyDrop::new(Vec::with_capacity(len as usize));
        let ptr = vec.as_mut_ptr();

        // SAFETY: The capacity of `vec` is exactly `self.len`.
        // Newly allocated `Vec` won't overlap with existing data.
        unsafe {
            self.ptr.get().copy_to_nonoverlapping(ptr, len as usize);
        }

        Uri {
            // SAFETY: The pointer is not null and the length and capacity are correct.
            ptr: unsafe { Capped::new(ptr, len, len) },
            data: self.data.clone(),
            _marker: PhantomData,
        }
    }
}

impl<'i, 'o, T: Io<'i, 'o> + AsRef<str>> Uri<T> {
    #[inline]
    /// Returns the URI reference as a string slice.
    pub fn as_str(&'i self) -> &'o str {
        // SAFETY: The indexes are within bounds and the validation is done.
        unsafe { self.slice(0, self.len()) }
    }

    /// Creates a mutable copy of this `Uri` in the given buffer.
    ///
    /// The type of a buffer may be:
    ///
    /// - [`Vec<u8>`]: bytes appended to the end; returns a [`TryReserveError`]
    ///   when the allocation fails.
    ///
    /// - [`[u8]`](prim@slice) or [`[MaybeUninit<u8>]`](prim@slice): bytes
    ///   written from the start; returns a [`BufferTooSmallError`] when
    ///   the buffer is too small.
    ///
    /// [`TryReserveError`]: std::collections::TryReserveError
    /// [`BufferTooSmallError`]: crate::enc::BufferTooSmallError
    #[cfg(feature = "unstable")]
    #[inline]
    pub fn to_mut_in<'b, B: crate::enc::internal::Buf + ?Sized>(
        &self,
        buf: &'b mut B,
    ) -> Result<Uri<&'b mut [u8]>, B::PrepareError> {
        let len = self.len();
        let ptr = buf.prepare(len as usize)?;

        // SAFETY: We have reserved enough space in the buffer, and
        // mutable reference `buf` ensures exclusive access.
        unsafe {
            self.ptr.get().copy_to_nonoverlapping(ptr, len as usize);
            buf.finish(len as usize);
        }

        Ok(Uri {
            // SAFETY: The pointer is not null and the length and capacity are correct.
            ptr: unsafe { Uncapped::new(ptr, len, 0) },
            data: self.data.clone(),
            _marker: PhantomData,
        })
    }
}

#[cold]
fn component_taken() -> ! {
    panic!("component already taken");
}

impl<'i, 'o, T: Io<'i, 'o>> Uri<T> {
    #[inline]
    fn len(&self) -> u32 {
        self.ptr.len()
    }

    #[inline]
    unsafe fn slice(&'i self, start: u32, end: u32) -> &'o str {
        debug_assert!(start <= end && end <= self.len());
        // SAFETY: The caller must ensure that the indexes are within bounds.
        let bytes = unsafe {
            slice::from_raw_parts(self.ptr.get().add(start as usize), (end - start) as usize)
        };
        // SAFETY: The parser guarantees that the bytes are valid UTF-8.
        unsafe { str::from_utf8_unchecked(bytes) }
    }

    #[inline]
    unsafe fn eslice(&'i self, start: u32, end: u32) -> &'o EStr {
        // SAFETY: The caller must ensure that the indexes are within bounds.
        let s = unsafe { self.slice(start, end) };
        // SAFETY: The caller must ensure that the subslice is properly encoded.
        unsafe { EStr::new_unchecked(s.as_bytes()) }
    }

    /// Returns the [scheme] component.
    ///
    /// [scheme]: https://datatracker.ietf.org/doc/html/rfc3986/#section-3.1
    #[inline]
    pub fn scheme(&'i self) -> Option<&'o Scheme> {
        // SAFETY: The indexes are within bounds and the validation is done.
        self.scheme_end
            .map(|i| Scheme::new(unsafe { self.slice(0, i.get()) }))
    }

    /// Returns the [authority] component.
    ///
    /// [authority]: https://datatracker.ietf.org/doc/html/rfc3986/#section-3.2
    #[inline]
    pub fn authority(&self) -> Option<&Authority<T>> {
        if T::is_mut() && self.tag.contains(Tag::AUTH_TAKEN) {
            return None;
        }
        if self.auth.is_some() {
            // SAFETY: The authority is present and not modified.
            Some(unsafe { Authority::new(self) })
        } else {
            None
        }
    }

    /// Returns the [path] component.
    ///
    /// [path]: https://datatracker.ietf.org/doc/html/rfc3986/#section-3.3
    ///
    /// # Panics
    ///
    /// Panics if the path component is already taken.
    #[inline]
    pub fn path(&'i self) -> &'o Path {
        if T::is_mut() && self.tag.contains(Tag::PATH_TAKEN) {
            component_taken();
        }
        // SAFETY: The indexes are within bounds and the validation is done.
        Path::new(unsafe { self.eslice(self.path_bounds.0, self.path_bounds.1) })
    }

    /// Returns the [query] component.
    ///
    /// [query]: https://datatracker.ietf.org/doc/html/rfc3986/#section-3.4
    #[inline]
    pub fn query(&'i self) -> Option<&'o EStr> {
        // SAFETY: The indexes are within bounds and the validation is done.
        self.query_end
            .map(|i| unsafe { self.eslice(self.path_bounds.1 + 1, i.get()) })
    }

    /// Returns the [fragment] component.
    ///
    /// [fragment]: https://datatracker.ietf.org/doc/html/rfc3986/#section-3.5
    #[inline]
    pub fn fragment(&'i self) -> Option<&'o EStr> {
        // SAFETY: The indexes are within bounds and the validation is done.
        self.fragment_start
            .map(|i| unsafe { self.eslice(i.get(), self.len()) })
    }

    /// Returns `true` if the URI reference is [relative], i.e., without a scheme.
    ///
    /// Note that this method is not the opposite of [`is_absolute`].
    ///
    /// [relative]: https://datatracker.ietf.org/doc/html/rfc3986/#section-4.2
    /// [`is_absolute`]: Self::is_absolute
    ///
    /// # Examples
    ///
    /// ```
    /// use fluent_uri::Uri;
    ///
    /// let uri = Uri::parse("/path/to/file")?;
    /// assert!(uri.is_relative());
    /// let uri = Uri::parse("http://example.com/")?;
    /// assert!(!uri.is_relative());
    /// # Ok::<_, fluent_uri::ParseError>(())
    /// ```
    #[inline]
    pub fn is_relative(&self) -> bool {
        self.scheme_end.is_none()
    }

    /// Returns `true` if the URI reference is [absolute], i.e., with a scheme and without a fragment.
    ///
    /// Note that this method is not the opposite of [`is_relative`].
    ///
    /// [absolute]: https://datatracker.ietf.org/doc/html/rfc3986/#section-4.3
    /// [`is_relative`]: Self::is_relative
    ///
    /// # Examples
    ///
    /// ```
    /// use fluent_uri::Uri;
    ///
    /// let uri = Uri::parse("http://example.com/")?;
    /// assert!(uri.is_absolute());
    /// let uri = Uri::parse("http://example.com/#title1")?;
    /// assert!(!uri.is_absolute());
    /// let uri = Uri::parse("/path/to/file")?;
    /// assert!(!uri.is_absolute());
    /// # Ok::<_, fluent_uri::ParseError>(())
    /// ```
    #[inline]
    pub fn is_absolute(&self) -> bool {
        self.scheme_end.is_some() && self.fragment_start.is_none()
    }
}

impl<'a> Uri<&'a mut [u8]> {
    /// Parses a URI reference from a mutable byte sequence into a `Uri<&mut [u8]>`.
    ///
    /// See the [`parse`] function for more details.
    ///
    /// [`parse`]: Uri::parse
    ///
    /// # Panics
    ///
    /// Panics if the input length is greater than [`i32::MAX`].
    #[inline]
    pub fn parse_mut<S: AsMut<[u8]> + ?Sized>(s: &mut S) -> Result<Uri<&mut [u8]>> {
        let bytes = s.as_mut();
        if bytes.len() > i32::MAX as usize {
            len_overflow();
        }
        // SAFETY: We're using the correct pointer, length, capacity, and generics.
        unsafe { parser::parse(bytes.as_mut_ptr(), bytes.len() as u32, 0) }
    }

    #[inline]
    unsafe fn view<T>(&mut self, start: u32, end: u32) -> View<'a, T>
    where
        T: ?Sized + Lens<Target = [u8]>,
    {
        debug_assert!(start <= end && end <= self.len());
        // SAFETY: The caller must ensure that the indexes are within bounds.
        let bytes = unsafe {
            slice::from_raw_parts_mut(self.ptr.get().add(start as usize), (end - start) as usize)
        };
        // SAFETY: The caller must ensure that the bytes are properly encoded.
        unsafe { View::new(bytes) }
    }

    /// Takes a view of the scheme component, leaving a `None` in its place.
    #[inline]
    pub fn take_scheme(&mut self) -> Option<View<'a, Scheme>> {
        // SAFETY: The indexes are within bounds and the validation is done.
        self.scheme_end
            .take()
            .map(|i| unsafe { self.view(0, i.get()) })
    }

    /// Takes a view of the authority component, leaving a `None` in its place.
    #[inline]
    pub fn take_authority(&mut self) -> Option<View<'_, Authority<&'a mut [u8]>>> {
        if self.tag.contains(Tag::AUTH_TAKEN) {
            return None;
        }
        self.tag |= Tag::AUTH_TAKEN;

        if self.auth.is_some() {
            // SAFETY: The authority is present and not modified.
            Some(unsafe { View::new(self) })
        } else {
            None
        }
    }

    /// Takes a view of the path component.
    ///
    /// # Panics
    ///
    /// Panics if the path component is already taken.
    #[inline]
    pub fn take_path(&mut self) -> View<'a, Path> {
        if self.tag.contains(Tag::PATH_TAKEN) {
            component_taken();
        }
        self.tag |= Tag::PATH_TAKEN;

        // SAFETY: The indexes are within bounds and the validation is done.
        unsafe { self.view(self.path_bounds.0, self.path_bounds.1) }
    }

    /// Takes a view of the query component, leaving a `None` in its place.
    #[inline]
    pub fn take_query(&mut self) -> Option<View<'a, EStr>> {
        // SAFETY: The indexes are within bounds and the validation is done.
        self.query_end
            .take()
            .map(|i| unsafe { self.view(self.path_bounds.1 + 1, i.get()) })
    }

    /// Takes a view of the fragment component, leaving a `None` in its place.
    #[inline]
    pub fn take_fragment(&mut self) -> Option<View<'a, EStr>> {
        // SAFETY: The indexes are within bounds and the validation is done.
        self.fragment_start
            .take()
            .map(|i| unsafe { self.view(i.get(), self.len()) })
    }
}

impl Uri<String> {
    /// Parses a URI reference from a [`String`] or [`Vec<u8>`] into a `Uri<String>`.
    ///
    /// See the [`parse`] function for more details.
    ///
    /// [`parse`]: Uri::parse
    ///
    /// # Panics
    ///
    /// Panics if the input capacity is greater than [`i32::MAX`].
    #[inline]
    pub fn parse_from<T: IntoOwnedUri>(t: T) -> Result<Uri<String>, (T, ParseError)> {
        #[cold]
        fn cap_overflow() -> ! {
            panic!("input capacity exceeds i32::MAX");
        }

        let buf = ManuallyDrop::new(t);
        let (ptr, len, cap) = buf.as_raw_parts();
        if cap > i32::MAX as usize {
            cap_overflow();
        }

        // SAFETY: We're using the correct pointer, length, capacity, and generics.
        match unsafe { parser::parse(ptr, len as u32, cap as u32) } {
            Ok(out) => Ok(out),
            Err(e) => Err((ManuallyDrop::into_inner(buf), e)),
        }
    }

    /// Consumes this `Uri` and yields the underlying [`String`] storage.
    ///
    /// # Examples
    ///
    /// ```
    /// use fluent_uri::Uri;
    ///
    /// let uri = Uri::parse("https://www.rust-lang.org/")?.to_owned();
    /// let string = uri.into_string();
    /// # Ok::<_, fluent_uri::ParseError>(())
    /// ```
    #[inline]
    pub fn into_string(self) -> String {
        self.ptr.into_string()
    }

    /// Borrows this `Uri<String>` as a reference to `Uri<&str>`.
    #[inline]
    // We can't impl `Borrow` due to the limitation of lifetimes.
    #[allow(clippy::should_implement_trait)]
    pub fn borrow(&self) -> &Uri<&str> {
        // SAFETY: `Uri` has a fixed layout, `Uri<&str>` with a capacity is
        // always fine and the lifetimes are correct.
        unsafe { &*(self as *const Uri<String> as *const Uri<&str>) }
    }
}

impl Clone for Uri<String> {
    #[inline]
    fn clone(&self) -> Self {
        self.borrow().to_owned()
    }
}

impl<T: Storage> Default for Uri<T> {
    /// Creates an empty `Uri`.
    #[inline]
    fn default() -> Self {
        Uri {
            ptr: T::Ptr::DANGLING,
            data: Data::INIT,
            _marker: PhantomData,
        }
    }
}

// SAFETY: `&str`, `&mut [u8]` and `String` are all Send and Sync.
unsafe impl<T: Storage> Send for Uri<T> {}
unsafe impl<T: Storage> Sync for Uri<T> {}

/// The [scheme] component of URI reference.
///
/// [scheme]: https://datatracker.ietf.org/doc/html/rfc3986/#section-3.1
#[repr(transparent)]
pub struct Scheme(str);

const ASCII_CASE_MASK: u8 = 0b010_0000;

impl Scheme {
    #[inline]
    fn new(scheme: &str) -> &Scheme {
        // SAFETY: Transparency holds.
        unsafe { &*(scheme as *const str as *const Scheme) }
    }

    /// Returns the scheme as a string slice.
    ///
    /// Note that the scheme is case-insensitive. You should typically use
    /// [`eq_lowercase`] for testing if the scheme is a desired one.
    ///
    /// [`eq_lowercase`]: Self::eq_lowercase
    ///
    /// # Examples
    ///
    /// ```
    /// use fluent_uri::Uri;
    ///
    /// let uri = Uri::parse("HTTP://example.com/")?;
    /// let scheme = uri.scheme().unwrap();
    /// assert_eq!(scheme.as_str(), "HTTP");
    /// # Ok::<_, fluent_uri::ParseError>(())
    /// ```
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the scheme as a string in lower case.
    ///
    /// # Examples
    ///
    /// ```
    /// use fluent_uri::Uri;
    ///
    /// let uri = Uri::parse("HTTP://example.com/")?;
    /// let scheme = uri.scheme().unwrap();
    /// assert_eq!(scheme.to_lowercase(), "http");
    /// # Ok::<_, fluent_uri::ParseError>(())
    /// ```
    #[inline]
    pub fn to_lowercase(&self) -> String {
        let bytes = self.0.bytes().map(|x| x | ASCII_CASE_MASK).collect();
        // SAFETY: Setting the sixth bit keeps UTF-8.
        unsafe { String::from_utf8_unchecked(bytes) }
    }

    /// Checks if the scheme equals case-insensitively with a lowercase string.
    ///
    /// This method is slightly faster than [`str::eq_ignore_ascii_case`] but will
    /// always return `false` if there is any uppercase letter in the given string.
    ///
    /// # Examples
    ///
    /// ```
    /// use fluent_uri::Uri;
    ///
    /// let uri = Uri::parse("HTTP://example.com/")?;
    /// let scheme = uri.scheme().unwrap();
    /// assert!(scheme.eq_lowercase("http"));
    /// // Always return `false` if there's any uppercase letter in the given string.
    /// assert!(!scheme.eq_lowercase("hTTp"));
    /// # Ok::<_, fluent_uri::ParseError>(())
    /// ```
    #[inline]
    pub fn eq_lowercase(&self, other: &str) -> bool {
        let (a, b) = (self.0.as_bytes(), other.as_bytes());
        // NOTE: Using iterators results in poor codegen here.
        if a.len() != b.len() {
            false
        } else {
            for i in 0..a.len() {
                // The only characters allowed in a scheme are alphabets, digits, "+", "-" and ".",
                // the ASCII codes of which allow us to simply set the sixth bit and compare.
                if a[i] | ASCII_CASE_MASK != b[i] {
                    return false;
                }
            }
            true
        }
    }
}

/// The [authority] component of URI reference.
///
/// [authority]: https://datatracker.ietf.org/doc/html/rfc3986/#section-3.2
#[repr(transparent)]
pub struct Authority<T: Storage> {
    uri: Uri<T>,
}

impl<'i, 'o, T: Io<'i, 'o>> Authority<T> {
    #[inline]
    unsafe fn new(uri: &Uri<T>) -> &Authority<T> {
        // SAFETY: Transparency holds.
        // The caller must ensure that the authority is present and not modified.
        unsafe { &*(uri as *const Uri<T> as *const Authority<T>) }
    }

    #[inline]
    fn data(&self) -> &AuthData {
        // SAFETY: When authority is present, `auth` must be `Some`.
        unsafe { self.uri.auth.as_ref().unwrap_unchecked() }
    }

    #[inline]
    fn start(&self) -> u32 {
        self.data().start.get().get()
    }

    #[inline]
    fn end(&self) -> u32 {
        if T::is_mut() && self.uri.tag.contains(Tag::PORT_TAKEN) {
            self.host_bounds().1
        } else {
            self.uri.path_bounds.0
        }
    }

    #[inline]
    fn host_bounds(&self) -> (u32, u32) {
        self.data().host_bounds
    }

    /// Returns the authority as a string slice.
    ///
    /// # Panics
    ///
    /// Panics if the host subcomponent is already taken.
    ///
    /// # Examples
    ///
    /// ```
    /// use fluent_uri::Uri;
    ///
    /// let uri = Uri::parse("ftp://user@[fe80::abcd]:6780/")?;
    /// let authority = uri.authority().unwrap();
    /// assert_eq!(authority.as_str(), "user@[fe80::abcd]:6780");
    /// # Ok::<_, fluent_uri::ParseError>(())
    /// ```
    #[inline]
    pub fn as_str(&'i self) -> &'o str {
        if T::is_mut() && self.uri.tag.contains(Tag::HOST_TAKEN) {
            component_taken();
        }
        // SAFETY: The indexes are within bounds and the validation is done.
        unsafe { self.uri.slice(self.start(), self.end()) }
    }

    /// Returns the [userinfo] subcomponent.
    ///
    /// [userinfo]: https://datatracker.ietf.org/doc/html/rfc3986/#section-3.2.1
    ///
    /// # Examples
    ///
    /// ```
    /// use fluent_uri::Uri;
    ///
    /// let uri = Uri::parse("ftp://user@192.168.1.24/")?;
    /// let authority = uri.authority().unwrap();
    /// assert_eq!(authority.userinfo().unwrap(), "user");
    /// # Ok::<_, fluent_uri::ParseError>(())
    /// ```
    #[inline]
    pub fn userinfo(&'i self) -> Option<&'o EStr> {
        let (start, host_start) = (self.start(), self.host_bounds().0);
        // SAFETY: The indexes are within bounds and the validation is done.
        (start != host_start).then(|| unsafe { self.uri.eslice(start, host_start - 1) })
    }

    /// Returns the [host] subcomponent.
    ///
    /// [host]: https://datatracker.ietf.org/doc/html/rfc3986/#section-3.2.2
    ///
    /// # Panics
    ///
    /// Panics if the host subcomponent is already taken.
    pub fn host(&self) -> &Host<T> {
        if T::is_mut() && self.uri.tag.contains(Tag::HOST_TAKEN) {
            component_taken();
        }
        // SAFETY: The host is not modified.
        unsafe { Host::new(self) }
    }

    /// Returns the [port] subcomponent.
    ///
    /// [port]: https://datatracker.ietf.org/doc/html/rfc3986/#section-3.2.3
    ///
    /// Note that in the generic URI syntax, the port may be empty, with leading zeros, or very large.
    /// It is up to you to decide whether to deny such a port, fallback to the scheme's default if it
    /// is empty, ignore the leading zeros, or use a different addressing mechanism that allows a large port.
    ///
    /// # Examples
    ///
    /// ```
    /// use fluent_uri::Uri;
    ///
    /// let uri = Uri::parse("ssh://device.local:4673/")?;
    /// let authority = uri.authority().unwrap();
    /// assert_eq!(authority.port(), Some("4673"));
    ///
    /// let uri = Uri::parse("ssh://device.local:/")?;
    /// let authority = uri.authority().unwrap();
    /// assert_eq!(authority.port(), Some(""));
    ///
    /// let uri = Uri::parse("ssh://device.local/")?;
    /// let authority = uri.authority().unwrap();
    /// assert_eq!(authority.port(), None);
    /// # Ok::<_, fluent_uri::ParseError>(())
    /// ```
    #[inline]
    pub fn port(&'i self) -> Option<&'o str> {
        if T::is_mut() && self.uri.tag.contains(Tag::PORT_TAKEN) {
            return None;
        }
        let (host_end, end) = (self.host_bounds().1, self.uri.path_bounds.0);
        // SAFETY: The indexes are within bounds and the validation is done.
        (host_end != end).then(|| unsafe { self.uri.slice(host_end + 1, end) })
    }
}

/// The [host] subcomponent of authority.
///
/// [host]: https://datatracker.ietf.org/doc/html/rfc3986/#section-3.2.2
#[repr(transparent)]
pub struct Host<T: Storage> {
    auth: Authority<T>,
}

impl<'i, 'o, T: Io<'i, 'o>> Host<T> {
    #[inline]
    unsafe fn new(auth: &Authority<T>) -> &Host<T> {
        // SAFETY: Transparency holds.
        // The caller must ensure that the host is not modified.
        unsafe { &*(auth as *const Authority<T> as *const Host<T>) }
    }

    #[inline]
    fn bounds(&self) -> (u32, u32) {
        self.auth.host_bounds()
    }

    #[inline]
    fn raw_data(&self) -> &RawHostData {
        &self.auth.data().host_data
    }

    /// Returns the host as a string slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use fluent_uri::Uri;
    ///
    /// let uri = Uri::parse("ftp://user@[::1]/")?;
    /// let authority = uri.authority().unwrap();
    /// assert_eq!(authority.host().as_str(), "[::1]");
    /// # Ok::<_, fluent_uri::ParseError>(())
    /// ```
    #[inline]
    pub fn as_str(&'i self) -> &'o str {
        // SAFETY: The indexes are within bounds and the validation is done.
        unsafe { self.auth.uri.slice(self.bounds().0, self.bounds().1) }
    }

    /// Returns the structured host data.
    #[inline]
    pub fn data(&'i self) -> HostData<'o> {
        let _data = self.raw_data();
        let tag = self.auth.uri.tag;
        // SAFETY: We only access the union after checking the tag.
        unsafe {
            if tag.contains(Tag::HOST_REG_NAME) {
                // SAFETY: The validation is done.
                return HostData::RegName(EStr::new_unchecked(self.as_str().as_bytes()));
            } else if tag.contains(Tag::HOST_IPV4) {
                return HostData::Ipv4(
                    #[cfg(feature = "std")]
                    _data.ipv4_addr,
                );
            }
            #[cfg(feature = "ipv_future")]
            if !tag.contains(Tag::HOST_IPV6) {
                let dot_i = _data.ipv_future_dot_i;
                let bounds = self.bounds();
                // SAFETY: The indexes are within bounds and the validation is done.
                return HostData::IpvFuture {
                    ver: self.auth.uri.slice(bounds.0 + 2, dot_i),
                    addr: self.auth.uri.slice(dot_i + 1, bounds.1 - 1),
                };
            }
            HostData::Ipv6 {
                #[cfg(feature = "std")]
                addr: _data.ipv6.addr,
                // SAFETY: The indexes are within bounds and the validation is done.
                #[cfg(feature = "rfc6874bis")]
                zone_id: _data
                    .ipv6
                    .zone_id_start
                    .map(|start| self.auth.uri.slice(start.get(), self.bounds().1 - 1)),
            }
        }
    }
}

/// Structured host data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostData<'a> {
    /// An IPv4 address.
    #[cfg_attr(not(feature = "std"), non_exhaustive)]
    Ipv4(#[cfg(feature = "std")] Ipv4Addr),
    /// An IPv6 address.
    #[cfg_attr(not(feature = "std"), non_exhaustive)]
    Ipv6 {
        /// The address.
        #[cfg(feature = "std")]
        addr: Ipv6Addr,
        /// An optional zone identifier.
        ///
        /// This is supported on **crate feature `rfc6874bis`** only.
        #[cfg(feature = "rfc6874bis")]
        zone_id: Option<&'a str>,
    },
    /// An IP address of future version.
    ///
    /// This is supported on **crate feature `ipv_future`** only.
    #[cfg(feature = "ipv_future")]
    IpvFuture {
        /// The version.
        ver: &'a str,
        /// The address.
        addr: &'a str,
    },
    /// A registered name.
    RegName(&'a EStr),
}

/// The [path] component of URI reference.
///
/// [path]: https://datatracker.ietf.org/doc/html/rfc3986/#section-3.3
#[repr(transparent)]
pub struct Path {
    inner: EStr,
}

impl Path {
    #[inline]
    fn new(path: &EStr) -> &Path {
        // SAFETY: Transparency holds.
        unsafe { &*(path as *const EStr as *const Path) }
    }

    /// Yields the underlying [`EStr`].
    #[inline]
    pub fn as_estr(&self) -> &EStr {
        &self.inner
    }

    /// Returns the path as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }

    /// Returns `true` if the path is absolute, i.e., beginning with "/".
    #[inline]
    pub fn is_absolute(&self) -> bool {
        self.as_str().starts_with('/')
    }

    /// Returns `true` if the path is rootless, i.e., not beginning with "/".
    #[inline]
    pub fn is_rootless(&self) -> bool {
        !self.is_absolute()
    }

    /// Returns an iterator over the path [segments].
    ///
    /// [segments]: https://datatracker.ietf.org/doc/html/rfc3986/#section-3.3
    ///
    /// # Examples
    ///
    /// ```
    /// use fluent_uri::Uri;
    ///
    /// // An empty path has no segments.
    /// let uri = Uri::parse("")?;
    /// assert_eq!(uri.path().segments().next(), None);
    ///
    /// let uri = Uri::parse("a/b/c")?;
    /// assert!(uri.path().segments().eq(["a", "b", "c"]));
    ///
    /// // The empty string before a preceding "/" is not a segment.
    /// // However, segments can be empty in the other cases.
    /// let uri = Uri::parse("/path/to//dir/")?;
    /// assert!(uri.path().segments().eq(["path", "to", "", "dir", ""]));
    /// # Ok::<_, fluent_uri::ParseError>(())
    /// ```
    #[inline]
    pub fn segments(&self) -> Split<'_> {
        let mut path = self.inner.as_str();
        if self.is_absolute() {
            // SAFETY: Skipping "/" is fine.
            path = unsafe { path.get_unchecked(1..) };
        }
        // SAFETY: The validation is done.
        let path = unsafe { EStr::new_unchecked(path.as_bytes()) };

        let mut split = path.split('/');
        split.finished = self.as_str().is_empty();
        split
    }
}
