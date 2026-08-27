#![deny(rust_2018_idioms)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/z-galaxy/zbus/9f7a90d2b594ddc48b7a5f39fda5e00cd56a7dfb/logo.png"
)]
#![doc = include_str!("../README.md")]
#![doc(test(attr(
    warn(unused),
    deny(warnings),
    allow(dead_code),
    // W/o this, we seem to get some bogus warning about `extern crate zbus`.
    allow(unused_extern_crates),
)))]

mod error;
#[allow(deprecated)]
pub use error::{DeError, SeError};
pub use error::{Error, Result, XmlError};

mod xml;
use xml::escape;

pub mod telepathy;

use serde::{Deserialize, Serialize};
use std::{
    fmt,
    io::{BufWriter, Read, Write},
    ops::Deref,
};

use zbus_names::{InterfaceName, MemberName, PropertyName};

/// A warning about document content that was ignored during parsing.
///
/// The D-Bus introspection format is sometimes extended with elements from other vocabularies,
/// most notably the [Telepathy extensions] (`tp:enum`, `tp:struct`, …). The parser skips over
/// any element it has no use for — or understands but cannot make sense of, e. g. a Telepathy
/// type definition missing a required attribute — and records a `Warning`, which
/// [`Node::from_reader_with_warnings`] hands back to the caller.
///
/// [Telepathy extensions]: https://telepathy.freedesktop.org/spec/
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    element: String,
    position: usize,
    message: String,
}

impl Warning {
    /// A warning about a skipped element that is not part of the introspection format.
    pub(crate) fn unsupported(element: impl Into<String>, position: usize) -> Self {
        let element = element.into();
        let message = format!("unsupported element `<{element}>` ignored");
        Self {
            element,
            position,
            message,
        }
    }

    /// A warning about a skipped element that is understood but could not be parsed.
    pub(crate) fn malformed(
        element: impl Into<String>,
        position: usize,
        reason: impl fmt::Display,
    ) -> Self {
        let element = element.into();
        let message = format!("malformed element `<{element}>` ignored: {reason}");
        Self {
            element,
            position,
            message,
        }
    }

    /// The name of the element that was ignored.
    pub fn element(&self) -> &str {
        &self.element
    }

    /// The byte offset in the document at which the element starts.
    pub fn position(&self) -> usize {
        self.position
    }

    /// A message describing what was ignored, and why.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (at byte offset {})", self.message, self.position)
    }
}

/// Annotations are generic key/value pairs of metadata.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Annotation {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@value")]
    value: String,
}

impl Annotation {
    /// Return the annotation name/key.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the annotation value.
    pub fn value(&self) -> &str {
        &self.value
    }

    fn write_xml<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        write!(
            w,
            "<annotation name=\"{}\" value=\"{}\"/>",
            escape(&self.name),
            escape(&self.value)
        )
    }
}

/// A direction of an argument
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ArgDirection {
    #[serde(rename = "in")]
    In,
    #[serde(rename = "out")]
    Out,
}

impl ArgDirection {
    fn xml_value(&self) -> &'static str {
        match self {
            ArgDirection::In => "in",
            ArgDirection::Out => "out",
        }
    }
}

/// An argument
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct Arg {
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "@type")]
    ty: Signature,
    #[serde(rename = "@direction")]
    direction: Option<ArgDirection>,
    #[serde(rename = "annotation", default)]
    annotations: Vec<Annotation>,
    #[serde(skip)]
    docstring: Option<String>,
    #[serde(skip)]
    tp_type: Option<String>,
}

impl Arg {
    /// Return the argument name, if any.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Return the argument type.
    pub fn ty(&self) -> &Signature {
        &self.ty
    }

    /// Return the argument direction, if any.
    pub fn direction(&self) -> Option<ArgDirection> {
        self.direction
    }

    /// Return the associated annotations.
    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    /// Return the content of the Telepathy `tp:docstring` extension element, if any.
    ///
    /// The content — typically HTML — is returned as it appears in the document, with the
    /// surrounding whitespace trimmed. Note that docstrings are only captured when parsing;
    /// the writer does not emit them.
    pub fn docstring(&self) -> Option<&str> {
        self.docstring.as_deref()
    }

    /// Return the named Telepathy type of the argument (its `tp:type` attribute), if any.
    ///
    /// The name refers to a [type definition](telepathy::TypeDef) in scope, with one `[]`
    /// suffix per level of array nesting (e. g. `Playlist[]`).
    pub fn tp_type(&self) -> Option<&str> {
        self.tp_type.as_deref()
    }

    fn write_xml<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        write!(w, "<arg")?;
        if let Some(name) = &self.name {
            write!(w, " name=\"{}\"", escape(name))?;
        }
        write!(w, " type=\"{}\"", escape(&self.ty.to_string()))?;
        if let Some(direction) = self.direction {
            write!(w, " direction=\"{}\"", direction.xml_value())?;
        }
        if self.annotations.is_empty() {
            return write!(w, "/>");
        }
        write!(w, ">")?;
        for annotation in &self.annotations {
            annotation.write_xml(w)?;
        }
        write!(w, "</arg>")
    }
}

/// A method
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct Method<'a> {
    #[serde(rename = "@name", borrow)]
    name: MemberName<'a>,
    #[serde(rename = "arg", default)]
    args: Vec<Arg>,
    #[serde(rename = "annotation", default)]
    annotations: Vec<Annotation>,
    #[serde(skip)]
    docstring: Option<String>,
}

impl Method<'_> {
    /// Return the method name.
    pub fn name(&self) -> MemberName<'_> {
        self.name.as_ref()
    }

    /// Return the method arguments.
    pub fn args(&self) -> &[Arg] {
        &self.args
    }

    /// Return the method annotations.
    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    /// Return the content of the Telepathy `tp:docstring` extension element, if any.
    ///
    /// The content — typically HTML — is returned as it appears in the document, with the
    /// surrounding whitespace trimmed. Note that docstrings are only captured when parsing;
    /// the writer does not emit them.
    pub fn docstring(&self) -> Option<&str> {
        self.docstring.as_deref()
    }

    fn write_xml<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        write!(w, "<method name=\"{}\"", escape(self.name.as_str()))?;
        if self.args.is_empty() && self.annotations.is_empty() {
            return write!(w, "/>");
        }
        write!(w, ">")?;
        for arg in &self.args {
            arg.write_xml(w)?;
        }
        for annotation in &self.annotations {
            annotation.write_xml(w)?;
        }
        write!(w, "</method>")
    }
}

/// A signal
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct Signal<'a> {
    #[serde(rename = "@name", borrow)]
    name: MemberName<'a>,

    #[serde(rename = "arg", default)]
    args: Vec<Arg>,
    #[serde(rename = "annotation", default)]
    annotations: Vec<Annotation>,
    #[serde(skip)]
    docstring: Option<String>,
}

impl Signal<'_> {
    /// Return the signal name.
    pub fn name(&self) -> MemberName<'_> {
        self.name.as_ref()
    }

    /// Return the signal arguments.
    pub fn args(&self) -> &[Arg] {
        &self.args
    }

    /// Return the signal annotations.
    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    /// Return the content of the Telepathy `tp:docstring` extension element, if any.
    ///
    /// The content — typically HTML — is returned as it appears in the document, with the
    /// surrounding whitespace trimmed. Note that docstrings are only captured when parsing;
    /// the writer does not emit them.
    pub fn docstring(&self) -> Option<&str> {
        self.docstring.as_deref()
    }

    fn write_xml<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        write!(w, "<signal name=\"{}\"", escape(self.name.as_str()))?;
        if self.args.is_empty() && self.annotations.is_empty() {
            return write!(w, "/>");
        }
        write!(w, ">")?;
        for arg in &self.args {
            arg.write_xml(w)?;
        }
        for annotation in &self.annotations {
            annotation.write_xml(w)?;
        }
        write!(w, "</signal>")
    }
}

/// The possible property access types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropertyAccess {
    #[serde(rename = "read")]
    Read,
    #[serde(rename = "write")]
    Write,
    #[serde(rename = "readwrite")]
    ReadWrite,
}

impl PropertyAccess {
    pub fn read(&self) -> bool {
        matches!(self, PropertyAccess::Read | PropertyAccess::ReadWrite)
    }

    pub fn write(&self) -> bool {
        matches!(self, PropertyAccess::Write | PropertyAccess::ReadWrite)
    }

    fn xml_value(&self) -> &'static str {
        match self {
            PropertyAccess::Read => "read",
            PropertyAccess::Write => "write",
            PropertyAccess::ReadWrite => "readwrite",
        }
    }
}

/// A property
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct Property<'a> {
    #[serde(rename = "@name", borrow)]
    name: PropertyName<'a>,

    #[serde(rename = "@type")]
    ty: Signature,
    #[serde(rename = "@access")]
    access: PropertyAccess,

    #[serde(rename = "annotation", default)]
    annotations: Vec<Annotation>,
    #[serde(skip)]
    docstring: Option<String>,
    #[serde(skip)]
    tp_type: Option<String>,
}

impl Property<'_> {
    /// Returns the property name.
    pub fn name(&self) -> PropertyName<'_> {
        self.name.as_ref()
    }

    /// Returns the property type.
    pub fn ty(&self) -> &Signature {
        &self.ty
    }

    /// Returns the property access flags (should be "read", "write" or "readwrite").
    pub fn access(&self) -> PropertyAccess {
        self.access
    }

    /// Return the associated annotations.
    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    /// Return the content of the Telepathy `tp:docstring` extension element, if any.
    ///
    /// The content — typically HTML — is returned as it appears in the document, with the
    /// surrounding whitespace trimmed. Note that docstrings are only captured when parsing;
    /// the writer does not emit them.
    pub fn docstring(&self) -> Option<&str> {
        self.docstring.as_deref()
    }

    /// Return the named Telepathy type of the property (its `tp:type` attribute), if any.
    ///
    /// The name refers to a [type definition](telepathy::TypeDef) in scope, with one `[]`
    /// suffix per level of array nesting (e. g. `Playlist[]`).
    pub fn tp_type(&self) -> Option<&str> {
        self.tp_type.as_deref()
    }

    fn write_xml<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        write!(
            w,
            "<property name=\"{}\" type=\"{}\" access=\"{}\"",
            escape(self.name.as_str()),
            escape(&self.ty.to_string()),
            self.access.xml_value()
        )?;
        if self.annotations.is_empty() {
            return write!(w, "/>");
        }
        write!(w, ">")?;
        for annotation in &self.annotations {
            annotation.write_xml(w)?;
        }
        write!(w, "</property>")
    }
}

/// An interface
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct Interface<'a> {
    #[serde(rename = "@name", borrow)]
    name: InterfaceName<'a>,

    #[serde(rename = "method", default)]
    methods: Vec<Method<'a>>,
    #[serde(rename = "property", default)]
    properties: Vec<Property<'a>>,
    #[serde(rename = "signal", default)]
    signals: Vec<Signal<'a>>,
    #[serde(rename = "annotation", default)]
    annotations: Vec<Annotation>,
    #[serde(skip)]
    docstring: Option<String>,
    #[serde(skip)]
    telepathy_types: Vec<telepathy::TypeDef>,
}

impl<'a> Interface<'a> {
    /// Returns the interface name.
    pub fn name(&self) -> InterfaceName<'_> {
        self.name.as_ref()
    }

    /// Returns the interface methods.
    pub fn methods(&self) -> &[Method<'a>] {
        &self.methods
    }

    /// Returns the interface signals.
    pub fn signals(&self) -> &[Signal<'a>] {
        &self.signals
    }

    /// Returns the interface properties.
    pub fn properties(&self) -> &[Property<'_>] {
        &self.properties
    }

    /// Return the associated annotations.
    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    /// Return the content of the Telepathy `tp:docstring` extension element, if any.
    ///
    /// The content — typically HTML — is returned as it appears in the document, with the
    /// surrounding whitespace trimmed. Note that docstrings are only captured when parsing;
    /// the writer does not emit them.
    pub fn docstring(&self) -> Option<&str> {
        self.docstring.as_deref()
    }

    /// Return the Telepathy type definitions on this interface.
    pub fn telepathy_types(&self) -> &[telepathy::TypeDef] {
        &self.telepathy_types
    }

    fn write_xml<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        write!(w, "<interface name=\"{}\"", escape(self.name.as_str()))?;
        if self.methods.is_empty()
            && self.properties.is_empty()
            && self.signals.is_empty()
            && self.annotations.is_empty()
        {
            return write!(w, "/>");
        }
        write!(w, ">")?;
        for method in &self.methods {
            method.write_xml(w)?;
        }
        for property in &self.properties {
            property.write_xml(w)?;
        }
        for signal in &self.signals {
            signal.write_xml(w)?;
        }
        for annotation in &self.annotations {
            annotation.write_xml(w)?;
        }
        write!(w, "</interface>")
    }
}

/// An introspection tree node (typically the root of the XML document).
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct Node<'a> {
    #[serde(rename = "@name")]
    name: Option<String>,

    #[serde(rename = "interface", default, borrow)]
    interfaces: Vec<Interface<'a>>,
    #[serde(rename = "node", default, borrow)]
    nodes: Vec<Node<'a>>,
    #[serde(skip)]
    docstring: Option<String>,
    #[serde(skip)]
    telepathy_types: Vec<telepathy::TypeDef>,
}

impl<'a> Node<'a> {
    /// Parse the introspection XML document from reader.
    ///
    /// Note that `reader` is consumed until end-of-stream before parsing, so this must not be
    /// used with a reader that stays open past the end of the document (e.g. a socket).
    pub fn from_reader<R: Read>(reader: R) -> Result<Node<'a>> {
        Ok(Node::from_reader_with_warnings(reader)?.0)
    }

    /// Parse the introspection XML document from reader, collecting warnings.
    ///
    /// In addition to the parsed node, a [`Warning`] is returned for every element that is not
    /// part of the [introspection format] (except Telepathy docstrings, which are captured —
    /// see [`Interface::docstring`]) and was therefore ignored, e. g. the type-definition
    /// elements of the Telepathy extensions (`tp:enum`, `tp:struct`, …).
    ///
    /// Note that `reader` is consumed until end-of-stream before parsing, so this must not be
    /// used with a reader that stays open past the end of the document (e.g. a socket).
    ///
    /// [introspection format]: https://dbus.freedesktop.org/doc/dbus-specification.html#introspection-format
    pub fn from_reader_with_warnings<R: Read>(mut reader: R) -> Result<(Node<'a>, Vec<Warning>)> {
        let mut input = String::new();
        reader.read_to_string(&mut input)?;

        xml::parse_with_warnings(&input)
    }

    /// Write the XML document to writer.
    ///
    /// Note that data which is only captured when parsing — Telepathy docstrings, type
    /// definitions and `tp:type` references — is not written. Consequently, a document that
    /// carried any does not compare equal to its written-and-reparsed self.
    pub fn to_writer<W: Write>(&self, writer: W) -> Result<()> {
        let mut writer = BufWriter::new(writer);
        self.write_xml(&mut writer)?;
        writer.flush()?;

        Ok(())
    }

    /// Returns the node name, if any.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns the children nodes.
    pub fn nodes(&self) -> &[Node<'a>] {
        &self.nodes
    }

    /// Returns the interfaces on this node.
    pub fn interfaces(&self) -> &[Interface<'a>] {
        &self.interfaces
    }

    /// Return the content of the Telepathy `tp:docstring` extension element, if any.
    ///
    /// The content — typically HTML — is returned as it appears in the document, with the
    /// surrounding whitespace trimmed. Note that docstrings are only captured when parsing;
    /// the writer does not emit them.
    pub fn docstring(&self) -> Option<&str> {
        self.docstring.as_deref()
    }

    /// Return the Telepathy type definitions on this node.
    pub fn telepathy_types(&self) -> &[telepathy::TypeDef] {
        &self.telepathy_types
    }

    fn write_xml<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        write!(w, "<node")?;
        if let Some(name) = &self.name {
            write!(w, " name=\"{}\"", escape(name))?;
        }
        if self.interfaces.is_empty() && self.nodes.is_empty() {
            return write!(w, "/>");
        }
        write!(w, ">")?;
        for interface in &self.interfaces {
            interface.write_xml(w)?;
        }
        for node in &self.nodes {
            node.write_xml(w)?;
        }
        write!(w, "</node>")
    }
}

impl<'a> TryFrom<&'a str> for Node<'a> {
    type Error = Error;

    /// Parse the introspection XML document from `s`.
    fn try_from(s: &'a str) -> Result<Node<'a>> {
        xml::parse(s)
    }
}

/// A thin wrapper around `zvariant::parsed::Signature`.
///
/// This is to allow `Signature` to be deserialized from an owned string, which is what XML
/// deserializers typically produce.
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct Signature(zvariant::Signature);

impl Signature {
    /// Return the inner `zvariant::Signature`.
    pub fn inner(&self) -> &zvariant::Signature {
        &self.0
    }

    /// Convert this `Signature` into the inner `zvariant::parsed::Signature`.
    pub fn into_inner(self) -> zvariant::Signature {
        self.0
    }
}

impl<'de> serde::de::Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        String::deserialize(deserializer).and_then(|s| {
            zvariant::Signature::try_from(s.as_bytes())
                .map_err(serde::de::Error::custom)
                .map(Signature)
        })
    }
}

impl Deref for Signature {
    type Target = zvariant::Signature;

    fn deref(&self) -> &Self::Target {
        self.inner()
    }
}

impl PartialEq<str> for Signature {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}
