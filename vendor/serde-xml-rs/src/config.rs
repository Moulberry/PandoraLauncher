use crate::{error::Result, Deserializer, Serializer};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
};
use xml::{
    namespace::NS_NO_PREFIX, writer::events::StartElementBuilder, EmitterConfig, ParserConfig,
};

pub const TEXT: &str = "#text";
pub const CONTENT: &str = "#content";

#[derive(Clone, Debug)]
pub struct SerdeXml {
    pub(crate) emitter: EmitterConfig,
    pub(crate) parser: ParserConfig,
    pub(crate) namespaces: Namespaces,
    pub(crate) overlapping_sequences: bool,
}

impl Default for SerdeXml {
    fn default() -> Self {
        Self {
            emitter: Default::default(),
            parser: ParserConfig::new()
                .trim_whitespace(true)
                .whitespace_to_characters(true)
                .cdata_to_characters(true)
                .ignore_comments(true)
                .coalesce_characters(true),
            namespaces: Default::default(),
            overlapping_sequences: false,
        }
    }
}

impl SerdeXml {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn emitter(mut self, emitter: EmitterConfig) -> Self {
        self.emitter = emitter;
        self
    }

    pub fn parser(mut self, parser: ParserConfig) -> Self {
        self.parser = parser;
        self
    }

    pub fn default_namespace<S: ToString>(mut self, name: S) -> Self {
        self.namespaces.put_default(name);
        self
    }

    pub fn namespace<S: ToString>(mut self, prefix: S, name: S) -> Self {
        self.namespaces.put(prefix, name);
        self
    }

    /// Configures whether the deserializer should search all sibling elements when building a
    /// sequence. Not required if all XML elements for sequences are adjacent. Disabled by
    /// default. Enabling this option may incur additional memory usage.
    ///
    /// ```rust
    /// # use serde::Deserialize;
    /// # use serde_xml_rs::from_reader;
    /// #[derive(Debug, Deserialize, PartialEq)]
    /// struct Foo {
    ///     bar: Vec<usize>,
    ///     baz: String,
    /// }
    /// # fn main() {
    /// let s = r##"
    ///     <foo>
    ///         <bar>1</bar>
    ///         <bar>2</bar>
    ///         <baz>Hello, world</baz>
    ///         <bar>3</bar>
    ///         <bar>4</bar>
    ///     </foo>
    /// "##;
    /// let foo: Foo = serde_xml_rs::SerdeXml::new().overlapping_sequences(true).from_str(s).unwrap();
    /// assert_eq!(foo, Foo { bar: vec![1, 2, 3, 4], baz: "Hello, world".to_string()});
    /// # }
    /// ```
    pub fn overlapping_sequences(mut self, b: bool) -> Self {
        self.overlapping_sequences = b;
        self
    }

    pub fn from_str<'de, T: Deserialize<'de>>(self, s: &str) -> Result<T> {
        self.from_reader(s.as_bytes())
    }

    pub fn from_reader<'de, T: Deserialize<'de>, R: Read>(self, reader: R) -> Result<T> {
        T::deserialize(&mut Deserializer::from_config(self, reader))
    }

    pub fn to_string<S: Serialize>(self, value: &S) -> Result<String> {
        let mut buffer = Vec::new();
        self.to_writer(&mut buffer, value)?;
        Ok(String::from_utf8(buffer)?)
    }

    pub fn to_writer<W, S>(self, writer: W, value: &S) -> Result<()>
    where
        W: Write,
        S: Serialize,
    {
        let mut s = Serializer::from_config(self, writer);
        value.serialize(&mut s)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Namespaces {
    mapping: BTreeMap<String, String>,
}

impl Namespaces {
    pub fn put_default<S: ToString>(&mut self, name: S) {
        self.mapping
            .insert(NS_NO_PREFIX.to_string(), name.to_string());
    }

    pub fn put<S: ToString>(&mut self, prefix: S, name: S) {
        self.mapping.insert(prefix.to_string(), name.to_string());
    }

    pub fn get<S: AsRef<str>>(&self, prefix: S) -> Option<&String> {
        self.mapping.get(prefix.as_ref())
    }

    pub(crate) fn add_to_start_element<'a>(
        &self,
        mut start_element_builder: StartElementBuilder<'a>,
    ) -> StartElementBuilder<'a> {
        for (prefix, name) in &self.mapping {
            start_element_builder = start_element_builder.ns(prefix, name);
        }
        start_element_builder
    }
}
