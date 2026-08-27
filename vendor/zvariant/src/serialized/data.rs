#[cfg(unix)]
use crate::{Fd, OwnedFd};
use std::{
    borrow::Cow,
    ops::{Bound, Deref, Range, RangeBounds},
    sync::Arc,
};

use serde::{Deserialize, de::DeserializeSeed};

use crate::{
    DynamicDeserialize, DynamicType, Error, Result, Signature, Type,
    de::Deserializer,
    serialized::{Context, Format},
};

/// Represents serialized bytes in a specific format.
///
/// On Unix platforms, it also contains a list of file descriptors, whose indexes are included in
/// the serialized bytes. By packing them together, we ensure that the file descriptors are never
/// closed before the serialized bytes are dropped.
#[derive(Clone, Debug)]
pub struct Data<'bytes, 'fds> {
    inner: Arc<Inner<'bytes, 'fds>>,
    context: Context,
    range: Range<usize>,
}

#[derive(Debug)]
pub struct Inner<'bytes, 'fds> {
    bytes: Cow<'bytes, [u8]>,
    #[cfg(unix)]
    fds: Vec<Fd<'fds>>,
    #[cfg(not(unix))]
    _fds: std::marker::PhantomData<&'fds ()>,
}

impl<'bytes, 'fds> Data<'bytes, 'fds> {
    /// Create a new `Data` instance containing borrowed file descriptors.
    ///
    /// This method is only available on Unix platforms.
    #[cfg(unix)]
    pub fn new_borrowed_fds<T>(
        bytes: T,
        context: Context,
        fds: impl IntoIterator<Item = impl Into<Fd<'fds>>>,
    ) -> Self
    where
        T: Into<Cow<'bytes, [u8]>>,
    {
        let bytes = bytes.into();
        let range = Range {
            start: 0,
            end: bytes.len(),
        };
        Data {
            inner: Arc::new(Inner {
                bytes,
                fds: fds.into_iter().map(Into::into).collect(),
            }),
            range,
            context,
        }
    }

    /// The serialized bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.inner.bytes[self.range.start..self.range.end]
    }

    /// The encoding context.
    pub fn context(&self) -> Context {
        self.context
    }

    /// The file descriptors that are references by the serialized bytes.
    ///
    /// This method is only available on Unix platforms.
    #[cfg(unix)]
    pub fn fds(&self) -> &[Fd<'fds>] {
        &self.inner.fds
    }

    /// Returns a slice of `self` for the provided range.
    ///
    /// # Panics
    ///
    /// Requires that begin <= end and end <= self.len(), otherwise slicing will panic.
    pub fn slice(&self, range: impl RangeBounds<usize>) -> Data<'bytes, 'fds> {
        let len = self.range.end - self.range.start;
        let start = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&n) => n + 1,
            Bound::Excluded(&n) => n,
            Bound::Unbounded => len,
        };
        assert!(
            start <= end,
            "range start must not be greater than end: {start:?} > {end:?}",
        );
        assert!(end <= len, "range end out of bounds: {end:?} > {len:?}");

        let context = Context::new(
            self.context.format(),
            self.context.endian(),
            self.context.position() + start,
        );
        let range = Range {
            start: self.range.start + start,
            end: self.range.start + end,
        };

        Data {
            inner: self.inner.clone(),
            context,
            range,
        }
    }

    /// Deserialize `T` from `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use zvariant::LE;
    /// use zvariant::to_bytes;
    /// use zvariant::serialized::Context;
    ///
    /// let ctxt = Context::new_dbus(LE, 0);
    /// let encoded = to_bytes(ctxt, "hello world").unwrap();
    /// let decoded: &str = encoded.deserialize().unwrap().0;
    /// assert_eq!(decoded, "hello world");
    /// ```
    ///
    /// # Return value
    ///
    /// A tuple containing the deserialized value and the number of bytes parsed from `bytes`.
    pub fn deserialize<'d, T>(&'d self) -> Result<(T, usize)>
    where
        T: Deserialize<'d> + Type,
    {
        self.deserialize_for_signature(T::SIGNATURE)
    }

    /// Deserialize `T` from `self` with the given signature.
    ///
    /// Use this method instead of [`Data::deserialize`] if the value being deserialized does not
    /// implement [`Type`].
    ///
    /// # Examples
    ///
    /// While `Type` derive supports enums, for this example, let's supposed it doesn't and we don't
    /// want to manually implement `Type` trait either:
    ///
    /// ```rust
    /// use serde::{Deserialize, Serialize};
    /// use zvariant::{
    ///     LE, to_bytes_for_signature, serialized::Context,
    ///     signature::{Signature, Fields},
    /// };
    ///
    /// let ctxt = Context::new_dbus(LE, 0);
    /// #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    /// enum Unit {
    ///     Variant1,
    ///     Variant2,
    ///     Variant3,
    /// }
    ///
    /// let encoded = to_bytes_for_signature(ctxt, &Signature::U32, &Unit::Variant2).unwrap();
    /// assert_eq!(encoded.len(), 4);
    /// let decoded: Unit = encoded.deserialize_for_signature(&Signature::U32).unwrap().0;
    /// assert_eq!(decoded, Unit::Variant2);
    ///
    /// #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    /// enum NewType<'s> {
    ///     Variant1(&'s str),
    ///     Variant2(&'s str),
    ///     Variant3(&'s str),
    /// }
    ///
    /// let signature = Signature::Structure(Fields::Static {
    ///     fields: &[&Signature::U32, &Signature::Str],
    /// });
    /// let encoded =
    ///     to_bytes_for_signature(ctxt, &signature, &NewType::Variant2("hello")).unwrap();
    /// assert_eq!(encoded.len(), 14);
    /// let decoded: NewType<'_> = encoded.deserialize_for_signature(&signature).unwrap().0;
    /// assert_eq!(decoded, NewType::Variant2("hello"));
    ///
    /// #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    /// enum Structs {
    ///     Tuple(u8, u64),
    ///     Struct { y: u8, t: u64 },
    /// }
    ///
    /// let signature = Signature::Structure(Fields::Static {
    ///     fields: &[
    ///         &Signature::U32,
    ///         &Signature::Structure(Fields::Static {
    ///             fields: &[&Signature::U8, &Signature::U64],
    ///         }),
    ///     ],
    /// });
    /// let encoded = to_bytes_for_signature(ctxt, &signature, &Structs::Tuple(42, 42)).unwrap();
    /// assert_eq!(encoded.len(), 24);
    /// let decoded: Structs = encoded.deserialize_for_signature(&signature).unwrap().0;
    /// assert_eq!(decoded, Structs::Tuple(42, 42));
    ///
    /// let s = Structs::Struct { y: 42, t: 42 };
    /// let encoded = to_bytes_for_signature(ctxt, &signature, &s).unwrap();
    /// assert_eq!(encoded.len(), 24);
    /// let decoded: Structs = encoded.deserialize_for_signature(&signature).unwrap().0;
    /// assert_eq!(decoded, Structs::Struct { y: 42, t: 42 });
    /// ```
    ///
    /// # Return value
    ///
    /// A tuple containing the deserialized value and the number of bytes parsed from `bytes`.
    pub fn deserialize_for_signature<'d, S, T>(&'d self, signature: S) -> Result<(T, usize)>
    where
        T: Deserialize<'d>,
        S: TryInto<Signature>,
        S::Error: Into<Error>,
    {
        let signature = signature.try_into().map_err(Into::into)?;

        #[cfg(unix)]
        let fds = &self.inner.fds;
        let mut de = match self.context.format() {
            #[cfg(feature = "gvariant")]
            #[allow(deprecated)]
            Format::GVariant => {
                #[cfg(unix)]
                {
                    crate::gvariant::Deserializer::new(
                        self.bytes(),
                        Some(fds),
                        &signature,
                        self.context,
                    )
                }
                #[cfg(not(unix))]
                {
                    crate::gvariant::Deserializer::<()>::new(self.bytes(), &signature, self.context)
                }
            }
            .map(Deserializer::GVariant)?,
            Format::DBus => {
                #[cfg(unix)]
                {
                    crate::dbus::Deserializer::new(
                        self.bytes(),
                        Some(fds),
                        &signature,
                        self.context,
                    )
                }
                #[cfg(not(unix))]
                {
                    crate::dbus::Deserializer::<()>::new(self.bytes(), &signature, self.context)
                }
            }
            .map(Deserializer::DBus)?,
            // `Format` can still have a `GVariant` variant here even with `zvariant`'s own
            // `gvariant` feature disabled: if some other crate in the dependency graph (e.g.
            // `zgvariant`) enables `zvariant_utils/gvariant`, Cargo feature unification adds
            // the variant to this build regardless. `zvariant`'s own `#[cfg(feature = ...)]`
            // can't detect that (Cargo features don't propagate that way), so the variant
            // can't be named explicitly here without breaking the common case where it
            // doesn't exist at all. Fall back to a wildcard instead: it's unreachable
            // whenever `gvariant` is enabled (the two arms above are then exhaustive) or the
            // variant doesn't exist.
            #[cfg(not(feature = "gvariant"))]
            #[allow(unreachable_patterns)]
            _ => {
                return Err(Error::Message(
                    "GVariant support has moved to the `zgvariant` crate; enable `zvariant`'s \
                     deprecated `gvariant` feature only for legacy compatibility"
                        .to_owned(),
                ));
            }
        };

        T::deserialize(&mut de).map(|t| match de {
            #[cfg(feature = "gvariant")]
            #[allow(deprecated)]
            Deserializer::GVariant(de) => (t, de.0.pos),
            Deserializer::DBus(de) => (t, de.0.pos),
        })
    }

    /// Deserialize `T` from `self`, with the given dynamic signature.
    ///
    /// # Return value
    ///
    /// A tuple containing the deserialized value and the number of bytes parsed from `bytes`.
    pub fn deserialize_for_dynamic_signature<'d, S, T>(&'d self, signature: S) -> Result<(T, usize)>
    where
        T: DynamicDeserialize<'d>,
        S: TryInto<Signature>,
        S::Error: Into<Error>,
    {
        let signature = signature.try_into().map_err(Into::into)?;
        let seed = T::deserializer_for_signature(&signature)?;

        self.deserialize_with_seed(seed)
    }

    /// Deserialize `T` from `self`, using the given seed.
    ///
    /// # Return value
    ///
    /// A tuple containing the deserialized value and the number of bytes parsed from `bytes`.
    pub fn deserialize_with_seed<'d, S>(&'d self, seed: S) -> Result<(S::Value, usize)>
    where
        S: DeserializeSeed<'d> + DynamicType,
    {
        let signature = S::signature(&seed);

        #[cfg(unix)]
        let fds = &self.inner.fds;
        let mut de = match self.context.format() {
            #[cfg(feature = "gvariant")]
            #[allow(deprecated)]
            Format::GVariant => {
                #[cfg(unix)]
                {
                    crate::gvariant::Deserializer::new(
                        self.bytes(),
                        Some(fds),
                        &signature,
                        self.context,
                    )
                }
                #[cfg(not(unix))]
                {
                    crate::gvariant::Deserializer::new(self.bytes(), &signature, self.context)
                }
            }
            .map(Deserializer::GVariant)?,
            Format::DBus => {
                #[cfg(unix)]
                {
                    crate::dbus::Deserializer::new(
                        self.bytes(),
                        Some(fds),
                        &signature,
                        self.context,
                    )
                }
                #[cfg(not(unix))]
                {
                    crate::dbus::Deserializer::<()>::new(self.bytes(), &signature, self.context)
                }
            }
            .map(Deserializer::DBus)?,
            // See the comment on the equivalent arm in `deserialize_for_signature` above.
            #[cfg(not(feature = "gvariant"))]
            #[allow(unreachable_patterns)]
            _ => {
                return Err(Error::Message(
                    "GVariant support has moved to the `zgvariant` crate; enable `zvariant`'s \
                     deprecated `gvariant` feature only for legacy compatibility"
                        .to_owned(),
                ));
            }
        };

        seed.deserialize(&mut de).map(|t| match de {
            #[cfg(feature = "gvariant")]
            #[allow(deprecated)]
            Deserializer::GVariant(de) => (t, de.0.pos),
            Deserializer::DBus(de) => (t, de.0.pos),
        })
    }
}

impl<'bytes> Data<'bytes, 'static> {
    /// Create a new `Data` instance.
    pub fn new<T>(bytes: T, context: Context) -> Self
    where
        T: Into<Cow<'bytes, [u8]>>,
    {
        let bytes = bytes.into();
        let range = Range {
            start: 0,
            end: bytes.len(),
        };
        Data {
            inner: Arc::new(Inner {
                bytes,
                #[cfg(unix)]
                fds: vec![],
                #[cfg(not(unix))]
                _fds: std::marker::PhantomData,
            }),
            context,
            range,
        }
    }

    /// Create a new `Data` instance containing owned file descriptors.
    ///
    /// This method is only available on Unix platforms.
    #[cfg(unix)]
    pub fn new_fds<T>(
        bytes: T,
        context: Context,
        fds: impl IntoIterator<Item = impl Into<OwnedFd>>,
    ) -> Self
    where
        T: Into<Cow<'bytes, [u8]>>,
    {
        let bytes = bytes.into();
        let range = Range {
            start: 0,
            end: bytes.len(),
        };
        Data {
            inner: Arc::new(Inner {
                bytes,
                fds: fds.into_iter().map(Into::into).map(Fd::from).collect(),
            }),
            context,
            range,
        }
    }
}

impl Deref for Data<'_, '_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.bytes()
    }
}

impl<T> AsRef<T> for Data<'_, '_>
where
    T: ?Sized,
    for<'bytes, 'fds> <Data<'bytes, 'fds> as Deref>::Target: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}
