use super::{
    child::ChildDeserializer,
    reader::{ChildReader, Event, Reader},
};
use crate::error::{Error, Result};
use log::trace;
use std::io::Read;

pub struct SeqAccess<'a, R: Read> {
    reader: ChildReader<'a, R>,
    element_name: Option<String>,
}

impl<'a, R: Read> SeqAccess<'a, R> {
    pub fn new(reader: ChildReader<'a, R>, element_name: Option<String>) -> Self {
        Self {
            reader,
            element_name,
        }
    }
}

impl<'de, R: Read> serde::de::SeqAccess<'de> for SeqAccess<'_, R> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        trace!("next element");
        let overlapping_sequences = self.reader.overlapping_sequences;
        loop {
            match (&self.element_name, self.reader.peek()?) {
                (Some(element_name), Event::StartElement(element))
                    if &element.qname() == element_name =>
                {
                    break seed
                        .deserialize(ChildDeserializer::new_with_element_name(
                            self.reader.child(),
                            element_name.to_string(),
                        ))
                        .map(Some);
                }
                (Some(_), Event::StartElement(_)) if overlapping_sequences => {
                    trace!("ff {}", self.reader.peek()?);
                    self.reader.fast_forward()?;
                }
                (None, Event::StartElement(_)) => {
                    break match seed.deserialize(ChildDeserializer::new(self.reader.child())) {
                        Ok(r) => Ok(Some(r)),
                        Err(Error::Custom(_) | Error::Unexpected { .. }) => Ok(None),
                        Err(e) => Err(e),
                    }
                }
                _ => {
                    trace!("end sequence");
                    break Ok(None);
                }
            }
        }
    }
}
