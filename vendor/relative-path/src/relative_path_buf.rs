use core::cmp;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::iter::FromIterator;
use core::ops;
use core::str;

use alloc::borrow::{Borrow, Cow, ToOwned};
use alloc::boxed::Box;
use alloc::string::String;

#[cfg(feature = "std")]
use std::path;

use super::{Component, RelativePath, PARENT_STR, SEP, STEM_SEP};

#[cfg(feature = "std")]
use super::{FromPathError, FromPathErrorKind};

/// Traverse the given components and apply to the provided stack.
///
/// This takes '.', and '..' into account. Where '.' doesn't change the stack, and '..' pops the
/// last item or further adds parent components.
#[inline(always)]
pub(super) fn relative_traversal<'a, C>(buf: &mut RelativePathBuf, components: C)
where
    C: IntoIterator<Item = Component<'a>>,
{
    use self::Component::{CurDir, Normal, ParentDir};

    for c in components {
        match c {
            CurDir => (),
            ParentDir => match buf.components().next_back() {
                Some(Component::ParentDir) | None => {
                    buf.push(PARENT_STR);
                }
                _ => {
                    buf.pop();
                }
            },
            Normal(name) => {
                buf.push(name);
            }
        }
    }
}

/// An owned, mutable relative path.
///
/// This type provides methods to manipulate relative path objects.
#[derive(Clone)]
pub struct RelativePathBuf {
    inner: String,
}

impl RelativePathBuf {
    /// Create a new relative path buffer.
    #[must_use]
    #[inline]
    pub fn new() -> RelativePathBuf {
        RelativePathBuf {
            inner: String::new(),
        }
    }

    /// Internal constructor to allocate a relative path buf with the given capacity.
    #[inline]
    pub(super) fn with_capacity(cap: usize) -> RelativePathBuf {
        RelativePathBuf {
            inner: String::with_capacity(cap),
        }
    }

    /// Get the length of the underlying string in bytes.
    #[inline]
    pub(super) fn len(&self) -> usize {
        self.inner.len()
    }

    /// Try to convert a [`Path`] to a [`RelativePathBuf`].
    ///
    /// [`Path`]: https://doc.rust-lang.org/std/path/struct.Path.html
    ///
    /// # Examples
    ///
    /// ```
    /// use relative_path::{RelativePath, RelativePathBuf, FromPathErrorKind};
    /// use std::path::Path;
    ///
    /// assert_eq!(
    ///     Ok(RelativePath::new("foo/bar").to_owned()),
    ///     RelativePathBuf::from_path(Path::new("foo/bar"))
    /// );
    /// ```
    ///
    /// # Errors
    ///
    /// This will error in case the provided path is not a relative path, which
    /// is identifier by it having a [`Prefix`] or [`RootDir`] component.
    ///
    /// [`Prefix`]: std::path::Component::Prefix
    /// [`RootDir`]: std::path::Component::RootDir
    #[cfg(feature = "std")]
    #[cfg_attr(relative_path_docsrs, doc(cfg(feature = "std")))]
    #[inline]
    pub fn from_path<P>(path: P) -> Result<RelativePathBuf, FromPathError>
    where
        P: AsRef<path::Path>,
    {
        use std::path::Component::{CurDir, Normal, ParentDir, Prefix, RootDir};

        let mut buffer = RelativePathBuf::new();

        for c in path.as_ref().components() {
            match c {
                Prefix(_) | RootDir => return Err(FromPathErrorKind::NonRelative.into()),
                CurDir => continue,
                ParentDir => buffer.push(PARENT_STR),
                Normal(s) => buffer.push(s.to_str().ok_or(FromPathErrorKind::NonUtf8)?),
            }
        }

        Ok(buffer)
    }

    /// Extends `self` with `path`.
    ///
    /// # Examples
    ///
    /// ```
    /// use relative_path::RelativePathBuf;
    ///
    /// let mut path = RelativePathBuf::new();
    /// path.push("foo");
    /// path.push("bar");
    ///
    /// assert_eq!("foo/bar", path);
    ///
    /// let mut path = RelativePathBuf::new();
    /// path.push("foo");
    /// path.push("/bar");
    ///
    /// assert_eq!("foo/bar", path);
    /// ```
    pub fn push<P>(&mut self, path: P)
    where
        P: AsRef<RelativePath>,
    {
        let other = path.as_ref();

        let other = if other.starts_with_sep() {
            &other.inner[1..]
        } else {
            &other.inner[..]
        };

        if !self.inner.is_empty() && !self.ends_with_sep() {
            self.inner.push(SEP);
        }

        self.inner.push_str(other);
    }

    /// Updates [`file_name`] to `file_name`.
    ///
    /// If [`file_name`] was [`None`], this is equivalent to pushing
    /// `file_name`.
    ///
    /// Otherwise it is equivalent to calling [`pop`] and then pushing
    /// `file_name`. The new path will be a sibling of the original path. (That
    /// is, it will have the same parent.)
    ///
    /// [`file_name`]: RelativePath::file_name
    /// [`pop`]: RelativePathBuf::pop
    /// [`None`]: https://doc.rust-lang.org/std/option/enum.Option.html
    ///
    /// # Examples
    ///
    /// ```
    /// use relative_path::RelativePathBuf;
    ///
    /// let mut buf = RelativePathBuf::from("");
    /// assert!(buf.file_name() == None);
    /// buf.set_file_name("bar");
    /// assert_eq!(RelativePathBuf::from("bar"), buf);
    ///
    /// assert!(buf.file_name().is_some());
    /// buf.set_file_name("baz.txt");
    /// assert_eq!(RelativePathBuf::from("baz.txt"), buf);
    ///
    /// buf.push("bar");
    /// assert!(buf.file_name().is_some());
    /// buf.set_file_name("bar.txt");
    /// assert_eq!(RelativePathBuf::from("baz.txt/bar.txt"), buf);
    /// ```
    pub fn set_file_name<S>(&mut self, file_name: S)
    where
        S: AsRef<str>,
    {
        if self.file_name().is_some() {
            let popped = self.pop();
            debug_assert!(popped);
        }

        self.push(file_name.as_ref());
    }

    /// Updates [`extension`] to `extension`.
    ///
    /// Returns `false` and does nothing if
    /// [`file_name`][RelativePath::file_name] is [`None`], returns `true` and
    /// updates the extension otherwise.
    ///
    /// If [`extension`] is [`None`], the extension is added; otherwise it is
    /// replaced.
    ///
    /// [`extension`]: RelativePath::extension
    ///
    /// # Examples
    ///
    /// ```
    /// use relative_path::{RelativePath, RelativePathBuf};
    ///
    /// let mut p = RelativePathBuf::from("feel/the");
    ///
    /// p.set_extension("force");
    /// assert_eq!(RelativePath::new("feel/the.force"), p);
    ///
    /// p.set_extension("dark_side");
    /// assert_eq!(RelativePath::new("feel/the.dark_side"), p);
    ///
    /// assert!(p.pop());
    /// p.set_extension("nothing");
    /// assert_eq!(RelativePath::new("feel.nothing"), p);
    /// ```
    pub fn set_extension<S>(&mut self, extension: S) -> bool
    where
        S: AsRef<str>,
    {
        let file_stem = match self.file_stem() {
            Some(stem) => stem,
            None => return false,
        };

        let end_file_stem = file_stem[file_stem.len()..].as_ptr() as usize;
        let start = self.inner.as_ptr() as usize;
        self.inner.truncate(end_file_stem.wrapping_sub(start));

        let extension = extension.as_ref();

        if !extension.is_empty() {
            self.inner.push(STEM_SEP);
            self.inner.push_str(extension);
        }

        true
    }

    /// Truncates `self` to [`parent`][RelativePath::parent].
    ///
    /// # Examples
    ///
    /// ```
    /// use relative_path::{RelativePath, RelativePathBuf};
    ///
    /// let mut p = RelativePathBuf::from("test/test.rs");
    ///
    /// assert_eq!(true, p.pop());
    /// assert_eq!(RelativePath::new("test"), p);
    /// assert_eq!(true, p.pop());
    /// assert_eq!(RelativePath::new(""), p);
    /// assert_eq!(false, p.pop());
    /// assert_eq!(RelativePath::new(""), p);
    /// ```
    pub fn pop(&mut self) -> bool {
        match self.parent().map(|p| p.inner.len()) {
            Some(len) => {
                self.inner.truncate(len);
                true
            }
            None => false,
        }
    }

    /// Coerce to a [`RelativePath`] slice.
    #[must_use]
    #[inline]
    pub fn as_relative_path(&self) -> &RelativePath {
        self
    }

    /// Consumes the `RelativePathBuf`, yielding its internal [`String`] storage.
    ///
    /// # Examples
    ///
    /// ```
    /// use relative_path::RelativePathBuf;
    ///
    /// let p = RelativePathBuf::from("/the/head");
    /// let string = p.into_string();
    /// assert_eq!(string, "/the/head".to_owned());
    /// ```
    #[must_use]
    #[inline]
    pub fn into_string(self) -> String {
        self.inner
    }

    /// Converts this `RelativePathBuf` into a [boxed][alloc::boxed::Box]
    /// [`RelativePath`].
    #[must_use]
    #[inline]
    pub fn into_boxed_relative_path(self) -> Box<RelativePath> {
        let rw = Box::into_raw(self.inner.into_boxed_str()) as *mut RelativePath;
        unsafe { Box::from_raw(rw) }
    }
}

impl Default for RelativePathBuf {
    #[inline]
    fn default() -> Self {
        RelativePathBuf::new()
    }
}

impl<'a> From<&'a RelativePath> for Cow<'a, RelativePath> {
    #[inline]
    fn from(s: &'a RelativePath) -> Cow<'a, RelativePath> {
        Cow::Borrowed(s)
    }
}

impl<'a> From<RelativePathBuf> for Cow<'a, RelativePath> {
    #[inline]
    fn from(s: RelativePathBuf) -> Cow<'a, RelativePath> {
        Cow::Owned(s)
    }
}

impl fmt::Debug for RelativePathBuf {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:?}", &self.inner)
    }
}

impl AsRef<RelativePath> for RelativePathBuf {
    #[inline]
    fn as_ref(&self) -> &RelativePath {
        RelativePath::new(&self.inner)
    }
}

impl AsRef<str> for RelativePath {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.inner
    }
}

impl Borrow<RelativePath> for RelativePathBuf {
    #[inline]
    fn borrow(&self) -> &RelativePath {
        self
    }
}

impl<'a, T: ?Sized + AsRef<str>> From<&'a T> for RelativePathBuf {
    #[inline]
    fn from(path: &'a T) -> RelativePathBuf {
        RelativePathBuf {
            inner: path.as_ref().to_owned(),
        }
    }
}

impl From<String> for RelativePathBuf {
    #[inline]
    fn from(path: String) -> RelativePathBuf {
        RelativePathBuf { inner: path }
    }
}

impl From<RelativePathBuf> for String {
    #[inline]
    fn from(path: RelativePathBuf) -> String {
        path.into_string()
    }
}

impl ops::Deref for RelativePathBuf {
    type Target = RelativePath;

    #[inline]
    fn deref(&self) -> &RelativePath {
        RelativePath::new(&self.inner)
    }
}

impl cmp::PartialEq for RelativePathBuf {
    #[inline]
    fn eq(&self, other: &RelativePathBuf) -> bool {
        self.components() == other.components()
    }
}

impl cmp::Eq for RelativePathBuf {}

impl cmp::PartialOrd for RelativePathBuf {
    #[inline]
    fn partial_cmp(&self, other: &RelativePathBuf) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Ord for RelativePathBuf {
    #[inline]
    fn cmp(&self, other: &RelativePathBuf) -> cmp::Ordering {
        self.components().cmp(other.components())
    }
}

impl Hash for RelativePathBuf {
    #[inline]
    fn hash<H>(&self, h: &mut H)
    where
        H: Hasher,
    {
        self.as_relative_path().hash(h);
    }
}

impl<P> Extend<P> for RelativePathBuf
where
    P: AsRef<RelativePath>,
{
    #[inline]
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = P>,
    {
        iter.into_iter().for_each(move |p| self.push(p.as_ref()));
    }
}

impl<P> FromIterator<P> for RelativePathBuf
where
    P: AsRef<RelativePath>,
{
    #[inline]
    fn from_iter<I>(iter: I) -> RelativePathBuf
    where
        I: IntoIterator<Item = P>,
    {
        let mut buf = RelativePathBuf::new();
        buf.extend(iter);
        buf
    }
}

impl fmt::Display for RelativePathBuf {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

/// [`AsRef<str>`] implementation for [`RelativePathBuf`].
///
/// # Examples
///
/// ```
/// use relative_path::RelativePathBuf;
///
/// let path = RelativePathBuf::from("foo/bar");
/// let string: &str = path.as_ref();
/// assert_eq!(string, "foo/bar");
/// ```
impl AsRef<str> for RelativePathBuf {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.inner
    }
}

/// [`serde::ser::Serialize`] implementation for [`RelativePathBuf`].
///
/// ```
/// use serde::Serialize;
/// use relative_path::RelativePathBuf;
///
/// #[derive(Serialize)]
/// struct Document {
///     path: RelativePathBuf,
/// }
/// ```
#[cfg(feature = "serde")]
#[cfg_attr(relative_path_docsrs, doc(cfg(feature = "serde")))]
impl serde::ser::Serialize for RelativePathBuf {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(&self.inner)
    }
}

/// [`serde::de::Deserialize`] implementation for [`RelativePathBuf`].
///
/// ```
/// use serde::Deserialize;
/// use relative_path::RelativePathBuf;
///
/// #[derive(Deserialize)]
/// struct Document {
///     path: RelativePathBuf,
/// }
/// ```
#[cfg(feature = "serde")]
#[cfg_attr(relative_path_docsrs, doc(cfg(feature = "serde")))]
impl<'de> serde::de::Deserialize<'de> for RelativePathBuf {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct Visitor;

        impl serde::de::Visitor<'_> for Visitor {
            type Value = RelativePathBuf;

            #[inline]
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a relative path")
            }

            #[inline]
            fn visit_string<E>(self, input: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RelativePathBuf::from(input))
            }

            #[inline]
            fn visit_str<E>(self, input: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RelativePathBuf::from(input.to_owned()))
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}
