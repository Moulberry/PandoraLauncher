mod child;
mod map;
mod plain;
mod seq;
mod tuple;
mod writer;

use self::{child::ChildSerializer, map::StructSerializer, tuple::TupleSerializer};
use crate::{
    config::{Namespaces, SerdeXml},
    error::{Error, Result},
};
use log::trace;
use serde::Serialize;
use std::io::Write;
use writer::Writer;
use xml::EventWriter;

/// A convenience method for serializing some object to a string.
///
/// # Examples
///
/// ```rust
/// # use serde::Serialize;
/// # use serde_xml_rs::to_string;
/// #[derive(Serialize)]
/// struct Person {
///   name: String,
///   age: u32,
/// }
///
/// # fn main() {
///
/// let joe = Person {name: "Joe".to_string(), age: 42};
/// let serialized = to_string(&joe).unwrap();
/// println!("{}", serialized);
/// # }
/// ```
pub fn to_string<S: Serialize>(value: &S) -> Result<String> {
    let mut buffer = Vec::new();
    to_writer(&mut buffer, value)?;
    Ok(String::from_utf8(buffer)?)
}

/// A convenience method for serializing some object to a buffer.
///
/// # Examples
///
/// ```rust
/// # use serde::Serialize;
/// # use serde_xml_rs::to_writer;
/// #[derive(Serialize)]
/// struct Person {
///   name: String,
///   age: u32,
/// }
///
/// # fn main() {
/// let mut buffer = Vec::new();
/// let joe = Person {name: "Joe".to_string(), age: 42};
///
/// to_writer(&mut buffer, &joe).unwrap();
///
/// let serialized = String::from_utf8(buffer).unwrap();
/// println!("{}", serialized);
/// # }
/// ```
pub fn to_writer<W: Write, S: Serialize>(writer: W, value: &S) -> Result<()> {
    let mut serializer = Serializer::from_config(SerdeXml::default(), writer);
    value.serialize(&mut serializer)
}

/// An XML `Serializer`.
pub struct Serializer<W> {
    writer: Writer<W>,
}

impl<W: Write> Serializer<W> {
    pub fn new(writer: EventWriter<W>) -> Self {
        Self {
            writer: Writer::new(writer, Namespaces::default()),
        }
    }

    pub fn new_from_writer(writer: W) -> Self {
        Self::from_config(SerdeXml::default(), writer)
    }

    pub fn into_inner(self) -> W {
        self.writer.into_inner()
    }

    pub(crate) fn from_config(config: SerdeXml, sink: W) -> Self {
        Self {
            writer: Writer::new(config.emitter.create_writer(sink), config.namespaces),
        }
    }
}

impl<'a, W: Write> serde::ser::Serializer for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = TupleSerializer<'a, W>;
    type SerializeTupleVariant = TupleSerializer<'a, W>;
    type SerializeMap = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = StructSerializer<'a, W>;
    type SerializeStructVariant = StructSerializer<'a, W>;

    fn serialize_bool(self, _v: bool) -> Result<Self::Ok> {
        Err(Error::Unsupported("bool in document root"))
    }

    fn serialize_i8(self, _v: i8) -> Result<Self::Ok> {
        Err(Error::Unsupported("integer in document root"))
    }

    fn serialize_i16(self, _v: i16) -> Result<Self::Ok> {
        Err(Error::Unsupported("integer in document root"))
    }

    fn serialize_i32(self, _v: i32) -> Result<Self::Ok> {
        Err(Error::Unsupported("integer in document root"))
    }

    fn serialize_i64(self, _v: i64) -> Result<Self::Ok> {
        Err(Error::Unsupported("integer in document root"))
    }

    fn serialize_u8(self, _v: u8) -> Result<Self::Ok> {
        Err(Error::Unsupported("integer in document root"))
    }

    fn serialize_u16(self, _v: u16) -> Result<Self::Ok> {
        Err(Error::Unsupported("integer in document root"))
    }

    fn serialize_u32(self, _v: u32) -> Result<Self::Ok> {
        Err(Error::Unsupported("integer in document root"))
    }

    fn serialize_u64(self, _v: u64) -> Result<Self::Ok> {
        Err(Error::Unsupported("integer in document root"))
    }

    fn serialize_f32(self, _v: f32) -> Result<Self::Ok> {
        Err(Error::Unsupported("float in document root"))
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok> {
        Err(Error::Unsupported("float in document root"))
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok> {
        Err(Error::Unsupported("char in document root"))
    }

    fn serialize_str(self, _v: &str) -> Result<Self::Ok> {
        Err(Error::Unsupported("string in document root"))
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok> {
        Err(Error::Unsupported("bytes"))
    }

    fn serialize_none(self) -> Result<Self::Ok> {
        trace!("none");
        Ok(())
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        trace!("some");
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok> {
        Err(Error::Unsupported("unit in document root"))
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok> {
        trace!("unit struct '{name}'");
        self.writer.start_element(name)?;
        self.writer.end_element()?;
        Ok(())
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok> {
        trace!("unit variant '{name}' '{variant}'");
        self.writer.start_element(name)?;
        self.writer.start_element(variant)?;
        self.writer.end_element()?;
        self.writer.end_element()?;
        Ok(())
    }

    fn serialize_newtype_struct<T>(self, name: &'static str, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        trace!("newtype struct '{name}'");
        value.serialize(ChildSerializer::for_newtype_struct(
            &mut self.writer,
            name.to_string(),
        ))?;
        Ok(())
    }

    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        trace!("newtype variant '{name}' '{variant}'");
        self.writer.start_element(name)?;
        value.serialize(ChildSerializer::new(
            &mut self.writer,
            Some(variant.to_string()),
        ))?;
        self.writer.end_element()?;
        Ok(())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(Error::Unsupported("sequence in document root"))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Err(Error::Unsupported("tuple in document root"))
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        trace!("tuple struct '{name}'");
        self.writer.start_element(name)?;
        Ok(TupleSerializer::new(&mut self.writer, true))
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        trace!("tuple variant '{name}' '{variant}'");
        self.writer.start_element(name)?;
        self.writer.start_element(variant)?;
        Ok(TupleSerializer::new(&mut self.writer, true))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(Error::Unsupported("map in document root"))
    }

    fn serialize_struct(self, name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        trace!("struct '{name}'");
        Ok(StructSerializer::new(&mut self.writer, name.to_string()))
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        trace!("struct variant '{name}' '{variant}'");
        Ok(StructSerializer::new_variant(
            &mut self.writer,
            Some(name.to_string()),
            variant.to_string(),
        ))
    }
}
