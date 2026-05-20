use crate::error::{Error, Result};
use serde::Serializer;

pub struct PlainTextSerializer;

impl Serializer for PlainTextSerializer {
    type Ok = Option<String>;
    type Error = Error;

    type SerializeSeq = PlainTextSeqSerializer;
    type SerializeTuple = PlainTextSeqSerializer;
    type SerializeTupleStruct = PlainTextSeqSerializer;
    type SerializeTupleVariant = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeMap = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = serde::ser::Impossible<Self::Ok, Self::Error>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok> {
        Ok(Some(v.to_string()))
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok> {
        Ok(Some(v.to_string()))
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok> {
        Ok(Some(v.to_string()))
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok> {
        Ok(Some(v.to_string()))
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok> {
        Ok(Some(v.to_string()))
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok> {
        Ok(Some(v.to_string()))
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok> {
        Ok(Some(v.to_string()))
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok> {
        Ok(Some(v.to_string()))
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok> {
        Ok(Some(v.to_string()))
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok> {
        Ok(Some(v.to_string()))
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok> {
        Ok(Some(v.to_string()))
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok> {
        Ok(Some(v.to_string()))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok> {
        Ok(Some(v.to_string()))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok> {
        let s = String::from_utf8(v.to_vec())?;
        Ok(Some(s))
    }

    fn serialize_none(self) -> Result<Self::Ok> {
        Ok(None)
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok> {
        Ok(Some("".to_string()))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok> {
        Ok(Some("".to_string()))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok> {
        Ok(Some(variant.to_string()))
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok>
    where
        T: ?Sized + serde::Serialize,
    {
        Err(Error::Unsupported("newtype variant in text"))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Ok(PlainTextSeqSerializer::new())
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Ok(PlainTextSeqSerializer::new())
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Ok(PlainTextSeqSerializer::new())
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::Unsupported("tuple variant in text"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(Error::Unsupported("map in text"))
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Err(Error::Unsupported("struct in text"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(Error::Unsupported("struct variant in text"))
    }
}

pub struct PlainTextSeqSerializer {
    buffer: Vec<String>,
}

impl PlainTextSeqSerializer {
    fn new() -> Self {
        Self { buffer: Vec::new() }
    }
}

impl serde::ser::SerializeSeq for PlainTextSeqSerializer {
    type Ok = Option<String>;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + serde::Serialize,
    {
        if let Some(value) = value.serialize(PlainTextSerializer)? {
            self.buffer.push(value);
        }
        Ok(())
    }

    fn end(self) -> Result<Self::Ok> {
        Ok(Some(self.buffer.join(" ")))
    }
}

impl serde::ser::SerializeTuple for PlainTextSeqSerializer {
    type Ok = Option<String>;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + serde::Serialize,
    {
        <Self as serde::ser::SerializeSeq>::serialize_element(self, value)
    }

    fn end(self) -> std::result::Result<Self::Ok, Self::Error> {
        <Self as serde::ser::SerializeSeq>::end(self)
    }
}

impl serde::ser::SerializeTupleStruct for PlainTextSeqSerializer {
    type Ok = Option<String>;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + serde::Serialize,
    {
        <Self as serde::ser::SerializeSeq>::serialize_element(self, value)
    }

    fn end(self) -> std::result::Result<Self::Ok, Self::Error> {
        <Self as serde::ser::SerializeSeq>::end(self)
    }
}
