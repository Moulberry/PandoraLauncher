use super::{plain::PlainTextSerializer, writer::Writer};
use crate::error::{Error, Result};
use serde::ser::{SerializeTuple, SerializeTupleStruct, SerializeTupleVariant};
use std::io::Write;

pub struct TupleSerializer<'a, W> {
    writer: &'a mut Writer<W>,
    buffer: Vec<String>,
    should_end_element: bool,
}

impl<'a, W> TupleSerializer<'a, W> {
    pub fn new(writer: &'a mut Writer<W>, should_end_element: bool) -> Self {
        Self {
            writer,
            buffer: Vec::new(),
            should_end_element,
        }
    }
}

impl<W: Write> SerializeTuple for TupleSerializer<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + serde::Serialize,
    {
        if let Some(text) = value.serialize(PlainTextSerializer)? {
            self.buffer.push(text);
        }
        Ok(())
    }

    fn end(self) -> Result<Self::Ok> {
        self.writer.characters(self.buffer.join(" ").as_str())?;
        if self.should_end_element {
            self.writer.end_element()?;
        }
        Ok(())
    }
}

impl<W: Write> SerializeTupleStruct for TupleSerializer<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + serde::Serialize,
    {
        <Self as SerializeTuple>::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok> {
        <Self as SerializeTuple>::end(self)
    }
}

impl<W: Write> SerializeTupleVariant for TupleSerializer<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + serde::Serialize,
    {
        <Self as SerializeTuple>::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok> {
        self.writer.characters(self.buffer.join(" ").as_str())?;
        self.writer.end_element()?;
        if self.should_end_element {
            self.writer.end_element()?;
        }
        Ok(())
    }
}
