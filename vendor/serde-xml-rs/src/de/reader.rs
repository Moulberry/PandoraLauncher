use crate::error::{Error, Result};
use log::trace;
use std::{collections::VecDeque, io::Read};
use xml::{attribute::OwnedAttribute, name::OwnedName, reader::XmlEvent, EventReader};

#[derive(Debug, PartialEq)]
pub enum Event {
    StartElement(Element),
    Text(String),
    EndElement,
    Eof,
}

#[derive(Debug, PartialEq)]
pub struct Element {
    pub prefix: Option<String>,
    pub name: String,
    pub attributes: Vec<Attribute>,
}

impl Element {
    pub fn from(name: OwnedName, attributes: Vec<OwnedAttribute>) -> Self {
        Self {
            prefix: name.prefix,
            name: name.local_name,
            attributes: attributes.into_iter().map(Attribute::from).collect(),
        }
    }

    pub fn qname(&self) -> String {
        if let Some(prefix) = &self.prefix {
            format!("{}:{}", prefix, &self.name)
        } else {
            self.name.clone()
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Attribute {
    pub prefix: Option<String>,
    pub name: String,
    pub value: String,
}

impl Attribute {
    pub fn qname(&self) -> String {
        if let Some(prefix) = &self.prefix {
            format!("{}:{}", prefix, &self.name)
        } else {
            self.name.clone()
        }
    }
}

impl From<OwnedAttribute> for Attribute {
    fn from(value: OwnedAttribute) -> Self {
        Self {
            prefix: value.name.prefix,
            name: value.name.local_name,
            value: value.value,
        }
    }
}

pub fn parse_xml_bool(text: &str) -> Result<bool> {
    match text {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        s => Err(Error::Unexpected {
            expected: "boolean",
            but_got: s.to_string(),
        }),
    }
}

pub trait Reader<R: Read> {
    /// Look at the next event without consuming it
    fn peek(&mut self) -> Result<&Event>;

    /// Look at the `n`th event without consuming it
    fn peek_nth(&mut self, n: usize) -> Result<&Event>;
    /// Consume the `n`th event
    fn take_nth(&mut self, n: usize) -> Result<Event>;
    /// Consume the next event
    fn next(&mut self) -> Result<Event>;
    /// Create a child buffer whose cursor starts at the same position as this buffer.
    fn child(&mut self) -> ChildReader<R>;

    /// Consume the next event as a string
    fn chars(&mut self) -> Result<String> {
        match self.next()? {
            Event::Text(cs) => Ok(cs),
            event => Err(Error::Unexpected {
                expected: "text",
                but_got: event.to_string(),
            }),
        }
    }

    /// Consume the next event as a boolean
    fn bool(&mut self) -> Result<bool> {
        parse_xml_bool(self.chars()?.as_str())
    }

    /// Consume the next event as a start of element
    fn start_element(&mut self) -> Result<Element> {
        match self.next()? {
            Event::StartElement(element) => Ok(element),
            event => Err(Error::Unexpected {
                expected: "start of element",
                but_got: event.to_string(),
            }),
        }
    }

    /// Consume the next event as an end of element
    fn end_element(&mut self) -> Result<()> {
        match self.next()? {
            Event::EndElement => Ok(()),
            event => Err(Error::Unexpected {
                expected: "end of element",
                but_got: event.to_string(),
            }),
        }
    }
}

fn next_significant_event<R: Read>(xml_reader: &mut EventReader<R>) -> Result<Event> {
    let event = loop {
        let event = xml_reader.next()?;
        match event {
            XmlEvent::StartElement {
                name, attributes, ..
            } => {
                break Event::StartElement(Element::from(name, attributes));
            }
            XmlEvent::EndElement { .. } => break Event::EndElement,
            XmlEvent::Characters(s) | XmlEvent::CData(s) => break Event::Text(s),
            XmlEvent::EndDocument => break Event::Eof,
            _ => (),
        };
    };
    Ok(event)
}

pub struct RootReader<R: Read> {
    xml_reader: EventReader<R>,
    lookahead: VecDeque<Event>,
    overlapping_sequences: bool,
}

impl<R: Read> RootReader<R> {
    pub fn new(xml_reader: EventReader<R>, overlapping_sequences: bool) -> Self {
        Self {
            xml_reader,
            lookahead: VecDeque::new(),
            overlapping_sequences,
        }
    }
}

impl<R: Read> Reader<R> for RootReader<R> {
    fn child(&mut self) -> ChildReader<R> {
        ChildReader {
            xml_reader: &mut self.xml_reader,
            lookahead: &mut self.lookahead,
            overlapping_sequences: self.overlapping_sequences,
            cursor: 0,
        }
    }

    fn peek(&mut self) -> Result<&Event> {
        self.peek_nth(0)
    }

    fn peek_nth(&mut self, n: usize) -> Result<&Event> {
        while self.lookahead.len() <= n {
            self.lookahead
                .push_back(next_significant_event(&mut self.xml_reader)?);
        }
        Ok(&self.lookahead[n])
    }

    fn take_nth(&mut self, n: usize) -> Result<Event> {
        self.peek_nth(n)?;
        let event = self.lookahead.remove(n).unwrap();
        trace!("EVENT: {event:?}");
        Ok(event)
    }

    fn next(&mut self) -> Result<Event> {
        let event = if self.lookahead.is_empty() {
            next_significant_event(&mut self.xml_reader)?
        } else {
            self.lookahead.pop_front().unwrap()
        };
        trace!("EVENT: {event:?}");
        Ok(event)
    }
}

pub struct ChildReader<'r, R: Read> {
    xml_reader: &'r mut EventReader<R>,
    lookahead: &'r mut VecDeque<Event>,
    pub overlapping_sequences: bool,
    cursor: usize,
}

impl<R: Read> ChildReader<'_, R> {
    /// Consume the next element
    pub fn ignore(&mut self) -> Result<()> {
        self.start_element()?;
        let mut depth = 1usize;
        while depth > 0 {
            match self.next()? {
                event @ Event::Eof => {
                    return Err(Error::Unexpected {
                        expected: "anything",
                        but_got: event.to_string(),
                    });
                }
                Event::EndElement => depth -= 1,
                Event::StartElement(_) => depth += 1,
                Event::Text(_) => (),
            }
        }
        Ok(())
    }

    /// Advance the child buffer without consuming the events
    pub fn fast_forward(&mut self) -> Result<()> {
        self.cursor += 1;
        let mut depth = 1usize;
        while depth > 0 {
            match self.peek()? {
                event @ Event::Eof => {
                    return Err(Error::Unexpected {
                        expected: "anything",
                        but_got: event.to_string(),
                    });
                }
                Event::EndElement => depth -= 1,
                Event::StartElement(_) => depth += 1,
                Event::Text(_) => (),
            }
            self.cursor += 1;
        }
        Ok(())
    }
}

impl<R: Read> Reader<R> for ChildReader<'_, R> {
    fn peek(&mut self) -> Result<&Event> {
        self.peek_nth(self.cursor)
    }

    fn peek_nth(&mut self, n: usize) -> Result<&Event> {
        while self.lookahead.len() <= n {
            self.lookahead
                .push_back(next_significant_event(self.xml_reader)?);
        }
        Ok(&self.lookahead[n])
    }

    fn take_nth(&mut self, n: usize) -> Result<Event> {
        self.peek_nth(n)?;
        let event = self.lookahead.remove(n).unwrap();
        trace!("EVENT: {event:?}");
        Ok(event)
    }

    fn next(&mut self) -> Result<Event> {
        self.take_nth(self.cursor)
    }

    fn child(&mut self) -> ChildReader<R> {
        ChildReader {
            xml_reader: self.xml_reader,
            lookahead: self.lookahead,
            overlapping_sequences: self.overlapping_sequences,
            cursor: self.cursor,
        }
    }
}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::StartElement(element) => write!(f, "start of {}", element),
            Event::Text(_) => write!(f, "text"),
            Event::EndElement => write!(f, "end of element"),
            Event::Eof => write!(f, "end of input"),
        }
    }
}

impl std::fmt::Display for Element {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "element {} [attributes", self.qname())?;
        for attribute in &self.attributes {
            write!(f, " {}", attribute)?;
        }
        write!(f, "]")?;
        Ok(())
    }
}

impl std::fmt::Display for Attribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, r#"{}="{}""#, self.qname(), self.value)
    }
}
