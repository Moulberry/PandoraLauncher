use super::{
    child::ChildDeserializer,
    map::MapAccess,
    plain::PlainTextDeserializer,
    reader::{ChildReader, Event, Reader},
};
use crate::{
    config::TEXT,
    error::{Error, Result},
};
use log::trace;
use serde::de::{value::StrDeserializer, IntoDeserializer};
use std::io::Read;

pub struct EnumAccess<'a, R: Read> {
    reader: ChildReader<'a, R>,
}

impl<'a, R: Read> EnumAccess<'a, R> {
    pub fn new(reader: ChildReader<'a, R>) -> Self {
        Self { reader }
    }
}

impl<'de, 'a, R: Read> serde::de::EnumAccess<'de> for EnumAccess<'a, R> {
    type Error = Error;
    type Variant = VariantAccess<'a, R>;

    fn variant_seed<V>(mut self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let element_name = match self.reader.peek()? {
            Event::StartElement(element) => element.qname(),
            Event::Text(_) => TEXT.to_string(),
            event => {
                return Err(Error::Unexpected {
                    expected: "start of element",
                    but_got: event.to_string(),
                });
            }
        };
        trace!("enum variant {element_name}");
        let name = seed.deserialize::<StrDeserializer<Self::Error>>(
            element_name.as_str().into_deserializer(),
        )?;
        Ok((name, VariantAccess::new(self.reader, element_name)))
    }
}

pub struct VariantAccess<'a, R: Read> {
    reader: ChildReader<'a, R>,
    element_name: String,
}

impl<'a, R: Read> VariantAccess<'a, R> {
    pub fn new(reader: ChildReader<'a, R>, element_name: String) -> Self {
        Self {
            reader,
            element_name,
        }
    }
}

impl<'de, R: Read> serde::de::VariantAccess<'de> for VariantAccess<'_, R> {
    type Error = Error;

    fn unit_variant(mut self) -> Result<()> {
        trace!("unit variant");
        self.reader.start_element()?;
        self.reader.end_element()?;
        Ok(())
    }

    fn newtype_variant_seed<T>(mut self, seed: T) -> Result<T::Value>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        trace!("newtype variant");
        if self.element_name == TEXT {
            seed.deserialize(PlainTextDeserializer::new(&self.reader.chars()?))
        } else {
            seed.deserialize(ChildDeserializer::new_with_element_name(
                self.reader,
                self.element_name,
            ))
        }
    }

    fn tuple_variant<V>(mut self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        trace!("tuple variant");
        self.reader.start_element()?;
        let text = self.reader.chars()?;
        let value = visitor.visit_seq(PlainTextDeserializer::new(&text))?;
        self.reader.end_element()?;
        Ok(value)
    }

    fn struct_variant<V>(mut self, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        trace!("struct variant");
        let element = self.reader.start_element()?;
        let value = visitor.visit_map(MapAccess::new_struct(
            self.reader.child(),
            element.attributes,
            fields,
        ))?;
        self.reader.end_element()?;
        Ok(value)
    }
}
