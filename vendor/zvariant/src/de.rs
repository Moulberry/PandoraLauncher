use serde::de::{self, DeserializeSeed, VariantAccess, Visitor};

use std::{marker::PhantomData, str};

#[cfg(unix)]
use std::os::fd::{AsFd, AsRawFd};

#[cfg(feature = "gvariant")]
#[allow(deprecated)]
use crate::gvariant::Deserializer as GVDeserializer;
use crate::{
    Basic, Error, Result, Signature, container_depths::ContainerDepths,
    dbus::Deserializer as DBusDeserializer, serialized::Context, utils::*,
};

/// Our deserialization implementation.
#[derive(Debug)]
pub(crate) struct DeserializerCommon<'de, 'sig, 'f, F> {
    pub(crate) ctxt: Context,
    pub(crate) bytes: &'de [u8],

    #[cfg(unix)]
    pub(crate) fds: Option<&'f [F]>,
    #[cfg(not(unix))]
    pub(crate) fds: PhantomData<&'f F>,

    pub(crate) pos: usize,

    pub(crate) signature: &'sig Signature,

    pub(crate) container_depths: ContainerDepths,
}

/// Our deserialization implementation.
///
/// Using this deserializer involves an redirection to the actual deserializer. It's best
/// to use the serialization functions, e.g [`crate::to_bytes`] or specific serializers,
/// [`crate::dbus::Deserializer`] or [`crate::zvariant::Deserializer`].
pub(crate) enum Deserializer<'ser, 'sig, 'f, F> {
    DBus(DBusDeserializer<'ser, 'sig, 'f, F>),
    #[cfg(feature = "gvariant")]
    #[allow(deprecated)]
    GVariant(GVDeserializer<'ser, 'sig, 'f, F>),
}

#[cfg(unix)]
impl<F> DeserializerCommon<'_, '_, '_, F>
where
    F: AsFd,
{
    pub fn get_fd(&self, idx: u32) -> Result<i32> {
        self.fds
            .and_then(|fds| fds.get(idx as usize).map(|fd| fd.as_fd().as_raw_fd()))
            .ok_or(Error::UnknownFd)
    }
}

impl<'de, F> DeserializerCommon<'de, '_, '_, F> {
    pub fn parse_padding(&mut self, alignment: usize) -> Result<usize> {
        let padding = padding_for_n_bytes(self.abs_pos(), alignment);
        if padding > 0 {
            if self.pos + padding > self.bytes.len() {
                return Err(serde::de::Error::invalid_length(
                    self.bytes.len(),
                    &format!(">= {}", self.pos + padding).as_str(),
                ));
            }

            for i in 0..padding {
                let byte = self.bytes[self.pos + i];
                if byte != 0 {
                    return Err(Error::PaddingNot0(byte));
                }
            }
            self.pos += padding;
        }

        Ok(padding)
    }

    pub fn prep_deserialize_basic<T>(&mut self) -> Result<()>
    where
        T: Basic,
    {
        self.parse_padding(T::alignment(self.ctxt.format()))?;

        Ok(())
    }

    pub fn next_slice(&mut self, len: usize) -> Result<&'de [u8]> {
        if self.pos + len > self.bytes.len() {
            return Err(serde::de::Error::invalid_length(
                self.bytes.len(),
                &format!(">= {}", self.pos + len).as_str(),
            ));
        }

        let slice = &self.bytes[self.pos..self.pos + len];
        self.pos += len;

        Ok(slice)
    }

    pub fn next_const_size_slice<T>(&mut self) -> Result<&[u8]>
    where
        T: Basic,
    {
        self.prep_deserialize_basic::<T>()?;

        self.next_slice(T::alignment(self.ctxt.format()))
    }

    pub fn abs_pos(&self) -> usize {
        self.ctxt.position() + self.pos
    }
}

macro_rules! deserialize_method {
    ($method:ident($($arg:ident: $type:ty),*)) => {
        #[inline]
        fn $method<V>(self, $($arg: $type,)* visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            match self {
                #[cfg(feature = "gvariant")]
                Deserializer::GVariant(de) => {
                    de.$method($($arg,)* visitor)
                }
                Deserializer::DBus(de) => {
                    de.$method($($arg,)* visitor)
                }
            }
        }
    }
}

impl<'de, #[cfg(unix)] F: AsFd, #[cfg(not(unix))] F> de::Deserializer<'de>
    for &mut Deserializer<'de, '_, '_, F>
{
    type Error = Error;

    deserialize_method!(deserialize_any());
    deserialize_method!(deserialize_bool());
    deserialize_method!(deserialize_i8());
    deserialize_method!(deserialize_i16());
    deserialize_method!(deserialize_i32());
    deserialize_method!(deserialize_i64());
    deserialize_method!(deserialize_u8());
    deserialize_method!(deserialize_u16());
    deserialize_method!(deserialize_u32());
    deserialize_method!(deserialize_u64());
    deserialize_method!(deserialize_f32());
    deserialize_method!(deserialize_f64());
    deserialize_method!(deserialize_char());
    deserialize_method!(deserialize_str());
    deserialize_method!(deserialize_string());
    deserialize_method!(deserialize_bytes());
    deserialize_method!(deserialize_byte_buf());
    deserialize_method!(deserialize_option());
    deserialize_method!(deserialize_unit());
    // Brace-delimited invocations: the args are `name: type` pairs, which recent nightly rustfmt
    // mis-parses as parenthesized generic arguments (dropping the names) in `!(...)` form.
    deserialize_method! { deserialize_unit_struct(n: &'static str) }
    deserialize_method! { deserialize_newtype_struct(n: &'static str) }
    deserialize_method!(deserialize_seq());
    deserialize_method!(deserialize_map());
    deserialize_method! { deserialize_tuple(n: usize) }
    deserialize_method! { deserialize_tuple_struct(n: &'static str, l: usize) }
    deserialize_method! { deserialize_struct(n: &'static str, f: &'static [&'static str]) }
    deserialize_method! { deserialize_enum(n: &'static str, f: &'static [&'static str]) }
    deserialize_method!(deserialize_identifier());
    deserialize_method!(deserialize_ignored_any());

    fn is_human_readable(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub(crate) enum ValueParseStage {
    Signature,
    Value,
    Done,
}

pub(crate) fn deserialize_any<'de, 'f, D, V>(
    de: D,
    signature: &Signature,
    visitor: V,
) -> Result<V::Value>
where
    D: de::Deserializer<'de, Error = Error>,
    V: Visitor<'de>,
{
    match signature {
        Signature::Unit => de.deserialize_unit(visitor),
        Signature::U8 => de.deserialize_u8(visitor),
        Signature::Bool => de.deserialize_bool(visitor),
        Signature::I16 => de.deserialize_i16(visitor),
        Signature::U16 => de.deserialize_u16(visitor),
        Signature::I32 => de.deserialize_i32(visitor),
        #[cfg(unix)]
        Signature::Fd => de.deserialize_i32(visitor),
        Signature::U32 => de.deserialize_u32(visitor),
        Signature::I64 => de.deserialize_i64(visitor),
        Signature::U64 => de.deserialize_u64(visitor),
        Signature::F64 => de.deserialize_f64(visitor),
        Signature::Str | Signature::ObjectPath | Signature::Signature => {
            de.deserialize_str(visitor)
        }
        Signature::Variant => de.deserialize_seq(visitor),
        Signature::Array(_) => de.deserialize_seq(visitor),
        Signature::Dict { .. } => de.deserialize_map(visitor),
        Signature::Structure { .. } => de.deserialize_seq(visitor),
        #[cfg(feature = "gvariant")]
        Signature::Maybe(_) => de.deserialize_option(visitor),
        // `Signature` can still have a `Maybe` variant here even with `zvariant`'s own
        // `gvariant` feature disabled: if some other crate in the dependency graph (e.g.
        // `zgvariant`) enables `zvariant_utils/gvariant`, Cargo feature unification adds the
        // variant to this build regardless. `zvariant`'s own `#[cfg(feature = ...)]` can't
        // detect that (Cargo features don't propagate that way), so the variant can't be
        // named explicitly here without breaking the common case where it doesn't exist at
        // all. Fall back to a wildcard instead: it's unreachable whenever `gvariant` is
        // enabled (all the arms above are then exhaustive) or the variant doesn't exist.
        #[cfg(not(feature = "gvariant"))]
        #[allow(unreachable_patterns)]
        _ => Err(Error::SignatureMismatch(
            signature.clone(),
            "GVariant `Maybe` support has moved to the `zgvariant` crate; enable `zvariant`'s \
             deprecated `gvariant` feature only for legacy compatibility"
                .to_string(),
        )),
    }
}

// Enum handling is very generic so it can be here and specific deserializers can use this.
pub(crate) struct Enum<D, F> {
    pub(crate) de: D,
    pub(crate) name: &'static str,
    pub(crate) _phantom: PhantomData<F>,
}

impl<'de, D, F> VariantAccess<'de> for Enum<D, F>
where
    D: de::Deserializer<'de, Error = Error>,
{
    type Error = Error;

    fn unit_variant(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(self.de)
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_struct(self.de, self.name, &[], visitor)
    }

    fn struct_variant<V>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_struct(self.de, self.name, fields, visitor)
    }
}
