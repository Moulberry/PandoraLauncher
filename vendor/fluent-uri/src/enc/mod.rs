#[cfg(not(feature = "unstable"))]
pub(crate) mod table;
#[cfg(feature = "unstable")]
pub mod table;
#[cfg(feature = "unstable")]
use table::Table;

pub(crate) mod imp;
#[cfg(feature = "unstable")]
pub use imp::{
    decode_in_place_unchecked, decode_unchecked, decode_with_unchecked, EncodingError,
    EncodingErrorKind,
};
#[cfg(feature = "unstable")]
use imp::{err, Result};

#[cfg(feature = "unstable")]
pub mod encoder;

#[cfg(feature = "unstable")]
mod estring;
#[cfg(feature = "unstable")]
pub use estring::*;

/// Percent-encodes a byte sequence.
///
/// # Panics
///
/// Panics if the table does not allow percent-encoding.
#[cfg(feature = "unstable")]
#[inline]
pub fn encode<'a, S: AsRef<[u8]> + ?Sized>(s: &'a S, table: &Table) -> Cow<'a, str> {
    assert!(table.allows_enc(), "table does not allow percent-encoding");
    imp::encode(s.as_ref(), table)
}

/// Percent-encodes a byte sequence to a buffer.
///
/// The buffer may either be a [`String`] or a [`Vec<u8>`].
///
/// # Panics
///
/// Panics if the table does not allow percent-encoding.
#[cfg(feature = "unstable")]
#[inline]
pub fn encode_to<'a, S: AsRef<[u8]> + ?Sized, B: internal::AsMutVec>(
    s: &S,
    table: &Table,
    buf: &'a mut B,
) {
    assert!(table.allows_enc(), "table does not allow percent-encoding");
    // SAFETY: The encoded bytes are valid UTF-8.
    let buf = unsafe { buf.as_mut_vec() };
    imp::encode_to(s.as_ref(), table, buf)
}

/// Decodes a percent-encoded string.
#[cfg(feature = "unstable")]
#[inline]
pub fn decode<S: AsRef<[u8]> + ?Sized>(s: &S) -> Result<Cow<'_, [u8]>> {
    imp::decode(s.as_ref())
}

/// Decodes a percent-encoded string with a buffer.
///
/// If the string needs no decoding, this function returns `Ok(None)`
/// and no bytes will be appended to the buffer.
#[cfg(feature = "unstable")]
#[inline]
pub fn decode_with<'a, S: AsRef<[u8]> + ?Sized>(
    s: &S,
    buf: &'a mut Vec<u8>,
) -> Result<Option<&'a [u8]>> {
    imp::decode_with(s.as_ref(), buf)
}

/// Checks if all characters in a string are allowed by the given table.
#[cfg(feature = "unstable")]
#[inline]
pub fn validate<S: AsRef<[u8]> + ?Sized>(s: &S, table: &Table) -> Result<()> {
    let s = s.as_ref();
    if table.allows_enc() {
        imp::validate_enc(s, table)
    } else {
        match s.iter().position(|&x| !table.allows(x)) {
            Some(i) => err!(i, UnexpectedChar),
            None => Ok(()),
        }
    }
}

use alloc::{
    borrow::{self, Cow},
    string::{FromUtf8Error, String},
    vec::Vec,
};
use core::{
    fmt, hash,
    iter::FusedIterator,
    mem,
    str::{self, Utf8Error},
};

use crate::view::View;

/// Percent-encoded string slices.
///
/// # Examples
///
/// Parse key-value pairs from a query string into a hash map:
///
/// ```
/// use std::collections::HashMap;
/// use fluent_uri::enc::EStr;
///
/// let query = "name=%E5%BC%A0%E4%B8%89&speech=%C2%A1Ol%C3%A9!";
/// let map: HashMap<_, _> = EStr::new(query)
///     .split('&')
///     .filter_map(|pair| pair.split_once('='))
///     .map(|(k, v)| (k.decode(), v.decode()))
///     .filter_map(|(k, v)| k.into_string().ok().zip(v.into_string().ok()))
///     .collect();
/// assert_eq!(map["name"], "张三");
/// assert_eq!(map["speech"], "¡Olé!");
/// ```
#[repr(transparent)]
pub struct EStr {
    inner: [u8],
}

impl AsRef<str> for EStr {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<[u8]> for EStr {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.inner
    }
}

/// Implements equality comparisons on `EStr`s.
///
/// `EStr`s are compared by their byte values. Percent-encoding
/// normalization is **not** performed prior to comparison.
impl PartialEq<EStr> for EStr {
    #[inline]
    fn eq(&self, other: &EStr) -> bool {
        self.inner == other.inner
    }
}

impl PartialEq<str> for EStr {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<EStr> for str {
    #[inline]
    fn eq(&self, other: &EStr) -> bool {
        self == other.as_str()
    }
}

impl Eq for EStr {}

impl fmt::Debug for EStr {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for EStr {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl hash::Hash for EStr {
    #[inline]
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl borrow::Borrow<str> for &EStr {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Default for &EStr {
    /// Creates an empty `EStr`.
    #[inline]
    fn default() -> &'static EStr {
        EStr::EMPTY
    }
}

impl EStr {
    const EMPTY: &'static EStr = EStr::new("");

    /// Converts a string slice to an `EStr`.
    ///
    /// # Panics
    ///
    /// Panics if the string is not properly encoded.
    pub const fn new(s: &str) -> &EStr {
        if imp::validate_estr(s.as_bytes()) {
            // SAFETY: The validation is done.
            unsafe { EStr::new_unchecked(s.as_bytes()) }
        } else {
            panic!("invalid percent-encoded string");
        }
    }

    /// Converts a byte slice into an `EStr` assuming validity.
    #[inline]
    pub(crate) const unsafe fn new_unchecked(s: &[u8]) -> &EStr {
        // SAFETY: The caller must ensure that the bytes are valid percent-encoded UTF-8.
        unsafe { &*(s as *const [u8] as *const EStr) }
    }

    /// Yields the underlying string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        // SAFETY: The validation is done.
        unsafe { str::from_utf8_unchecked(&self.inner) }
    }

    /// Decodes the `EStr`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fluent_uri::enc::EStr;
    ///
    /// let dec = EStr::new("%C2%BF").decode();
    /// assert_eq!(dec.as_bytes(), &[0xc2, 0xbf]);
    /// assert_eq!(dec.into_string()?, "¿");
    /// # Ok::<_, std::string::FromUtf8Error>(())
    /// ```
    #[inline]
    pub fn decode(&self) -> Decode<'_> {
        // SAFETY: `EStr::new_unchecked` ensures that the string is properly encoded.
        match unsafe { imp::decode_unchecked(&self.inner) } {
            Some(s) => Decode::Dst(s),
            None => Decode::Src(self.as_str()),
        }
    }

    /// Decodes the `EStr` with a buffer.
    ///
    /// If the string needs no decoding, no bytes will be appended to the buffer.
    ///
    /// Note that the buffer is not cleared prior to decoding.
    ///
    /// # Examples
    ///
    /// ```
    /// use fluent_uri::enc::EStr;
    ///
    /// let mut buf = Vec::new();
    /// let dec = EStr::new("233").decode_with(&mut buf);
    /// assert_eq!(dec.to_str()?, "233");
    /// assert!(!dec.decoded_any());
    /// assert!(buf.is_empty());
    ///
    /// let dec = EStr::new("2%333").decode_with(&mut buf);
    /// assert_eq!(dec.to_str()?, "233");
    /// assert!(dec.decoded_any());
    /// assert_eq!(buf, b"233");
    /// # Ok::<_, core::str::Utf8Error>(())
    /// ```
    #[cfg(feature = "unstable")]
    #[inline]
    pub fn decode_with<'dst>(&self, buf: &'dst mut Vec<u8>) -> DecodeRef<'_, 'dst> {
        // SAFETY: `EStr::new_unchecked` ensures that the string is properly encoded.
        let decoded = unsafe { decode_with_unchecked(&self.inner, buf) };

        match decoded {
            Some(s) => DecodeRef::Dst(s),
            None => DecodeRef::Src(self),
        }
    }

    /// Returns an iterator over subslices of the `EStr` separated by the given delimiter.
    ///
    /// # Panics
    ///
    /// Panics if the delimiter is not a [reserved] character.
    ///
    /// [reserved]: https://datatracker.ietf.org/doc/html/rfc3986/#section-2.2
    ///
    /// # Examples
    ///
    /// ```
    /// use fluent_uri::enc::EStr;
    ///
    /// assert!(EStr::new("a,b,c").split(',').eq(["a", "b", "c"]));
    /// assert!(EStr::new(",").split(',').eq(["", ""]));
    /// ```
    #[inline]
    pub fn split(&self, delim: char) -> Split<'_> {
        assert!(
            delim.is_ascii() && table::RESERVED.allows(delim as u8),
            "splitting with non-reserved character"
        );

        Split {
            s: &self.inner,
            delim: delim as u8,
            finished: false,
        }
    }

    /// Splits the `EStr` on the first occurrence of the given delimiter and
    /// returns prefix before delimiter and suffix after delimiter.
    ///
    /// Returns `None` if the delimiter is not found.
    ///
    /// # Panics
    ///
    /// Panics if the delimiter is not a [reserved] character.
    ///
    /// [reserved]: https://datatracker.ietf.org/doc/html/rfc3986/#section-2.2
    ///
    /// # Examples
    ///
    /// ```
    /// use fluent_uri::enc::EStr;
    ///
    /// let (k, v) = EStr::new("key=value").split_once('=').unwrap();
    /// assert_eq!(k, "key");
    /// assert_eq!(v, "value");
    ///
    /// assert!(EStr::new("abc").split_once(';').is_none());
    /// ```
    #[inline]
    pub fn split_once(&self, delim: char) -> Option<(&EStr, &EStr)> {
        assert!(
            delim.is_ascii() && table::RESERVED.allows(delim as u8),
            "splitting with non-reserved character"
        );
        let bytes = &self.inner;

        let i = bytes.iter().position(|&x| x == delim as u8)?;
        let (head, tail) = (&bytes[..i], &bytes[i + 1..]);
        // SAFETY: Splitting at a reserved character leaves valid percent-encoded UTF-8.
        unsafe { Some((EStr::new_unchecked(head), EStr::new_unchecked(tail))) }
    }
}

/// An [`EStr`] view into a mutable byte slice that allows in-place percent-decoding.
impl<'a> View<'a, EStr> {
    /// Decodes the `View<EStr>` in-place.
    #[inline]
    pub fn decode_in_place(self) -> DecodeInPlace<'a> {
        let bytes = self.into_bytes();
        // SAFETY: `Self::new` ensures that the bytes are properly encoded.
        let len = unsafe { imp::decode_in_place_unchecked(bytes) };
        if len == bytes.len() {
            // SAFETY: Nothing is decoded so the bytes are valid UTF-8.
            DecodeInPlace::Src(unsafe { View::new(bytes) })
        } else {
            // SAFETY: The length must be less.
            DecodeInPlace::Dst(unsafe { bytes.get_unchecked_mut(..len) })
        }
    }

    /// Returns an iterator over subslices of the `View<EStr>` separated by the given delimiter.
    ///
    /// # Panics
    ///
    /// Panics if the delimiter is not a [reserved] character.
    ///
    /// [reserved]: https://datatracker.ietf.org/doc/html/rfc3986/#section-2.2
    #[inline]
    pub fn split_view(self, delim: char) -> SplitView<'a> {
        assert!(
            delim.is_ascii() && table::RESERVED.allows(delim as u8),
            "splitting with non-reserved character"
        );

        SplitView {
            s: self.into_bytes(),
            delim: delim as u8,
            finished: false,
        }
    }

    /// Splits the `View<EStr>` on the first occurrence of the given delimiter and
    /// returns prefix before delimiter and suffix after delimiter.
    ///
    /// Returns `Err(self)` if the delimiter is not found.
    ///
    /// # Panics
    ///
    /// Panics if the delimiter is not a [reserved] character.
    ///
    /// [reserved]: https://datatracker.ietf.org/doc/html/rfc3986/#section-2.2
    #[inline]
    pub fn split_once_view(self, delim: char) -> Result<(Self, Self), Self> {
        assert!(
            delim.is_ascii() && table::RESERVED.allows(delim as u8),
            "splitting with non-reserved character"
        );

        let i = match self.as_str().bytes().position(|x| x == delim as u8) {
            Some(i) => i,
            None => return Err(self),
        };
        let (head, tail) = self.into_bytes().split_at_mut(i);
        // SAFETY: Splitting at a reserved character leaves valid percent-encoded UTF-8.
        unsafe { Ok((View::new(head), View::new(&mut tail[1..]))) }
    }
}

/// A wrapper of percent-decoded bytes.
///
/// This enum is created by the [`decode`] method on [`EStr`].
///
/// [`decode`]: EStr::decode
#[derive(Clone, Debug)]
pub enum Decode<'a> {
    /// No percent-encoded octets are decoded.
    Src(&'a str),
    /// One or more percent-encoded octets are decoded.
    Dst(Vec<u8>),
}

impl<'a> Decode<'a> {
    /// Returns a reference to the decoded bytes.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Src(s) => s.as_bytes(),
            Self::Dst(vec) => vec,
        }
    }

    /// Consumes this `Decode` and yields the underlying decoded bytes.
    #[inline]
    pub fn into_bytes(self) -> Cow<'a, [u8]> {
        match self {
            Self::Src(s) => Cow::Borrowed(s.as_bytes()),
            Self::Dst(vec) => Cow::Owned(vec),
        }
    }

    /// Returns `true` if anything is decoded.
    #[cfg(feature = "unstable")]
    #[inline]
    pub fn decoded_any(&self) -> bool {
        matches!(self, Self::Dst(_))
    }

    /// Converts the decoded bytes to a string.
    ///
    /// An error is returned if the decoded bytes are not valid UTF-8.
    #[inline]
    pub fn into_string(self) -> Result<Cow<'a, str>, FromUtf8Error> {
        // FIXME: A (maybe) more efficient approach: only validating encoded sequences.
        match self {
            Self::Src(s) => Ok(Cow::Borrowed(s)),
            Self::Dst(vec) => String::from_utf8(vec).map(Cow::Owned),
        }
    }

    /// Converts the decoded bytes to a string lossily.
    pub fn into_string_lossy(self) -> Cow<'a, str> {
        match self {
            Self::Src(s) => Cow::Borrowed(s),
            Self::Dst(vec) => Cow::Owned(match String::from_utf8_lossy(&vec) {
                // SAFETY: If a borrowed string slice is returned, the bytes must be valid UTF-8.
                Cow::Borrowed(_) => unsafe { String::from_utf8_unchecked(vec) },
                Cow::Owned(string) => string,
            }),
        }
    }
}

/// A wrapper of borrowed percent-decoded bytes.
///
/// This enum is created by the [`decode_with`] method on [`EStr`].
///
/// [`decode_with`]: EStr::decode_with
#[cfg(feature = "unstable")]
#[derive(Clone, Copy, Debug)]
pub enum DecodeRef<'src, 'dst> {
    /// Nothing decoded, i.e., borrowed from the source.
    Src(&'src EStr),
    /// Something decoded, i.e., borrowed from the buffer.
    Dst(&'dst [u8]),
}

#[cfg(feature = "unstable")]
impl<'src, 'dst> DecodeRef<'src, 'dst> {
    /// Returns a reference to the decoded bytes.
    #[inline]
    pub fn as_bytes<'a>(&self) -> &'a [u8]
    where
        'src: 'a,
        'dst: 'a,
    {
        match *self {
            Self::Src(s) => s.as_str().as_bytes(),
            Self::Dst(s) => s,
        }
    }

    /// Returns `true` if anything is decoded.
    #[inline]
    pub fn decoded_any(&self) -> bool {
        matches!(self, Self::Dst(_))
    }

    /// Converts the decoded bytes to a string slice.
    ///
    /// An error is returned if the decoded bytes are not valid UTF-8.
    #[inline]
    pub fn to_str<'a>(&self) -> Result<&'a str, Utf8Error>
    where
        'src: 'a,
        'dst: 'a,
    {
        match *self {
            Self::Src(s) => Ok(s.as_str()),
            Self::Dst(s) => str::from_utf8(s),
        }
    }

    /// Converts the decoded bytes to a string lossily.
    #[inline]
    pub fn to_string_lossy<'a>(&self) -> Cow<'a, str>
    where
        'src: 'a,
        'dst: 'a,
    {
        match *self {
            Self::Src(s) => Cow::Borrowed(s.as_str()),
            Self::Dst(s) => String::from_utf8_lossy(s),
        }
    }
}

/// A wrapper of in-place percent-decoded bytes.
///
/// This enum is created by the [`decode_in_place`] method on [`View<EStr>`].
///
/// [`decode_in_place`]: View::<EStr>::decode_in_place
#[derive(Debug)]
pub enum DecodeInPlace<'a> {
    /// No percent-encoded octets are decoded.
    Src(View<'a, str>),
    /// One or more percent-encoded octets are decoded.
    Dst(&'a mut [u8]),
}

impl<'a> DecodeInPlace<'a> {
    /// Returns a reference to the decoded bytes.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Src(s) => s.as_bytes(),
            Self::Dst(s) => s,
        }
    }

    /// Consumes this `DecodeInPlace` and yields the underlying decoded bytes.
    #[inline]
    pub fn into_bytes(self) -> &'a mut [u8] {
        match self {
            Self::Src(s) => s.into_bytes(),
            Self::Dst(s) => s,
        }
    }

    /// Returns `true` if anything is decoded.
    #[cfg(feature = "unstable")]
    #[inline]
    pub fn decoded_any(&self) -> bool {
        matches!(self, Self::Dst(_))
    }

    /// Converts the decoded bytes to a [`View<str>`].
    ///
    /// An error along with the decoded bytes is returned if the bytes are not valid UTF-8.
    #[inline]
    pub fn into_str_view(self) -> Result<View<'a, str>, (&'a mut [u8], Utf8Error)> {
        match self {
            Self::Src(s) => Ok(s),
            Self::Dst(s) => match str::from_utf8(s) {
                // SAFETY: The validation is done.
                Ok(_) => Ok(unsafe { View::new(s) }),
                Err(e) => Err((s, e)),
            },
        }
    }

    /// Converts the decoded bytes to a string slice.
    ///
    /// An error along with the decoded bytes is returned if the bytes are not valid UTF-8.
    #[inline]
    pub fn into_str(self) -> Result<&'a str, (&'a mut [u8], Utf8Error)> {
        self.into_str_view().map(View::into_ref)
    }

    /// Converts the decoded bytes to a string lossily.
    #[inline]
    pub fn into_string_lossy(self) -> Cow<'a, str> {
        match self {
            Self::Src(s) => Cow::Borrowed(s.into_ref()),
            Self::Dst(s) => String::from_utf8_lossy(s),
        }
    }
}

/// An iterator over subslices of an [`EStr`] separated by a delimiter.
///
/// This struct is created by the [`split`] method on [`EStr`].
///
/// [`split`]: EStr::split
#[derive(Clone, Debug)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Split<'a> {
    s: &'a [u8],
    delim: u8,
    pub(crate) finished: bool,
}

impl<'a> Iterator for Split<'a> {
    type Item = &'a EStr;

    #[inline]
    fn next(&mut self) -> Option<&'a EStr> {
        if self.finished {
            return None;
        }

        let head;
        match self.s.iter().position(|&x| x == self.delim) {
            Some(i) => {
                head = &self.s[..i];
                self.s = &self.s[i + 1..];
            }
            None => {
                self.finished = true;
                head = self.s;
            }
        }
        // SAFETY: Splitting at a reserved character leaves valid percent-encoded UTF-8.
        Some(unsafe { EStr::new_unchecked(head) })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.finished {
            (0, Some(0))
        } else {
            (1, Some(self.s.len() + 1))
        }
    }
}

impl<'a> DoubleEndedIterator for Split<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a EStr> {
        if self.finished {
            return None;
        }

        let tail;
        match self.s.iter().rposition(|&x| x == self.delim) {
            Some(i) => {
                tail = &self.s[i + 1..];
                self.s = &self.s[..i];
            }
            None => {
                self.finished = true;
                tail = self.s;
            }
        }
        // SAFETY: Splitting at a reserved character leaves valid percent-encoded UTF-8.
        Some(unsafe { EStr::new_unchecked(tail) })
    }
}

impl FusedIterator for Split<'_> {}

/// An iterator over subslices of a [`View<EStr>`] separated by a delimiter.
///
/// This struct is created by the [`split_view`] method on [`View<EStr>`].
///
/// [`split_view`]: View::<EStr>::split_view
#[derive(Debug)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct SplitView<'a> {
    s: &'a mut [u8],
    delim: u8,
    pub(crate) finished: bool,
}

impl<'a> Iterator for SplitView<'a> {
    type Item = View<'a, EStr>;

    #[inline]
    fn next(&mut self) -> Option<View<'a, EStr>> {
        if self.finished {
            return None;
        }

        let head = match self.s.iter().position(|&x| x == self.delim) {
            Some(i) => {
                let tmp = mem::take(&mut self.s);
                let (head, tail) = tmp.split_at_mut(i);
                self.s = &mut tail[1..];
                head
            }
            None => {
                self.finished = true;
                mem::take(&mut self.s)
            }
        };
        // SAFETY: Splitting at a reserved character leaves valid percent-encoded UTF-8.
        Some(unsafe { View::new(head) })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.finished {
            (0, Some(0))
        } else {
            (1, Some(self.s.len() + 1))
        }
    }
}

impl<'a> DoubleEndedIterator for SplitView<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<View<'a, EStr>> {
        if self.finished {
            return None;
        }

        let tail = match self.s.iter().rposition(|&x| x == self.delim) {
            Some(i) => {
                let tmp = mem::take(&mut self.s);
                let (head, tail) = tmp.split_at_mut(i);
                self.s = head;
                &mut tail[1..]
            }
            None => {
                self.finished = true;
                mem::take(&mut self.s)
            }
        };
        // SAFETY: Splitting at a reserved character leaves valid percent-encoded UTF-8.
        Some(unsafe { View::new(tail) })
    }
}

impl FusedIterator for SplitView<'_> {}

/// An error occurred when attempting to write to a buffer that is too small.
///
/// This error is created by the [`to_mut_in`] method on [`Uri`].
// FIXME: Add `EStr::decode_with` when it is stabilized.
///
/// [`to_mut_in`]: crate::Uri::to_mut_in
/// [`Uri`]: crate::Uri
#[cfg(feature = "unstable")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferTooSmallError(());

#[cfg(all(feature = "unstable", feature = "std"))]
impl std::error::Error for BufferTooSmallError {}

#[cfg(feature = "unstable")]
impl fmt::Display for BufferTooSmallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "buffer too small")
    }
}

#[cfg(feature = "unstable")]
pub(crate) mod internal {
    use crate::enc::BufferTooSmallError;
    use alloc::{collections::TryReserveError, string::String, vec::Vec};
    use core::mem::MaybeUninit;

    pub trait AsMutVec {
        unsafe fn as_mut_vec(&mut self) -> &mut Vec<u8>;
    }

    impl AsMutVec for Vec<u8> {
        #[inline]
        unsafe fn as_mut_vec(&mut self) -> &mut Vec<u8> {
            self
        }
    }

    impl AsMutVec for String {
        #[inline]
        unsafe fn as_mut_vec(&mut self) -> &mut Vec<u8> {
            // SAFETY: The caller must not mess up the string.
            unsafe { self.as_mut_vec() }
        }
    }

    pub trait Buf {
        type PrepareError;

        fn prepare(&mut self, len: usize) -> Result<*mut u8, Self::PrepareError>;

        unsafe fn finish(&mut self, len: usize);
    }

    impl Buf for Vec<u8> {
        type PrepareError = TryReserveError;

        #[inline]
        fn prepare(&mut self, len: usize) -> Result<*mut u8, TryReserveError> {
            self.try_reserve(len)?;
            Ok(self.as_mut_ptr_range().end)
        }

        #[inline]
        unsafe fn finish(&mut self, len: usize) {
            // SAFETY: The caller must ensure that the additional `len` bytes are initialized.
            unsafe { self.set_len(self.len() + len) }
        }
    }

    impl Buf for [u8] {
        type PrepareError = BufferTooSmallError;

        #[inline]
        fn prepare(&mut self, len: usize) -> Result<*mut u8, BufferTooSmallError> {
            if self.len() < len {
                Err(BufferTooSmallError(()))
            } else {
                Ok(self.as_mut_ptr())
            }
        }

        #[inline]
        unsafe fn finish(&mut self, _len: usize) {}
    }

    impl Buf for [MaybeUninit<u8>] {
        type PrepareError = BufferTooSmallError;

        #[inline]
        fn prepare(&mut self, len: usize) -> Result<*mut u8, BufferTooSmallError> {
            if self.len() < len {
                Err(BufferTooSmallError(()))
            } else {
                Ok(self.as_mut_ptr().cast())
            }
        }

        #[inline]
        unsafe fn finish(&mut self, _len: usize) {}
    }
}
