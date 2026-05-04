//! Fast, FFI-friendly string interning. A [`Ustr`] (**U**nique **Str**) is a
//! lightweight handle representing a static, immutable entry in a global string
//! cache, allowing for:
//!
//! * Extremely fast string assignment and comparisons -- it's just a pointer
//!   comparison.
//!
//! * Efficient storage -- only one copy of the string is held in memory, and
//!   getting access to it is just a pointer indirection.
//!
//! * Fast hashing -- the precomputed hash is stored with the string.
//!
//! * Fast FFI -- the string is stored with a terminating null byte so can be
//!   passed to C directly without doing the `CString` dance.
//!
//! The downside is no strings are ever freed, so if you're creating lots and
//! lots of strings, you might run out of memory. On the other hand, War and
//! Peace is only 3MB, so it's probably fine.
//!
//! This crate is based on [OpenImageIO's](https://openimageio.readthedocs.io/en/v2.4.10.0/)
//! (OIIO) [`ustring`](https://github.com/OpenImageIO/oiio/blob/master/src/include/OpenImageIO/ustring.h)
//! but it is *not* binary-compatible (yet). The underlying hash map
//! implementation is directy ported from OIIO.
//!
//! # Usage
//!
//! ```
//! use ustr::{Ustr, ustr, ustr as u};
//!
//! # unsafe { ustr::_clear_cache() };
//! // Creation is quick and easy using either `Ustr::from` or the ustr function
//! // and only one copy of any string is stored.
//! let u1 = Ustr::from("the quick brown fox");
//! let u2 = ustr("the quick brown fox");
//!
//! // Comparisons and copies are extremely cheap.
//! let u3 = u1;
//! assert_eq!(u2, u3);
//!
//! // You can pass straight to FFI.
//! let len = unsafe {
//!     libc::strlen(u1.as_char_ptr())
//! };
//! assert_eq!(len, 19);
//!
//! // Use as_str() to get a `str`.
//! let words: Vec<&str> = u1.as_str().split_whitespace().collect();
//! assert_eq!(words, ["the", "quick", "brown", "fox"]);
//!
//! // For best performance when using Ustr as key for a HashMap or HashSet,
//! // you'll want to use the precomputed hash. To make this easier, just use
//! // the UstrMap and UstrSet exports:
//! use ustr::UstrMap;
//!
//! // Key type is always `Ustr`.
//! let mut map: UstrMap<usize> = UstrMap::default();
//! map.insert(u1, 17);
//! assert_eq!(*map.get(&u1).unwrap(), 17);
//! ```
//!
//! By enabling the `"serde"` feature you can serialize individual `Ustr`s
//! or the whole cache with serde.
//!
//! ```
//! # #[cfg(feature = "serde")] {
//! use ustr::{Ustr, ustr};
//! let u_ser = ustr("serde");
//! let json = serde_json::to_string(&u_ser).unwrap();
//! let u_de : Ustr = serde_json::from_str(&json).unwrap();
//! assert_eq!(u_ser, u_de);
//! # }
//! ```
//!
//! Since the cache is global, use the `ustr::DeserializedCache` dummy object to
//! drive the deserialization.
//!
//! ```
//! # #[cfg(feature = "serde")] {
//! use ustr::{Ustr, ustr};
//! ustr("Send me to JSON and back");
//! let json = serde_json::to_string(ustr::cache()).unwrap();
//!
//! // ... some time later ...
//! let _: ustr::DeserializedCache = serde_json::from_str(&json).unwrap();
//! assert_eq!(ustr::num_entries(), 1);
//! assert_eq!(ustr::string_cache_iter().collect::<Vec<_>>(), vec!["Send me to JSON and back"]);
//! # }
//! ```
//!
//! ## Why?
//!
//! It is common in certain types of applications to use strings as identifiers,
//! but not really do any processing with them.
//! To paraphrase from OIIO's `Ustring` documentation -- compared to standard
//! strings, `Ustr`s have several advantages:
//!
//!   - Each individual `Ustr` is very small -- in fact, we guarantee that a
//!     `Ustr` is the same size and memory layout as an ordinary `*u8`.
//!
//!   - Storage is frugal, since there is only one allocated copy of each unique
//!     character sequence, throughout the lifetime of the program.
//!
//!   - Assignment from one `Ustr` to another is just copy of the pointer; no
//!     allocation, no character copying, no reference counting.
//!
//!   - Equality testing (do the strings contain the same characters) is a
//!     single operation, the comparison of the pointer.
//!
//!   - Memory allocation only occurs when a new `Ustr` is constructed from raw
//!     characters the FIRST time -- subsequent constructions of the same string
//!     just finds it in the canonial string set, but doesn't need to allocate
//!     new storage.  Destruction of a `Ustr` is trivial, there is no
//!     de-allocation because the canonical version stays in the set.  Also,
//!     therefore, no user code mistake can lead to memory leaks.
//!
//! But there are some problems, too.  Canonical strings are never freed
//! from the table.  So in some sense all the strings "leak", but they
//! only leak one copy for each unique string that the program ever comes
//! across.
//!
//! On the whole, `Ustr`s are a really great string representation
//!
//!   - if you tend to have (relatively) few unique strings, but many copies of
//!     those strings;
//!
//!   - if the creation of strings from raw characters is relatively rare
//!     compared to copying or comparing to existing strings;
//!
//!   - if you tend to make the same strings over and over again, and if it's
//!     relatively rare that a single unique character sequence is used only
//!     once in the entire lifetime of the program;
//!
//!   - if your most common string operations are assignment and equality
//!     testing and you want them to be as fast as possible;
//!
//!   - if you are doing relatively little character-by-character assembly of
//!     strings, string concatenation, or other "string manipulation" (other
//!     than equality testing).
//!
//! `Ustr`s are not so hot
//!
//!   - if your program tends to have very few copies of each character sequence
//!     over the entire lifetime of the program;
//!
//!   - if your program tends to generate a huge variety of unique strings over
//!     its lifetime, each of which is used only a short time and then
//!     discarded, never to be needed again;
//!
//!   - if you don't need to do a lot of string assignment or equality testing,
//!     but lots of more complex string manipulation.
//!
//! ## Safety and Compatibility
//!
//! This crate contains a significant amount of unsafe but usage has been
//! checked and is well-documented. It is also run through Miri as part of the
//! CI process. I use it regularly on 64-bit systems, and it has passed Miri on
//! a 32-bit system as well, bit 32-bit is not checked regularly. If you want to
//! use it on 32-bit, please make sure to run Miri and open and issue if you
//! find any problems.
use parking_lot::Mutex;
use std::{
    borrow::Cow,
    cmp::Ordering,
    ffi::{CStr, OsStr},
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
    os::raw::c_char,
    path::Path,
    ptr::NonNull,
    rc::Rc,
    slice, str,
    str::FromStr,
    sync::Arc,
};

mod hash;
pub use hash::*;
mod bumpalloc;

mod stringcache;
pub use stringcache::*;
#[cfg(feature = "serde")]
pub mod serialization;
#[cfg(feature = "serde")]
pub use serialization::DeserializedCache;

/// A handle representing a string in the global string cache.
///
/// To use, create one using [`Ustr::from`] or the [`ustr`] function. You can
/// freely copy, destroy or send `Ustr`s to other threads: the underlying string
/// is always valid in memory (and is never destroyed).
#[derive(Copy, Clone, PartialEq)]
#[repr(transparent)]
pub struct Ustr {
    char_ptr: NonNull<u8>,
}

/// Defer to `str` for equality.
///
/// Lexicographic ordering will be slower than pointer comparison, but much less
/// surprising if you use `Ustr`s as keys in e.g. a `BTreeMap`.
impl Ord for Ustr {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

/// Defer to `str` for equality.
///
/// Lexicographic ordering will be slower thanpointer comparison, but much less
/// surprising if you use `Ustr`s as keys in e.g. a `BTreeMap`.
#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for Ustr {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ustr {
    /// Create a new `Ustr` from the given `str`.
    ///
    /// You can also use the [`ustr`] function.
    ///
    /// # Examples
    ///
    /// ```
    /// use ustr::{Ustr, ustr as u};
    /// # unsafe { ustr::_clear_cache() };
    ///
    /// let u1 = Ustr::from("the quick brown fox");
    /// let u2 = u("the quick brown fox");
    /// assert_eq!(u1, u2);
    /// assert_eq!(ustr::num_entries(), 1);
    /// ```
    pub fn from(string: &str) -> Ustr {
        let hash = {
            let mut hasher = ahash::AHasher::default();
            hasher.write(string.as_bytes());
            hasher.finish()
        };
        let mut sc = STRING_CACHE.0[whichbin(hash)].lock();
        Ustr {
            // SAFETY: sc.insert does not give back a null pointer
            char_ptr: unsafe {
                NonNull::new_unchecked(sc.insert(string, hash) as *mut _)
            },
        }
    }

    pub fn from_existing(string: &str) -> Option<Ustr> {
        let hash = {
            let mut hasher = ahash::AHasher::default();
            hasher.write(string.as_bytes());
            hasher.finish()
        };
        let sc = STRING_CACHE.0[whichbin(hash)].lock();
        sc.get_existing(string, hash).map(|ptr| Ustr {
            char_ptr: unsafe { NonNull::new_unchecked(ptr as *mut _) },
        })
    }

    /// Get the cached `Ustr` as a `str`.
    ///
    /// # Examples
    ///
    /// ```
    /// use ustr::ustr as u;
    /// # unsafe { ustr::_clear_cache() };
    ///
    /// let u_fox = u("the quick brown fox");
    /// let words: Vec<&str> = u_fox.as_str().split_whitespace().collect();
    /// assert_eq!(words, ["the", "quick", "brown", "fox"]);
    /// ```
    pub fn as_str(&self) -> &'static str {
        // This is safe if:
        // 1) self.char_ptr points to a valid address
        // 2) len is a usize stored usize aligned usize bytes before char_ptr
        // 3) char_ptr points to a valid UTF-8 string of len bytes.
        // All these are guaranteed by StringCache::insert() and by the fact
        // we can only construct a Ustr from a valid &str.
        unsafe {
            str::from_utf8_unchecked(slice::from_raw_parts(
                self.char_ptr.as_ptr(),
                self.len(),
            ))
        }
    }

    /// Get the cached string as a C `char*`.
    ///
    /// This includes the null terminator so is safe to pass straight to FFI.
    ///
    /// # Examples
    ///
    /// ```
    /// use ustr::ustr as u;
    /// # unsafe { ustr::_clear_cache() };
    ///
    /// let u_fox = u("the quick brown fox");
    /// let len = unsafe {
    ///     libc::strlen(u_fox.as_char_ptr())
    /// };
    /// assert_eq!(len, 19);
    /// ```
    ///
    /// # Safety
    ///
    /// This is just passing a raw byte array with a null terminator to C. If
    /// your source string contains non-ascii bytes then this will pass them
    /// straight along with no checking.
    ///
    /// The string is **immutable**. That means that if you modify it across the
    /// FFI boundary then all sorts of terrible things will happen.
    pub fn as_char_ptr(&self) -> *const c_char {
        self.char_ptr.as_ptr() as *const c_char
    }

    /// Get this `Ustr` as a [`CStr`]
    ///
    /// This is useful for passing to APIs (like ash) that use `CStr`.
    ///
    /// # Safety
    ///
    /// This function by itself is safe as the pointer and length are guaranteed
    /// to be valid. All the same caveats for the use of the `CStr` as given in
    /// the `CStr` docs apply.
    pub fn as_cstr(&self) -> &CStr {
        unsafe {
            CStr::from_bytes_with_nul_unchecked(slice::from_raw_parts(
                self.as_ptr(),
                self.len() + 1,
            ))
        }
    }

    /// Get a raw pointer to the `StringCacheEntry`.
    #[inline]
    fn as_string_cache_entry(&self) -> &StringCacheEntry {
        // The allocator guarantees that the alignment is correct and that
        // this pointer is non-null
        unsafe { &*(self.char_ptr.as_ptr().cast::<StringCacheEntry>().sub(1)) }
    }

    /// Get the length (in bytes) of this string.
    #[inline]
    pub fn len(&self) -> usize {
        self.as_string_cache_entry().len
    }

    /// Returns true if the length is zero.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the precomputed hash for this string.
    #[inline]
    pub fn precomputed_hash(&self) -> u64 {
        self.as_string_cache_entry().hash
    }

    /// Get an owned String copy of this string.
    pub fn to_owned(&self) -> String {
        self.as_str().to_owned()
    }
}

// We're safe to impl these because the strings they reference are immutable
// and for all intents and purposes 'static since they're never deleted after
// being created
unsafe impl Send for Ustr {}
unsafe impl Sync for Ustr {}

impl PartialEq<str> for Ustr {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<Ustr> for str {
    fn eq(&self, u: &Ustr) -> bool {
        self == u.as_str()
    }
}

impl PartialEq<&str> for Ustr {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<Ustr> for &str {
    fn eq(&self, u: &Ustr) -> bool {
        *self == u.as_str()
    }
}

impl PartialEq<&&str> for Ustr {
    fn eq(&self, other: &&&str) -> bool {
        self.as_str() == **other
    }
}

impl PartialEq<Ustr> for &&str {
    fn eq(&self, u: &Ustr) -> bool {
        **self == u.as_str()
    }
}

impl PartialEq<String> for Ustr {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<Ustr> for String {
    fn eq(&self, u: &Ustr) -> bool {
        self == u.as_str()
    }
}

impl PartialEq<&String> for Ustr {
    fn eq(&self, other: &&String) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<Ustr> for &String {
    fn eq(&self, u: &Ustr) -> bool {
        *self == u.as_str()
    }
}

impl PartialEq<Box<str>> for Ustr {
    fn eq(&self, other: &Box<str>) -> bool {
        self.as_str() == &**other
    }
}

impl PartialEq<Ustr> for Box<str> {
    fn eq(&self, u: &Ustr) -> bool {
        &**self == u.as_str()
    }
}

impl PartialEq<Ustr> for &Box<str> {
    fn eq(&self, u: &Ustr) -> bool {
        &***self == u.as_str()
    }
}

impl PartialEq<Cow<'_, str>> for Ustr {
    fn eq(&self, other: &Cow<'_, str>) -> bool {
        self.as_str() == &*other
    }
}

impl PartialEq<Ustr> for Cow<'_, str> {
    fn eq(&self, u: &Ustr) -> bool {
        &*self == u.as_str()
    }
}

impl PartialEq<&Cow<'_, str>> for Ustr {
    fn eq(&self, other: &&Cow<'_, str>) -> bool {
        self.as_str() == &**other
    }
}

impl PartialEq<Ustr> for &Cow<'_, str> {
    fn eq(&self, u: &Ustr) -> bool {
        &**self == u.as_str()
    }
}

impl PartialEq<Ustr> for Path {
    fn eq(&self, u: &Ustr) -> bool {
        self == Path::new(u)
    }
}

impl PartialEq<Ustr> for &Path {
    fn eq(&self, u: &Ustr) -> bool {
        *self == Path::new(u)
    }
}

impl PartialEq<Ustr> for OsStr {
    fn eq(&self, u: &Ustr) -> bool {
        self == OsStr::new(u)
    }
}

impl PartialEq<Ustr> for &OsStr {
    fn eq(&self, u: &Ustr) -> bool {
        *self == OsStr::new(u)
    }
}

impl Eq for Ustr {}

impl<T: ?Sized> AsRef<T> for Ustr
where
    str: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.as_str().as_ref()
    }
}

impl FromStr for Ustr {
    type Err = std::string::ParseError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Ustr::from(s))
    }
}

impl From<&str> for Ustr {
    fn from(s: &str) -> Ustr {
        Ustr::from(s)
    }
}

impl From<Ustr> for &'static str {
    fn from(s: Ustr) -> &'static str {
        s.as_str()
    }
}

impl From<Ustr> for String {
    fn from(u: Ustr) -> Self {
        String::from(u.as_str())
    }
}

impl From<Ustr> for Box<str> {
    fn from(u: Ustr) -> Self {
        Box::from(u.as_str())
    }
}

impl From<Ustr> for Rc<str> {
    fn from(u: Ustr) -> Self {
        Rc::from(u.as_str())
    }
}

impl From<Ustr> for Arc<str> {
    fn from(u: Ustr) -> Self {
        Arc::from(u.as_str())
    }
}

impl From<Ustr> for Cow<'static, str> {
    fn from(u: Ustr) -> Self {
        Cow::Borrowed(u.as_str())
    }
}

impl From<String> for Ustr {
    fn from(s: String) -> Ustr {
        Ustr::from(&s)
    }
}

impl From<&String> for Ustr {
    fn from(s: &String) -> Ustr {
        Ustr::from(&**s)
    }
}

impl From<Box<str>> for Ustr {
    fn from(s: Box<str>) -> Ustr {
        Ustr::from(&*s)
    }
}

impl From<Rc<str>> for Ustr {
    fn from(s: Rc<str>) -> Ustr {
        Ustr::from(&*s)
    }
}

impl From<Arc<str>> for Ustr {
    fn from(s: Arc<str>) -> Ustr {
        Ustr::from(&*s)
    }
}

impl From<Cow<'_, str>> for Ustr {
    fn from(s: Cow<'_, str>) -> Ustr {
        Ustr::from(&*s)
    }
}

impl Default for Ustr {
    fn default() -> Self {
        Ustr::from("")
    }
}

impl Deref for Ustr {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for Ustr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl fmt::Debug for Ustr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "u!({:?})", self.as_str())
    }
}

// Just feed the precomputed hash into the Hasher. Note that this will of course
// be terrible unless the Hasher in question is expecting a precomputed hash.
impl Hash for Ustr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.precomputed_hash().hash(state);
    }
}

/// DO NOT CALL THIS.
///
/// Clears the cache -- used for benchmarking and testing purposes to clear the
/// cache. Calling this will invalidate any previously created `UStr`s and
/// probably cause your house to burn down. DO NOT CALL THIS.
///
/// # Safety
///
/// DO NOT CALL THIS.
#[doc(hidden)]
pub unsafe fn _clear_cache() {
    for m in STRING_CACHE.0.iter() {
        m.lock().clear();
    }
}

/// Returns the total amount of memory allocated and in use by the cache in
/// bytes.
pub fn total_allocated() -> usize {
    STRING_CACHE
        .0
        .iter()
        .map(|sc| {
            let t = sc.lock().total_allocated();

            t
        })
        .sum()
}

/// Returns the total amount of memory reserved by the cache in bytes.
pub fn total_capacity() -> usize {
    STRING_CACHE
        .0
        .iter()
        .map(|sc| {
            let t = sc.lock().total_capacity();
            t
        })
        .sum()
}

/// Create a new `Ustr` from the given `str`.
///
/// # Examples
///
/// ```
/// use ustr::ustr;
/// # unsafe { ustr::_clear_cache() };
///
/// let u1 = ustr("the quick brown fox");
/// let u2 = ustr("the quick brown fox");
/// assert_eq!(u1, u2);
/// assert_eq!(ustr::num_entries(), 1);
/// ```
#[inline]
pub fn ustr(s: &str) -> Ustr {
    Ustr::from(s)
}

/// Create a new `Ustr` from the given `str` but only if it already exists in
/// the string cache.
///
/// # Examples
///
/// ```
/// use ustr::{ustr, existing_ustr};
/// # unsafe { ustr::_clear_cache() };
///
/// let u1 = existing_ustr("the quick brown fox");
/// let u2 = ustr("the quick brown fox");
/// let u3 = existing_ustr("the quick brown fox");
/// assert_eq!(u1, None);
/// assert_eq!(u3, Some(u2));
/// ```
#[inline]
pub fn existing_ustr(s: &str) -> Option<Ustr> {
    Ustr::from_existing(s)
}

/// Utility function to get a reference to the main cache object for use with
/// serialization.
///
/// # Examples
///
/// ```
/// # use ustr::{Ustr, ustr, ustr as u};
/// # #[cfg(feature="serde")]
/// # {
/// # unsafe { ustr::_clear_cache() };
/// ustr("Send me to JSON and back");
/// let json = serde_json::to_string(ustr::cache()).unwrap();
/// # }
pub fn cache() -> &'static Bins {
    &STRING_CACHE
}

/// Returns the number of unique strings in the cache.
///
/// This may be an underestimate if other threads are writing to the cache
/// concurrently.
///
/// # Examples
///
/// ```
/// use ustr::ustr as u;
///
/// let _ = u("Hello");
/// let _ = u(", World!");
/// assert_eq!(ustr::num_entries(), 2);
/// ```
pub fn num_entries() -> usize {
    STRING_CACHE
        .0
        .iter()
        .map(|sc| {
            let t = sc.lock().num_entries();
            t
        })
        .sum()
}

#[doc(hidden)]
pub fn num_entries_per_bin() -> Vec<usize> {
    STRING_CACHE
        .0
        .iter()
        .map(|sc| {
            let t = sc.lock().num_entries();
            t
        })
        .collect::<Vec<_>>()
}

/// Return an iterator over the entire string cache.
///
/// If another thread is adding strings concurrently to this call then they
/// might not show up in the view of the cache presented by this iterator.
///
/// # Safety
///
/// This returns an iterator to the state of the cache at the time when
/// `string_cache_iter()` was called. It is of course possible that another
/// thread will add more strings to the cache after this, but since we never
/// destroy the strings, they remain valid, meaning it's safe to iterate over
/// them, the list just might not be completely up to date.
pub fn string_cache_iter() -> StringCacheIterator {
    let mut allocs = Vec::new();
    for m in STRING_CACHE.0.iter() {
        let sc = m.lock();
        // the start of the allocator's data is actually the ptr, start() just
        // points to the beginning of the allocated region. The first bytes will
        // be uninitialized since we're bumping down
        for a in &sc.old_allocs {
            allocs.push((a.ptr(), a.end()));
        }
        let ptr = sc.alloc.ptr();
        let end = sc.alloc.end();
        if ptr != end {
            allocs.push((sc.alloc.ptr(), sc.alloc.end()));
        }
    }

    let current_ptr =
        allocs.first().map(|s| s.0).unwrap_or_else(std::ptr::null);

    StringCacheIterator {
        allocs,
        current_alloc: 0,
        current_ptr,
    }
}

/// The type used for the global string cache.
///
/// This is exposed to allow e.g. serialization of the data returned by the
/// [`cache()`] function.
#[repr(transparent)]
pub struct Bins(pub(crate) [Mutex<StringCache>; NUM_BINS]);

#[cfg(test)]
lazy_static::lazy_static! {
    static ref TEST_LOCK: Mutex<()> = Mutex::new(());
}

#[cfg(test)]
mod tests {
    use super::TEST_LOCK;
    use lazy_static::lazy_static;
    use std::ffi::OsStr;
    use std::path::Path;
    use std::sync::Mutex;

    #[test]
    fn it_works() {
        let _t = TEST_LOCK.lock();
        use super::ustr as u;

        let u_hello = u("hello");
        assert_eq!(u_hello, "hello");
        let u_world = u("world");
        assert_eq!(u_world, String::from("world"));
    }

    #[test]
    fn empty_string() {
        let _t = TEST_LOCK.lock();
        use super::ustr as u;

        unsafe {
            super::_clear_cache();
        }

        let _empty = u("");
        let empty = u("");

        assert!(empty.as_str().is_empty());
        assert_eq!(super::num_entries(), 1);
    }

    #[test]
    fn c_str_works() {
        let _t = TEST_LOCK.lock();
        use super::ustr as u;
        use std::ffi::CStr;

        let s_fox = "The quick brown fox jumps over the lazy dog.";
        let u_fox = u(s_fox);
        let fox = unsafe { CStr::from_ptr(u_fox.as_char_ptr()) }
            .to_string_lossy()
            .into_owned();
        assert_eq!(fox, s_fox);

        let s_odys = "Τη γλώσσα μου έδωσαν ελληνική";
        let u_odys = u(s_odys);
        let odys = unsafe { CStr::from_ptr(u_odys.as_char_ptr()) }
            .to_string_lossy()
            .into_owned();
        assert_eq!(odys, s_odys);
    }

    #[test]
    // We have to disable miri here as it's far too slow unfortunately
    #[cfg_attr(miri, ignore)]
    fn blns() {
        let _t = TEST_LOCK.lock();
        use super::{string_cache_iter, ustr as u};
        use std::collections::HashSet;

        // clear the cache first or our results will be wrong
        unsafe { super::_clear_cache() };

        // let path =
        // std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        //     .join("data")
        //     .join("blns.txt");
        // let blns = std::fs::read_to_string(path).unwrap();
        let blns = include_str!("../data/blns.txt");

        let mut hs = HashSet::new();
        for s in blns.split_whitespace() {
            hs.insert(s);
        }

        let mut us = Vec::new();
        let mut ss = Vec::new();

        for s in blns.split_whitespace().cycle().take(100_000) {
            let u = u(s);
            us.push(u);
            ss.push(s.to_owned());
        }

        let mut hs_u = HashSet::new();
        for s in string_cache_iter() {
            hs_u.insert(s);
        }
        let diff: HashSet<_> = hs.difference(&hs_u).collect();

        // check that the number of entries is the same
        assert_eq!(super::num_entries(), hs.len());

        // check that we have the exact same (unique) strings in the cache as in
        // the source data
        assert_eq!(diff.len(), 0);

        let nbs = super::num_entries_per_bin();
        println!("{:?}", nbs);

        println!("Total allocated: {}", super::total_allocated());
        println!("Total capacity: {}", super::total_capacity());

        println!(
            "size of StringCache: {}",
            std::mem::size_of::<super::StringCache>()
        );
    }

    #[test]
    // We have to disable miri here as it's far too slow unfortunately
    #[cfg_attr(miri, ignore)]
    fn raft() {
        let _t = TEST_LOCK.lock();
        use super::ustr as u;
        use std::sync::Arc;

        // let path =
        // std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        //     .join("data")
        //     .join("raft-large-directories.txt");
        // let raft = std::fs::read_to_string(path).unwrap();
        let raft = include_str!("../data/raft-large-directories.txt");
        let raft = Arc::new(
            raft.split_whitespace()
                .collect::<Vec<_>>()
                .chunks(3)
                .map(|s| {
                    if s.len() == 3 {
                        format!("{}/{}/{}", s[0], s[1], s[2])
                    } else {
                        s[0].to_owned()
                    }
                })
                .collect::<Vec<_>>(),
        );

        let s = raft.clone();
        for _ in 0..600 {
            let mut v = Vec::with_capacity(20_000);
            unsafe { super::_clear_cache() };
            for s in s.iter().cycle().take(20_000) {
                v.push(u(s));
            }
        }
    }

    // This test is to have miri check the allocation code paths, but miri
    // can't open files so it's not usable right now
    // #[test]
    // fn words() {
    //     let _t = TEST_LOCK.lock();
    //     use super::ustr as u;
    //     use std::sync::Arc;

    //     let path = std::path::Path::new("/usr/share/dict/words");
    //     let wordlist = std::fs::read_to_string(path).unwrap();
    //     let wordlist = Arc::new(
    //         wordlist
    //             .split_whitespace()
    //             .collect::<Vec<_>>()
    //             .chunks(7)
    //             .cycle()
    //             .take(4_000_000)
    //             .enumerate()
    //             .map(|(i, s)| u(&format!("{}{}", i, s.join("-"))))
    //             .collect::<Vec<_>>(),
    //     );
    // }

    #[cfg(all(feature = "serde", not(miri)))]
    #[test]
    fn serialization() {
        let _t = TEST_LOCK.lock();
        use super::{string_cache_iter, ustr as u};
        use std::collections::HashSet;

        // clear the cache first or our results will be wrong
        unsafe { super::_clear_cache() };

        let path = std::path::Path::new(
            &std::env::var("CARGO_MANIFEST_DIR")
                .expect("CARGO_MANIFEST_DIR not set"),
        )
        .join("data")
        .join("blns.txt");
        let blns = std::fs::read_to_string(path).unwrap();

        let mut hs = HashSet::new();
        for s in blns.split_whitespace() {
            hs.insert(s);
        }

        let mut us = Vec::new();
        let mut ss = Vec::new();

        for s in blns.split_whitespace().cycle().take(100_000) {
            let u = u(s);
            us.push(u);
            ss.push(s.to_owned());
        }

        let json = serde_json::to_string(super::cache()).unwrap();
        unsafe {
            super::_clear_cache();
        }
        let _: super::DeserializedCache = serde_json::from_str(&json).unwrap();

        // now check that we've got the same data in the cache still
        let mut hs_u = HashSet::new();
        for s in string_cache_iter() {
            hs_u.insert(s);
        }
        let diff: HashSet<_> = hs.difference(&hs_u).collect();

        // check that the number of entries is the same
        assert_eq!(super::num_entries(), hs.len());

        // check that we have the exact same (unique) strings in the cache as in
        // the source data
        assert_eq!(diff.len(), 0);
    }

    #[cfg(all(feature = "serde", not(miri)))]
    #[test]
    fn serialization_ustr() {
        let _t = TEST_LOCK.lock();

        use super::{ustr, Ustr};

        let u_hello = ustr("hello");

        let json = serde_json::to_string(&u_hello).unwrap();
        let me_hello: Ustr = serde_json::from_str(&json).unwrap();

        assert_eq!(u_hello, me_hello);
    }

    #[test]
    fn partial_ord() {
        let _t = TEST_LOCK.lock();
        use super::ustr;
        let str_a = ustr("aaa");
        let str_z = ustr("zzz");
        let str_k = ustr("kkk");
        assert!(str_a < str_k);
        assert!(str_k < str_z);
    }

    #[test]
    fn ord() {
        let _t = TEST_LOCK.lock();
        use super::ustr;
        let u_apple = ustr("apple");
        let u_bravo = ustr("bravo");
        let u_charlie = ustr("charlie");
        let u_delta = ustr("delta");

        let mut v = vec![u_delta, u_bravo, u_charlie, u_apple];
        v.sort();
        assert_eq!(v, vec![u_apple, u_bravo, u_charlie, u_delta]);
    }

    fn takes_into_str<'a, S: Into<&'a str>>(s: S) -> &'a str {
        s.into()
    }

    #[test]
    fn test_into_str() {
        let _t = TEST_LOCK.lock();
        use super::ustr;

        assert_eq!("converted", takes_into_str(ustr("converted")));
    }

    #[test]
    fn test_existing_ustr() {
        let _t = TEST_LOCK.lock();
        use super::{existing_ustr, ustr};
        assert_eq!(existing_ustr("hello world!"), None);
        let s1 = ustr("hello world!");
        let s2 = existing_ustr("hello world!");
        assert_eq!(Some(s1), s2);
    }

    #[test]
    fn test_empty_cache() {
        unsafe { super::_clear_cache() };
        assert_eq!(
            super::string_cache_iter().collect::<Vec<_>>(),
            Vec::<&'static str>::new()
        );
    }

    #[test]
    fn as_refs() {
        let _t = TEST_LOCK.lock();

        let u = super::ustr("test");

        let s: String = u.to_owned();
        assert_eq!(u, s);
        assert_eq!(s, u);

        let p: &Path = u.as_ref();
        assert_eq!(p, u);

        let _: &[u8] = u.as_ref();

        let o: &OsStr = u.as_ref();
        assert_eq!(p, o);
        assert_eq!(o, p);

        let cow = std::borrow::Cow::from(u);
        assert_eq!(cow, u);
        assert_eq!(u, cow);

        let boxed: Box<str> = u.into();
        assert_eq!(boxed, u);
    }
}

lazy_static::lazy_static! {
    static ref STRING_CACHE: Bins = {
        use std::mem::{self, MaybeUninit};
        // This deeply unsafe feeling dance allows us to initialize an array of
        // arbitrary size and will have to tide us over until const generics
        // land. See:
        // https://doc.rust-lang.org/beta/std/mem/union.MaybeUninit.html#initializing-an-array-element-by-element

        // Create an uninitialized array of `MaybeUninit`. The `assume_init` is
        // safe because the type we are claiming to have initialized here is a
        // bunch of `MaybeUninit`s, which do not require initialization.
        let mut bins: [MaybeUninit<Mutex<StringCache>>; NUM_BINS] = unsafe {
            MaybeUninit::uninit().assume_init()
        };

        // Dropping a `MaybeUninit` does nothing. Thus using raw pointer
        // assignment instead of `ptr::write` does not cause the old
        // uninitialized value to be dropped. Also if there is a panic during
        // this loop, we have a memory leak, but there is no memory safety
        // issue.
        for bin in &mut bins[..] {
            *bin = MaybeUninit::new(Mutex::new(StringCache::default()));
        }

        // Everything is initialized. Transmute the array to the
        // initialized type.
        unsafe { mem::transmute::<_, Bins>(bins) }
    };
}

// Use the top bits of the hash to choose a bin
#[inline]
fn whichbin(hash: u64) -> usize {
    ((hash >> TOP_SHIFT as u64) % NUM_BINS as u64) as usize
}
