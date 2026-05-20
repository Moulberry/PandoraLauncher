use super::{
    child::ChildSerializer,
    plain::PlainTextSerializer,
    writer::{Attribute, Writer},
};
use crate::{
    config::{CONTENT, TEXT},
    error::{Error, Result},
};
use std::io::Write;

pub struct StructSerializer<'a, W> {
    writer: &'a mut Writer<W>,
    name: String,
    parent_name: Option<String>,
    attributes: Vec<Attribute>,
    parent_element_written: bool,
    start_element_written: bool,
}

impl<'a, W> StructSerializer<'a, W> {
    pub fn new(writer: &'a mut Writer<W>, name: String) -> Self {
        Self {
            writer,
            name,
            parent_name: None,
            attributes: Vec::new(),
            parent_element_written: false,
            start_element_written: false,
        }
    }

    pub fn new_variant(
        writer: &'a mut Writer<W>,
        parent_name: Option<String>,
        name: String,
    ) -> Self {
        Self {
            writer,
            name,
            parent_name,
            attributes: Vec::new(),
            parent_element_written: false,
            start_element_written: false,
        }
    }
}

impl<W: Write> StructSerializer<'_, W> {
    fn ensure_parent_element_written(&mut self) -> Result<()> {
        if let Some(parent_name) = &self.parent_name {
            if !self.parent_element_written {
                self.writer.start_element(parent_name)?;
                self.parent_element_written = true;
            }
        }
        Ok(())
    }

    fn ensure_start_element_written(&mut self) -> Result<()> {
        if !self.start_element_written {
            self.writer
                .start_element_with_attributes(&self.name, &self.attributes)?;
            self.start_element_written = true;
        }
        Ok(())
    }
}

impl<W: Write> serde::ser::SerializeStruct for StructSerializer<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + serde::Serialize,
    {
        self.ensure_parent_element_written()?;
        if let Some(name) = key.strip_prefix("@") {
            if self.start_element_written {
                Err(Error::AttributesMustComeBeforeElements {
                    element_name: self.name.to_string(),
                    attribute_name: key,
                })
            } else {
                if let Some(value) = value.serialize(PlainTextSerializer)? {
                    self.attributes.push(Attribute { name, value });
                }
                Ok(())
            }
        } else {
            self.ensure_start_element_written()?;
            if key == TEXT {
                if let Some(value) = value.serialize(PlainTextSerializer)? {
                    self.writer.characters(value)?;
                }
            } else if key == CONTENT {
                value.serialize(ChildSerializer::new(self.writer, None))?;
            } else {
                value.serialize(ChildSerializer::new(self.writer, Some(key.to_string())))?;
            }
            Ok(())
        }
    }

    fn end(mut self) -> Result<Self::Ok> {
        self.ensure_parent_element_written()?;
        self.ensure_start_element_written()?;
        self.writer.end_element()?;
        if self.parent_name.is_some() {
            self.writer.end_element()?;
        }
        Ok(())
    }
}

impl<W: Write> serde::ser::SerializeStructVariant for StructSerializer<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + serde::Serialize,
    {
        <StructSerializer<W> as serde::ser::SerializeStruct>::serialize_field(self, key, value)
    }

    fn end(self) -> Result<Self::Ok> {
        <StructSerializer<W> as serde::ser::SerializeStruct>::end(self)
    }
}

pub struct MapSerializer<'a, W> {
    writer: &'a mut Writer<W>,
    element_name: String,
    should_end_element: bool,
}

impl<'a, W> MapSerializer<'a, W> {
    pub fn new(writer: &'a mut Writer<W>, should_end_element: bool) -> Self {
        Self {
            writer,
            element_name: "".to_string(),
            should_end_element,
        }
    }
}

impl<W: Write> serde::ser::SerializeMap for MapSerializer<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + serde::Serialize,
    {
        self.element_name = key
            .serialize(PlainTextSerializer)?
            .ok_or(Error::Unexpected {
                expected: "key",
                but_got: "Option::None".to_string(),
            })?;
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + serde::Serialize,
    {
        let element_name = std::mem::replace(&mut self.element_name, "".to_string());
        if element_name == TEXT {
            if let Some(text) = value.serialize(PlainTextSerializer)? {
                self.writer.characters(text)?;
            }
        } else {
            value.serialize(ChildSerializer::new(self.writer, Some(element_name)))?;
        }
        Ok(())
    }

    fn end(self) -> Result<Self::Ok> {
        if self.should_end_element {
            self.writer.end_element()?;
        }
        Ok(())
    }
}
