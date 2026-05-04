use super::{
    map::{MapSerializer, StructSerializer},
    seq::SequenceSerializer,
    tuple::TupleSerializer,
    writer::Writer,
};
use crate::error::{Error, Result};
use serde::Serialize;
use std::io::Write;

pub struct ChildSerializer<'a, W> {
    writer: &'a mut Writer<W>,
    element_name: Option<String>,
    newtype_struct: bool,
}

impl<'a, W: 'a + Write> ChildSerializer<'a, W> {
    pub fn new(writer: &'a mut Writer<W>, element_name: Option<String>) -> Self {
        Self {
            writer,
            element_name,
            newtype_struct: false,
        }
    }

    pub fn for_newtype_struct(writer: &'a mut Writer<W>, element_name: String) -> Self {
        Self {
            writer,
            element_name: Some(element_name),
            newtype_struct: true,
        }
    }

    fn maybe_start_element(&mut self) -> Result<bool> {
        if let Some(element_name) = &self.element_name {
            self.writer.start_element(element_name)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn maybe_end_element(&mut self) -> Result<()> {
        if self.element_name.is_some() {
            self.writer.end_element()?;
        }
        Ok(())
    }
}

impl<'a, W: Write> serde::ser::Serializer for ChildSerializer<'a, W> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SequenceSerializer<'a, W>;
    type SerializeTuple = TupleSerializer<'a, W>;
    type SerializeTupleStruct = TupleSerializer<'a, W>;
    type SerializeTupleVariant = TupleSerializer<'a, W>;
    type SerializeMap = MapSerializer<'a, W>;
    type SerializeStruct = StructSerializer<'a, W>;
    type SerializeStructVariant = StructSerializer<'a, W>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(mut self, v: &str) -> Result<Self::Ok> {
        self.maybe_start_element()?;
        self.writer.characters(v)?;
        self.maybe_end_element()?;
        Ok(())
    }

    fn serialize_bytes(mut self, v: &[u8]) -> Result<Self::Ok> {
        self.maybe_start_element()?;
        self.writer.characters(String::from_utf8(v.to_vec())?)?;
        self.maybe_end_element()?;
        Ok(())
    }

    fn serialize_none(self) -> Result<Self::Ok> {
        if self.newtype_struct {
            self.serialize_unit()
        } else {
            Ok(())
        }
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(mut self) -> Result<Self::Ok> {
        self.maybe_start_element()?;
        self.maybe_end_element()?;
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        mut self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok> {
        self.maybe_start_element()?;
        self.writer.start_element(variant)?;
        self.writer.end_element()?;
        self.maybe_end_element()?;
        Ok(())
    }

    fn serialize_newtype_struct<T>(mut self, _name: &'static str, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        self.newtype_struct = true;
        value.serialize(self)?;
        Ok(())
    }

    fn serialize_newtype_variant<T>(
        mut self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        self.maybe_start_element()?;
        value.serialize(ChildSerializer::new(self.writer, Some(variant.to_string())))?;
        self.maybe_end_element()?;
        Ok(())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        if self.newtype_struct {
            Ok(SequenceSerializer::new(self.writer, None))
        } else {
            Ok(SequenceSerializer::new(self.writer, self.element_name))
        }
    }

    fn serialize_tuple(mut self, _len: usize) -> Result<Self::SerializeTuple> {
        let should_end_element = self.maybe_start_element()?;
        Ok(TupleSerializer::new(self.writer, should_end_element))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_tuple(len)
    }

    fn serialize_tuple_variant(
        mut self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        let should_end_element = self.maybe_start_element()?;
        self.writer.start_element(variant)?;
        Ok(TupleSerializer::new(self.writer, should_end_element))
    }

    fn serialize_map(mut self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        let should_end_element = self.maybe_start_element()?;
        Ok(MapSerializer::new(self.writer, should_end_element))
    }

    fn serialize_struct(self, name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Ok(StructSerializer::new(
            self.writer,
            self.element_name.clone().unwrap_or(name.to_string()),
        ))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Ok(StructSerializer::new_variant(
            self.writer,
            self.element_name.clone(),
            variant.to_string(),
        ))
    }
}
