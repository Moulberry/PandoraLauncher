use crate::error::{Error, Result};
use serde::ser::SerializeSeq;
use std::io::Write;

use super::{child::ChildSerializer, writer::Writer};

pub struct SequenceSerializer<'a, W> {
    writer: &'a mut Writer<W>,
    element_name: Option<String>,
}

impl<'a, W: Write> SequenceSerializer<'a, W> {
    pub fn new(writer: &'a mut Writer<W>, element_name: Option<String>) -> Self {
        Self {
            writer,
            element_name,
        }
    }
}

impl<W: Write> SerializeSeq for SequenceSerializer<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(ChildSerializer::new(self.writer, self.element_name.clone()))
    }

    fn end(self) -> Result<Self::Ok> {
        Ok(())
    }
}
