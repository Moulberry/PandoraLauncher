//! Support for the type-definition elements of the [Telepathy D-Bus introspection extensions].
//!
//! While the D-Bus type system only knows structural types (through signatures), the Telepathy
//! extensions allow an introspection document to give them names and documentation:
//!
//! * `<tp:simple-type>` names a plain D-Bus type ([`SimpleType`]),
//! * `<tp:enum>` enumerates the values a type can take ([`Enum`]),
//! * `<tp:struct>` names a structure and its members ([`Struct`]),
//! * `<tp:mapping>` names a dictionary type ([`Mapping`]).
//!
//! Type definitions appear as children of the `<node>` or `<interface>` elements (see
//! [`Node::telepathy_types`](crate::Node::telepathy_types) and
//! [`Interface::telepathy_types`](crate::Interface::telepathy_types)) and are referenced by
//! name through the `tp:type` attribute of `<arg>`, `<property>` and `<tp:member>` elements
//! (see e. g. [`Arg::tp_type`](crate::Arg::tp_type)), where the name may carry one `[]`
//! suffix per level of array nesting (e. g. `Playlist[]`).
//!
//! A definition that cannot be parsed — a missing required attribute, an invalid signature —
//! does not fail the document: it is skipped with a [`Warning`](crate::Warning), in the spirit
//! of treating everything beyond the core introspection format as optional extras. Type
//! definitions are also parse-only: [`Node::to_writer`](crate::Node::to_writer) does not emit
//! them.
//!
//! [Telepathy D-Bus introspection extensions]: https://telepathy.freedesktop.org/spec/

use crate::Signature;

/// A named type defined through the Telepathy introspection extensions.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeDef {
    /// A name given to a plain D-Bus type (`<tp:simple-type>`).
    SimpleType(SimpleType),
    /// An enumeration of the values of a type (`<tp:enum>`).
    Enum(Enum),
    /// A named structure (`<tp:struct>`).
    Struct(Struct),
    /// A named dictionary type (`<tp:mapping>`).
    Mapping(Mapping),
}

impl TypeDef {
    /// The name of the defined type, as referenced by `tp:type` attributes.
    pub fn name(&self) -> &str {
        match self {
            TypeDef::SimpleType(t) => t.name(),
            TypeDef::Enum(e) => e.name(),
            TypeDef::Struct(s) => s.name(),
            TypeDef::Mapping(m) => m.name(),
        }
    }

    /// The docstring of the definition, if any.
    pub fn docstring(&self) -> Option<&str> {
        match self {
            TypeDef::SimpleType(t) => t.docstring(),
            TypeDef::Enum(e) => e.docstring(),
            TypeDef::Struct(s) => s.docstring(),
            TypeDef::Mapping(m) => m.docstring(),
        }
    }

    /// The D-Bus signature of the defined type.
    pub fn signature(&self) -> zvariant::Signature {
        match self {
            TypeDef::SimpleType(t) => t.ty().inner().clone(),
            TypeDef::Enum(e) => e.ty().inner().clone(),
            TypeDef::Struct(s) => s.signature(),
            TypeDef::Mapping(m) => m.signature(),
        }
    }
}

/// A name given to a plain D-Bus type (`<tp:simple-type>`).
#[derive(Debug, Clone, PartialEq)]
pub struct SimpleType {
    pub(crate) name: String,
    pub(crate) ty: Signature,
    pub(crate) docstring: Option<String>,
}

impl SimpleType {
    /// The name of the type.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The underlying D-Bus type.
    pub fn ty(&self) -> &Signature {
        &self.ty
    }

    /// The docstring of the definition, if any.
    pub fn docstring(&self) -> Option<&str> {
        self.docstring.as_deref()
    }
}

/// An enumeration of the values of a type (`<tp:enum>`).
#[derive(Debug, Clone, PartialEq)]
pub struct Enum {
    pub(crate) name: String,
    pub(crate) ty: Signature,
    pub(crate) values: Vec<EnumValue>,
    pub(crate) docstring: Option<String>,
}

impl Enum {
    /// The name of the type.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The underlying D-Bus type.
    pub fn ty(&self) -> &Signature {
        &self.ty
    }

    /// The values of the enumeration.
    pub fn values(&self) -> &[EnumValue] {
        &self.values
    }

    /// The docstring of the definition, if any.
    pub fn docstring(&self) -> Option<&str> {
        self.docstring.as_deref()
    }
}

/// A single value of an [`Enum`] (`<tp:enumvalue>`).
#[derive(Debug, Clone, PartialEq)]
pub struct EnumValue {
    pub(crate) suffix: String,
    pub(crate) value: String,
    pub(crate) docstring: Option<String>,
}

impl EnumValue {
    /// The name of the value.
    pub fn suffix(&self) -> &str {
        &self.suffix
    }

    /// The value itself — a number for numeric enumerations, or e. g. a string.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The docstring of the value, if any.
    pub fn docstring(&self) -> Option<&str> {
        self.docstring.as_deref()
    }
}

/// A named structure (`<tp:struct>`).
#[derive(Debug, Clone, PartialEq)]
pub struct Struct {
    pub(crate) name: String,
    pub(crate) members: Vec<Member>,
    pub(crate) docstring: Option<String>,
}

impl Struct {
    /// The name of the type.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The members of the structure.
    pub fn members(&self) -> &[Member] {
        &self.members
    }

    /// The docstring of the definition, if any.
    pub fn docstring(&self) -> Option<&str> {
        self.docstring.as_deref()
    }

    /// The D-Bus signature of the structure.
    pub fn signature(&self) -> zvariant::Signature {
        zvariant::Signature::structure(
            self.members
                .iter()
                .map(|m| m.ty().inner().clone())
                .collect::<Vec<_>>(),
        )
    }
}

/// A member of a [`Struct`] or [`Mapping`] (`<tp:member>`).
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub(crate) name: String,
    pub(crate) ty: Signature,
    pub(crate) tp_type: Option<String>,
    pub(crate) docstring: Option<String>,
}

impl Member {
    /// The name of the member.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The D-Bus type of the member.
    pub fn ty(&self) -> &Signature {
        &self.ty
    }

    /// The named Telepathy type of the member (its `tp:type` attribute), if any.
    pub fn tp_type(&self) -> Option<&str> {
        self.tp_type.as_deref()
    }

    /// The docstring of the member, if any.
    pub fn docstring(&self) -> Option<&str> {
        self.docstring.as_deref()
    }
}

/// A named dictionary type (`<tp:mapping>`).
#[derive(Debug, Clone, PartialEq)]
pub struct Mapping {
    pub(crate) name: String,
    pub(crate) key: Member,
    pub(crate) value: Member,
    pub(crate) docstring: Option<String>,
}

impl Mapping {
    /// The name of the type.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The key member of the dictionary.
    pub fn key(&self) -> &Member {
        &self.key
    }

    /// The value member of the dictionary.
    pub fn value(&self) -> &Member {
        &self.value
    }

    /// The docstring of the definition, if any.
    pub fn docstring(&self) -> Option<&str> {
        self.docstring.as_deref()
    }

    /// The D-Bus signature of the dictionary.
    pub fn signature(&self) -> zvariant::Signature {
        zvariant::Signature::dict(
            self.key.ty().inner().clone(),
            self.value.ty().inner().clone(),
        )
    }
}

/// Whether `ty` is a basic (i. e. non-container) D-Bus type, as required for dictionary keys.
pub(crate) fn is_basic(ty: &Signature) -> bool {
    use zvariant::Signature as S;

    match ty.inner() {
        S::U8
        | S::Bool
        | S::I16
        | S::U16
        | S::I32
        | S::U32
        | S::I64
        | S::U64
        | S::F64
        | S::Str
        | S::Signature
        | S::ObjectPath => true,
        #[cfg(unix)]
        S::Fd => true,
        _ => false,
    }
}
