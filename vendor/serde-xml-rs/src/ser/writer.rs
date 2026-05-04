use crate::{config::Namespaces, error::Result};
use log::trace;
use std::io::Write;
use xml::{writer::XmlEvent, EventWriter};

#[derive(Debug, PartialEq)]
pub struct Attribute {
    pub name: &'static str,
    pub value: String,
}

pub struct Writer<W> {
    xml_writer: EventWriter<W>,
    namespaces: Namespaces,
}

impl<W> Writer<W> {
    pub fn new(xml_writer: EventWriter<W>, namespaces: Namespaces) -> Self {
        Self {
            xml_writer,
            namespaces,
        }
    }
}

impl<W: Write> Writer<W> {
    pub fn start_element<S: AsRef<str>>(&mut self, name: S) -> Result<()> {
        self.start_element_with_attributes(name, &[])
    }

    pub fn start_element_with_attributes<S: AsRef<str>>(
        &mut self,
        name: S,
        attributes: &[Attribute],
    ) -> Result<()> {
        let name = name.as_ref();
        trace!("EVENT: start element '{name}'");
        let mut element = self
            .namespaces
            .add_to_start_element(XmlEvent::start_element(name));
        for attribute in attributes {
            element = element.attr(attribute.name, &attribute.value);
        }
        self.xml_writer.write(element)?;
        Ok(())
    }

    pub fn end_element(&mut self) -> Result<()> {
        trace!("EVENT: end element");
        self.xml_writer.write(XmlEvent::end_element())?;
        Ok(())
    }

    pub fn characters<S: AsRef<str>>(&mut self, text: S) -> Result<()> {
        trace!("EVENT: text");
        self.xml_writer.write(XmlEvent::Characters(text.as_ref()))?;
        Ok(())
    }

    pub fn into_inner(self) -> W {
        self.xml_writer.into_inner()
    }
}
