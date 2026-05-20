use super::{
    child::ChildDeserializer,
    plain::PlainTextDeserializer,
    reader::{Attribute, ChildReader, Event, Reader},
};
use crate::{
    config::{CONTENT, TEXT},
    error::{Error, Result},
};
use log::trace;
use serde::de::IntoDeserializer;
use std::{io::Read, iter::Peekable};

pub struct MapAccess<'a, R: Read> {
    reader: ChildReader<'a, R>,
    attributes: Peekable<std::vec::IntoIter<Attribute>>,
    fields: &'static [&'static str],
}

impl<'a, R: Read> MapAccess<'a, R> {
    pub fn new_map(reader: ChildReader<'a, R>) -> Self {
        Self {
            reader,
            attributes: vec![].into_iter().peekable(),
            fields: &[],
        }
    }

    pub fn new_struct(
        reader: ChildReader<'a, R>,
        attributes: Vec<Attribute>,
        fields: &'static [&'static str],
    ) -> Self {
        Self {
            reader,
            attributes: attributes.into_iter().peekable(),
            fields,
        }
    }

    fn is_content(&self, element_name: &str) -> bool {
        !self.fields.contains(&element_name) && self.fields.contains(&CONTENT)
    }
}

impl<'de, R: Read> serde::de::MapAccess<'de> for MapAccess<'_, R> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        trace!("next map/struct key");
        if let Some(attr) = self.attributes.peek() {
            trace!("attribute {}", attr.qname());
            seed.deserialize(format!("@{}", attr.qname()).into_deserializer())
                .map(Some)
        } else {
            match self.reader.peek()? {
                Event::StartElement(element) => {
                    let element_name = element.qname();
                    if self.is_content(&element_name) {
                        trace!("#content");
                        seed.deserialize(CONTENT.into_deserializer()).map(Some)
                    } else {
                        trace!("element '{}'", element_name);
                        seed.deserialize(element_name.as_str().into_deserializer())
                            .map(Some)
                    }
                }
                Event::Text(_) => {
                    trace!("{}", TEXT);
                    seed.deserialize(TEXT.into_deserializer()).map(Some)
                }
                _ => Ok(None),
            }
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        if let Some(attr) = self.attributes.next() {
            seed.deserialize(PlainTextDeserializer::new(&attr.value))
        } else {
            match self.reader.peek()? {
                Event::StartElement(element) => {
                    let element_name = element.qname();
                    if self.is_content(&element_name) {
                        seed.deserialize(ChildDeserializer::new(self.reader.child()))
                    } else {
                        seed.deserialize(ChildDeserializer::new_with_element_name(
                            self.reader.child(),
                            element_name,
                        ))
                    }
                }
                Event::Text(_) => {
                    seed.deserialize(PlainTextDeserializer::new(&self.reader.chars()?))
                }
                event => Err(Error::Unexpected {
                    expected: "start of element or text",
                    but_got: event.to_string(),
                }),
            }
        }
    }
}
