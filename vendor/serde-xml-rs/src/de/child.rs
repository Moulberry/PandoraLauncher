use super::{
    map::MapAccess,
    plain::PlainTextDeserializer,
    reader::{ChildReader, Event, Reader},
    seq::SeqAccess,
    var::EnumAccess,
};
use crate::error::{Error, Result};
use log::trace;
use serde::de::Visitor;
use std::io::Read;

pub struct ChildDeserializer<'a, R: Read> {
    reader: ChildReader<'a, R>,
    element_name: Option<String>,
}

impl<'a, R: Read> ChildDeserializer<'a, R> {
    pub fn new(reader: ChildReader<'a, R>) -> Self {
        Self {
            reader,
            element_name: None,
        }
    }

    pub fn new_with_element_name(reader: ChildReader<'a, R>, element_name: String) -> Self {
        Self {
            reader,
            element_name: Some(element_name),
        }
    }

    pub fn maybe_start_element(&mut self) -> Result<()> {
        if self.element_name.is_some() {
            self.reader.start_element()?;
        }
        Ok(())
    }

    pub fn maybe_end_element(&mut self) -> Result<()> {
        if self.element_name.is_some() {
            self.reader.end_element()?;
        }
        Ok(())
    }
}

macro_rules! deserialize_type {
    ($deserialize:ident => $visit:ident) => {
        fn $deserialize<V: ::serde::de::Visitor<'de>>(mut self, visitor: V) -> Result<V::Value> {
            trace!("{}", stringify!($deserialize:ident));
            self.maybe_start_element()?;
            let value = self.reader.chars()?.parse()?;
            self.maybe_end_element()?;
            visitor.$visit(value)
        }
    };
}

impl<'de, R: Read> serde::Deserializer<'de> for ChildDeserializer<'_, R> {
    type Error = Error;

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::Unsupported("deserialize any"))
    }

    fn deserialize_bool<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        trace!("bool");
        self.maybe_start_element()?;
        let value = self.reader.bool()?;
        self.maybe_end_element()?;
        visitor.visit_bool(value)
    }

    deserialize_type!(deserialize_i8 => visit_i8);
    deserialize_type!(deserialize_i16 => visit_i16);
    deserialize_type!(deserialize_i32 => visit_i32);
    deserialize_type!(deserialize_i64 => visit_i64);
    deserialize_type!(deserialize_u8 => visit_u8);
    deserialize_type!(deserialize_u16 => visit_u16);
    deserialize_type!(deserialize_u32 => visit_u32);
    deserialize_type!(deserialize_u64 => visit_u64);
    deserialize_type!(deserialize_f32 => visit_f32);
    deserialize_type!(deserialize_f64 => visit_f64);

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        trace!("char");
        self.deserialize_string(visitor)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        trace!("str");
        self.deserialize_string(visitor)
    }

    fn deserialize_string<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        trace!("string");
        self.maybe_start_element()?;
        let text = if matches!(self.reader.peek()?, Event::Text(_)) {
            self.reader.chars()?
        } else {
            "".to_string()
        };
        let value = visitor.visit_string::<Self::Error>(text)?;
        self.maybe_end_element()?;
        Ok(value)
    }

    fn deserialize_bytes<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::Unsupported("bytes"))
    }

    fn deserialize_byte_buf<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        trace!("byte buffer");
        self.maybe_start_element()?;
        let value = visitor.visit_byte_buf::<Self::Error>(self.reader.chars()?.into_bytes())?;
        self.maybe_end_element()?;
        Ok(value)
    }

    fn deserialize_option<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        trace!("option");
        if self.element_name.is_some() {
            if matches!(self.reader.peek()?, Event::StartElement(_)) {
                visitor.visit_some(self)
            } else {
                visitor.visit_none()
            }
        } else if matches!(self.reader.peek()?, Event::EndElement) {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        trace!("unit");
        if self.element_name.is_none() {
            visitor.visit_unit()
        } else {
            self.reader.start_element()?;
            let value = visitor.visit_unit::<Self::Error>()?;
            self.reader.end_element()?;
            Ok(value)
        }
    }

    fn deserialize_unit_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        trace!("unit struct '{name}'");
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        trace!("newtype struct '{name}'");

        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        trace!("sequence");
        visitor.visit_seq(SeqAccess::new(self.reader.child(), self.element_name))
    }

    fn deserialize_tuple<V>(mut self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        trace!("tuple");
        self.maybe_start_element()?;
        let text = self.reader.chars()?;
        let value = visitor.visit_seq(PlainTextDeserializer::new(&text))?;
        self.maybe_end_element()?;
        Ok(value)
    }

    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        trace!("tuple struct '{name}'");
        self.deserialize_tuple(len, visitor)
    }

    fn deserialize_map<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        trace!("map");
        self.maybe_start_element()?;
        let value = visitor.visit_map(MapAccess::new_map(self.reader.child()))?;
        self.maybe_end_element()?;
        Ok(value)
    }

    fn deserialize_struct<V>(
        mut self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        trace!("struct '{name}'");
        let element = self.reader.start_element()?;
        let value = visitor.visit_map(MapAccess::new_struct(
            self.reader.child(),
            element.attributes,
            fields,
        ))?;
        self.reader.end_element()?;
        Ok(value)
    }

    fn deserialize_enum<V>(
        mut self,
        name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        trace!("enum '{name}'");
        self.maybe_start_element()?;
        let value = visitor.visit_enum(EnumAccess::new(self.reader.child()))?;
        self.maybe_end_element()?;
        Ok(value)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        trace!("identifier");
        self.deserialize_string(visitor)
    }

    fn deserialize_ignored_any<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        trace!("ignoring {:?}", self.reader.peek()?);
        self.reader.ignore()?;
        visitor.visit_unit()
    }
}
