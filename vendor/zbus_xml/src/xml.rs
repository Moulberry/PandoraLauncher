//! A parser for D-Bus introspection XML, and a matching attribute-value escaping helper.
//!
//! Introspection XML only uses elements and attributes, so rather than reaching for a
//! general-purpose XML library this is a small [`winnow`] parser that recognises exactly the
//! D-Bus introspection grammar — `<node>`, `<interface>`, `<method>`, `<signal>`, `<property>`,
//! `<arg>` and `<annotation>` — and builds the [`Node`] tree directly, one combinator per
//! element deciding which attributes are required, which are optional, and which child elements
//! are expected. Everything else a real document may carry (the XML declaration, a doctype,
//! comments, processing instructions, CDATA and foreign elements such as Telepathy's
//! `<tp:docstring>`) is recognised only well enough to skip over it.

use std::{borrow::Cow, collections::HashSet};

use winnow::{
    LocatingSlice, ModalResult, Parser,
    combinator::{alt, cut_err, delimited, eof, opt, preceded, repeat},
    error::{ErrMode, ParserError},
    stream::{Location, Stateful},
    token::{any, take_till, take_until, take_while},
};
use zbus_names::{InterfaceName, MemberName, PropertyName};

use crate::{
    Annotation, Arg, ArgDirection, Interface, Method, Node, Property, PropertyAccess, Signal,
    Signature, Warning,
    error::{Error, Result, XmlError},
    telepathy::{self, TypeDef},
};

/// The maximum element nesting depth accepted when parsing.
///
/// Far deeper than any real introspection tree (four levels, plus one per nested `<node>`). The
/// parser iterates rather than recurses over the two axes where a document can nest arbitrarily
/// deep — nested `<node>`s and skipped-over foreign elements — so this cap only bounds the memory
/// spent tracking the open elements, not the call stack.
const MAX_DEPTH: usize = 1024;

/// Parse a D-Bus introspection document into its [`Node`] tree.
pub(crate) fn parse<'a>(document: &str) -> Result<Node<'a>> {
    parse_with_warnings(document).map(|(node, _)| node)
}

/// Parse a D-Bus introspection document, also returning the warnings collected on the way.
pub(crate) fn parse_with_warnings<'a>(document: &str) -> Result<(Node<'a>, Vec<Warning>)> {
    let mut input = Input {
        input: LocatingSlice::new(document),
        state: State {
            document,
            warnings: Vec::new(),
        },
    };
    match document_node(&mut input) {
        // The tree owns its data, so it outlives `document` and satisfies any caller lifetime.
        Ok(node) => Ok((node, input.state.warnings)),
        Err(ErrMode::Backtrack(error) | ErrMode::Cut(error)) => Err(error.into_error()),
        Err(ErrMode::Incomplete(_)) => Err(Error::Xml(XmlError::new(
            "unexpected end of document",
            document.len(),
        ))),
    }
}

/// The document's root element, preceded by an optional prolog (declaration, doctype, comments).
///
/// The root element's name is not checked, for compatibility with servers that don't name it
/// `node` and with how previous (quick-xml-based) versions of this crate behaved.
fn document_node<'i>(input: &mut Input<'i>) -> PResult<Node<'static>> {
    ignorable(input)?;
    if opt(eof).parse_next(input)?.is_some() {
        return Err(error("missing root element", input));
    }
    let (tag, attrs, self_closing) = start_element(input)?;

    node(input, tag, attrs, self_closing)
}

/// The input stream: a [`LocatingSlice`] (byte offsets for error reporting) wrapped in the
/// parser [`State`], so the whole document and the collected warnings are reachable anywhere.
type Input<'i> = Stateful<LocatingSlice<&'i str>, State<'i>>;

/// The ambient state threaded through the parser.
#[derive(Debug)]
struct State<'i> {
    /// The whole document, for slicing out the raw content of captured elements.
    document: &'i str,
    /// Warnings about ignored content, handed back by [`parse_with_warnings`].
    warnings: Vec<Warning>,
}

/// Record a warning about ignored document content.
fn warn(input: &mut Input<'_>, warning: Warning) {
    input.state.warnings.push(warning);
}

/// A parser result, using [`ParseError`] as winnow's error type.
type PResult<O> = ModalResult<O, ParseError>;

/// A parse failure: either a positioned structural/XML error, or a rejected value (an invalid
/// name or signature) that carries the matching [`Error`] variant.
#[derive(Debug)]
enum ParseError {
    Xml {
        message: Cow<'static, str>,
        offset: usize,
    },
    Domain(Error),
}

impl ParseError {
    /// A fatal XML error at `offset`.
    fn xml(message: impl Into<Cow<'static, str>>, offset: usize) -> ErrMode<Self> {
        ErrMode::Cut(ParseError::Xml {
            message: message.into(),
            offset,
        })
    }

    /// A fatal error from rejecting a value (an invalid name or signature).
    fn domain(error: Error) -> ErrMode<Self> {
        ErrMode::Cut(ParseError::Domain(error))
    }

    fn into_error(self) -> Error {
        match self {
            ParseError::Xml { message, offset } => Error::Xml(XmlError::new(message, offset)),
            ParseError::Domain(error) => error,
        }
    }
}

impl<'i> ParserError<Input<'i>> for ParseError {
    type Inner = Self;

    fn from_input(input: &Input<'i>) -> Self {
        ParseError::Xml {
            message: Cow::Borrowed("malformed markup"),
            offset: input.current_token_start(),
        }
    }

    fn into_inner(self) -> std::result::Result<Self::Inner, Self> {
        Ok(self)
    }
}

/// A fatal XML error at the input's current position.
fn error(message: impl Into<Cow<'static, str>>, input: &Input<'_>) -> ErrMode<ParseError> {
    ParseError::xml(message, input.current_token_start())
}

/// A `<node>` and its subtree.
///
/// Nested `<node>`s — the one axis on which an introspection tree itself can nest arbitrarily
/// deep — are walked iteratively on an explicit stack, so that a deeply nested document grows
/// this `Vec` rather than the call stack. Interfaces (and their fixed, shallow subtrees) recurse
/// normally.
fn node<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    attrs: Attrs<'i>,
    self_closing: bool,
) -> PResult<Node<'static>> {
    let root = empty_node(&attrs);
    if self_closing {
        return Ok(root);
    }

    let mut open: Vec<(&'i str, Node<'static>)> = vec![(tag, root)];
    loop {
        ignorable(input)?;
        let tag = open.last().expect("the root is popped only by returning").0;
        if opt(eof).parse_next(input)?.is_some() {
            return Err(error(format!("missing `</{tag}>`"), input));
        }
        if let Some(close) = opt(closing_tag).parse_next(input)? {
            if close != tag {
                return Err(error(
                    format!("unexpected `</{close}>` while parsing `<{tag}>`"),
                    input,
                ));
            }
            let (_, finished) = open.pop().expect("a close tag was just matched");
            match open.last_mut() {
                Some((_, parent)) => parent.nodes.push(finished),
                None => return Ok(finished),
            }
            continue;
        }

        let (child, child_attrs, child_self_closing) = start_element(input)?;
        match child {
            "interface" => {
                let interface = interface(input, child, child_attrs, child_self_closing)?;
                open.last_mut()
                    .expect("non-empty")
                    .1
                    .interfaces
                    .push(interface);
            }
            "node" if child_self_closing => {
                open.last_mut()
                    .expect("non-empty")
                    .1
                    .nodes
                    .push(empty_node(&child_attrs));
            }
            "node" => {
                if open.len() >= MAX_DEPTH {
                    return Err(error("maximum element nesting depth exceeded", input));
                }
                open.push((child, empty_node(&child_attrs)));
            }
            other if is_docstring(other) => {
                let docstring = capture_docstring(input, other, child_self_closing)?;
                let node = &mut open.last_mut().expect("non-empty").1;
                node.docstring = docstring.or(node.docstring.take());
            }
            other => {
                if let Some(def) =
                    telepathy_type_def(input, other, &child_attrs, child_self_closing)?
                {
                    open.last_mut()
                        .expect("non-empty")
                        .1
                        .telepathy_types
                        .push(def);
                }
            }
        }
    }
}

/// A `<node>` with only its `name`, ready to be filled in.
fn empty_node(attrs: &Attrs<'_>) -> Node<'static> {
    Node {
        name: attrs.optional("name").map(str::to_owned),
        interfaces: Vec::new(),
        nodes: Vec::new(),
        docstring: None,
        telepathy_types: Vec::new(),
    }
}

/// An `<interface>` and its members.
fn interface<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    attrs: Attrs<'i>,
    self_closing: bool,
) -> PResult<Interface<'static>> {
    let name = attrs.name(|n| InterfaceName::try_from(n).map_err(Error::Name))?;
    let mut methods = Vec::new();
    let mut properties = Vec::new();
    let mut signals = Vec::new();
    let mut annotations = Vec::new();
    let mut docstring = None;
    let mut telepathy_types = Vec::new();
    children(
        input,
        tag,
        self_closing,
        |input, child, attrs, sc| match child {
            "method" => {
                methods.push(method(input, child, attrs, sc)?);
                Ok(true)
            }
            "property" => {
                properties.push(property(input, child, attrs, sc)?);
                Ok(true)
            }
            "signal" => {
                signals.push(signal(input, child, attrs, sc)?);
                Ok(true)
            }
            "annotation" => {
                annotations.push(annotation(input, child, attrs, sc)?);
                Ok(true)
            }
            other if is_docstring(other) => {
                docstring = capture_docstring(input, other, sc)?.or(docstring.take());
                Ok(true)
            }
            other => {
                if let Some(def) = telepathy_type_def(input, other, &attrs, sc)? {
                    telepathy_types.push(def);
                }
                Ok(true)
            }
        },
    )?;

    Ok(Interface {
        name,
        methods,
        properties,
        signals,
        annotations,
        docstring,
        telepathy_types,
    })
}

/// A `<method>` with its arguments and annotations.
fn method<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    attrs: Attrs<'i>,
    self_closing: bool,
) -> PResult<Method<'static>> {
    let name = attrs.name(|n| MemberName::try_from(n).map_err(Error::Name))?;
    let mut args = Vec::new();
    let mut annotations = Vec::new();
    let mut docstring = None;
    children(
        input,
        tag,
        self_closing,
        |input, child, attrs, sc| match child {
            "arg" => {
                args.push(arg(input, child, attrs, sc)?);
                Ok(true)
            }
            "annotation" => {
                annotations.push(annotation(input, child, attrs, sc)?);
                Ok(true)
            }
            other => docstring_or_skip(input, other, &attrs, sc, &mut docstring),
        },
    )?;

    Ok(Method {
        name,
        args,
        annotations,
        docstring,
    })
}

/// A `<signal>` with its arguments and annotations.
fn signal<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    attrs: Attrs<'i>,
    self_closing: bool,
) -> PResult<Signal<'static>> {
    let name = attrs.name(|n| MemberName::try_from(n).map_err(Error::Name))?;
    let mut args = Vec::new();
    let mut annotations = Vec::new();
    let mut docstring = None;
    children(
        input,
        tag,
        self_closing,
        |input, child, attrs, sc| match child {
            "arg" => {
                args.push(arg(input, child, attrs, sc)?);
                Ok(true)
            }
            "annotation" => {
                annotations.push(annotation(input, child, attrs, sc)?);
                Ok(true)
            }
            other => docstring_or_skip(input, other, &attrs, sc, &mut docstring),
        },
    )?;

    Ok(Signal {
        name,
        args,
        annotations,
        docstring,
    })
}

/// A `<property>`: a `name`, a `type` signature, an `access` mode and any annotations.
fn property<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    attrs: Attrs<'i>,
    self_closing: bool,
) -> PResult<Property<'static>> {
    let name = attrs.name(|n| PropertyName::try_from(n).map_err(Error::Name))?;
    let ty = attrs.signature()?;
    let access = match attrs.required("access")? {
        "read" => PropertyAccess::Read,
        "write" => PropertyAccess::Write,
        "readwrite" => PropertyAccess::ReadWrite,
        other => return Err(error(format!("invalid property access `{other}`"), input)),
    };
    let tp_type = attrs.tp_type().map(str::to_owned);
    let mut annotations = Vec::new();
    let mut docstring = None;
    children(
        input,
        tag,
        self_closing,
        |input, child, attrs, sc| match child {
            "annotation" => {
                annotations.push(annotation(input, child, attrs, sc)?);
                Ok(true)
            }
            other => docstring_or_skip(input, other, &attrs, sc, &mut docstring),
        },
    )?;

    Ok(Property {
        name,
        ty,
        access,
        annotations,
        docstring,
        tp_type,
    })
}

/// An `<arg>`: an optional `name`, a `type` signature, an optional `direction` and annotations.
fn arg<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    attrs: Attrs<'i>,
    self_closing: bool,
) -> PResult<Arg> {
    let name = attrs.optional("name").map(str::to_owned);
    let ty = attrs.signature()?;
    let direction = match attrs.optional("direction") {
        Some("in") => Some(ArgDirection::In),
        Some("out") => Some(ArgDirection::Out),
        Some(other) => {
            return Err(error(
                format!("invalid argument direction `{other}`"),
                input,
            ));
        }
        None => None,
    };
    let tp_type = attrs.tp_type().map(str::to_owned);
    let mut annotations = Vec::new();
    let mut docstring = None;
    children(
        input,
        tag,
        self_closing,
        |input, child, attrs, sc| match child {
            "annotation" => {
                annotations.push(annotation(input, child, attrs, sc)?);
                Ok(true)
            }
            other => docstring_or_skip(input, other, &attrs, sc, &mut docstring),
        },
    )?;

    Ok(Arg {
        name,
        ty,
        direction,
        annotations,
        docstring,
        tp_type,
    })
}

/// An `<annotation>`: a `name`/`value` pair. Its content, if any, is ignored.
fn annotation<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    attrs: Attrs<'i>,
    self_closing: bool,
) -> PResult<Annotation> {
    let name = attrs.required("name")?.to_owned();
    let value = attrs.required("value")?.to_owned();
    children(input, tag, self_closing, |input, child, attrs, sc| {
        skip_unsupported(input, child, &attrs, sc)?;
        Ok(true)
    })?;

    Ok(Annotation { name, value })
}

/// Dispatch each child element of the just-opened `<tag>` to `handle`, skipping over content it
/// does not claim, until the matching `</tag>`.
///
/// `handle` returns whether it consumed the element's body; elements it declines (returns
/// `Ok(false)` for) are skipped along with their subtree.
fn children<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    self_closing: bool,
    mut handle: impl FnMut(&mut Input<'i>, &'i str, Attrs<'i>, bool) -> PResult<bool>,
) -> PResult<()> {
    if self_closing {
        return Ok(());
    }
    loop {
        ignorable(input)?;
        if opt(eof).parse_next(input)?.is_some() {
            return Err(error(format!("missing `</{tag}>`"), input));
        }
        if let Some(close) = opt(closing_tag).parse_next(input)? {
            if close != tag {
                return Err(error(
                    format!("unexpected `</{close}>` while parsing `<{tag}>`"),
                    input,
                ));
            }
            return Ok(());
        }
        let (child, attrs, self_closing) = start_element(input)?;
        if !handle(input, child, attrs, self_closing)? {
            skip_element(input, child, self_closing)?;
        }
    }
}

/// Skip an element whose start tag has been read, along with its whole subtree.
///
/// Iterative (tracking the open elements on a `Vec`) so that a deeply nested foreign element
/// cannot exhaust the call stack.
fn skip_element<'i>(input: &mut Input<'i>, tag: &'i str, self_closing: bool) -> PResult<()> {
    if self_closing {
        return Ok(());
    }
    let mut open = vec![tag];
    while let Some(&expected) = open.last() {
        ignorable(input)?;
        if opt(eof).parse_next(input)?.is_some() {
            return Err(error(format!("missing `</{expected}>`"), input));
        }
        if let Some(close) = opt(closing_tag).parse_next(input)? {
            if close != expected {
                return Err(error(
                    format!("unexpected `</{close}>` while parsing `<{expected}>`"),
                    input,
                ));
            }
            open.pop();
            continue;
        }
        let (child, _, self_closing) = start_element(input)?;
        if !self_closing {
            if open.len() >= MAX_DEPTH {
                return Err(error("maximum element nesting depth exceeded", input));
            }
            open.push(child);
        }
    }

    Ok(())
}

/// Skip an element that has no place in the introspection format, recording a warning.
fn skip_unsupported<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    attrs: &Attrs<'i>,
    self_closing: bool,
) -> PResult<()> {
    warn(input, Warning::unsupported(tag, attrs.offset - 1));
    skip_element(input, tag, self_closing)
}

/// The fallback child handler for an element with no type definitions of its own: capture a
/// Telepathy docstring into `slot` (last non-empty wins), or skip an unknown element with a
/// warning. Always reports the child consumed.
fn docstring_or_skip<'i>(
    input: &mut Input<'i>,
    child: &'i str,
    attrs: &Attrs<'i>,
    self_closing: bool,
    slot: &mut Option<String>,
) -> PResult<bool> {
    if is_docstring(child) {
        *slot = capture_docstring(input, child, self_closing)?.or(slot.take());
    } else {
        skip_unsupported(input, child, attrs, self_closing)?;
    }
    Ok(true)
}

/// Consume a Telepathy docstring element, returning its content.
///
/// The content is returned verbatim — typically it is HTML — with the surrounding whitespace
/// trimmed; `None` if it is empty.
fn capture_docstring<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    self_closing: bool,
) -> PResult<Option<String>> {
    if self_closing {
        return Ok(None);
    }
    let start = input.current_token_start();
    let end = skip_to_close(input, tag)?;
    let content = input.state.document[start..end].trim();

    Ok((!content.is_empty()).then(|| content.to_owned()))
}

/// Consume an element's subtree, returning the byte offset of the `<` of its matching closing
/// tag. The `tag`'s (non-self-closing) start tag has already been read.
///
/// Iterative, like [`skip_element`], so deeply nested content cannot exhaust the call stack.
fn skip_to_close<'i>(input: &mut Input<'i>, tag: &'i str) -> PResult<usize> {
    let mut open = vec![tag];
    loop {
        ignorable(input)?;
        let here = input.current_token_start();
        let expected = *open.last().expect("non-empty until returning");
        if opt(eof).parse_next(input)?.is_some() {
            return Err(error(format!("missing `</{expected}>`"), input));
        }
        if let Some(close) = opt(closing_tag).parse_next(input)? {
            if close != expected {
                return Err(error(
                    format!("unexpected `</{close}>` while parsing `<{expected}>`"),
                    input,
                ));
            }
            open.pop();
            if open.is_empty() {
                return Ok(here);
            }
            continue;
        }
        let (child, _, self_closing) = start_element(input)?;
        if !self_closing {
            if open.len() >= MAX_DEPTH {
                return Err(error("maximum element nesting depth exceeded", input));
            }
            open.push(child);
        }
    }
}

/// The part of an element or attribute name after the namespace prefix, if any.
fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// Whether `name` is a Telepathy `tp:docstring` element, under any namespace prefix.
fn is_docstring(name: &str) -> bool {
    local_name(name) == "docstring"
}

/// Whether `name` is a Telepathy `tp:type` attribute.
///
/// The prefix is required: without one, `type` is the signature attribute.
fn is_tp_type(name: &str) -> bool {
    name.split_once(':')
        .is_some_and(|(prefix, local)| !prefix.is_empty() && local == "type")
}

/// A `type` attribute holding a D-Bus signature, in a Telepathy type definition.
enum SignatureAttr {
    Missing,
    Invalid,
    Value(Signature),
}

impl SignatureAttr {
    fn parse(value: Option<&str>) -> Self {
        match value {
            None => SignatureAttr::Missing,
            Some(value) => match zvariant::Signature::try_from(value.as_bytes()) {
                // The empty signature parses as `Unit`, which is only valid as a top-level
                // signature — inside a composed signature (`Struct`/`Mapping::signature`) it
                // produces invalid signatures.
                Ok(zvariant::Signature::Unit) | Err(_) => SignatureAttr::Invalid,
                Ok(signature) => SignatureAttr::Value(Signature(signature)),
            },
        }
    }
}

/// Parse a Telepathy type-definition element, if `tag` is one.
///
/// The element is consumed in all cases; `Ok(None)` — with a warning recorded — is returned
/// when `tag` is not a type definition, or is one that cannot be parsed.
fn telepathy_type_def<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    attrs: &Attrs<'i>,
    self_closing: bool,
) -> PResult<Option<TypeDef>> {
    match local_name(tag) {
        "simple-type" => simple_type(input, tag, attrs, self_closing),
        "enum" => enum_def(input, tag, attrs, self_closing),
        "struct" => struct_def(input, tag, attrs, self_closing),
        "mapping" => mapping_def(input, tag, attrs, self_closing),
        _ => {
            skip_unsupported(input, tag, attrs, self_closing)?;
            Ok(None)
        }
    }
}

/// Record a "malformed element" warning and return `Ok(None)`.
fn malformed<T>(
    input: &mut Input<'_>,
    tag: &str,
    position: usize,
    reason: impl std::fmt::Display,
) -> PResult<Option<T>> {
    warn(input, Warning::malformed(tag, position, reason));
    Ok(None)
}

/// Parse a `<tp:simple-type>`: a `name` given to a plain D-Bus `type`.
fn simple_type<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    attrs: &Attrs<'i>,
    self_closing: bool,
) -> PResult<Option<TypeDef>> {
    let position = attrs.offset - 1;
    let name = attrs.optional("name").map(str::to_owned);
    let ty = SignatureAttr::parse(attrs.optional("type"));
    let mut docstring = None;
    children(input, tag, self_closing, |input, child, attrs, sc| {
        docstring_or_skip(input, child, &attrs, sc, &mut docstring)
    })?;

    let Some(name) = name else {
        return malformed(input, tag, position, "missing attribute `name`");
    };
    let ty = match ty {
        SignatureAttr::Value(ty) => ty,
        SignatureAttr::Missing => {
            return malformed(input, tag, position, "missing attribute `type`");
        }
        SignatureAttr::Invalid => {
            return malformed(input, tag, position, "invalid signature in `type`");
        }
    };

    Ok(Some(TypeDef::SimpleType(telepathy::SimpleType {
        name,
        ty,
        docstring,
    })))
}

/// Parse a `<tp:enum>`: a `name`, an underlying `type` and its `<tp:enumvalue>`s.
fn enum_def<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    attrs: &Attrs<'i>,
    self_closing: bool,
) -> PResult<Option<TypeDef>> {
    let position = attrs.offset - 1;
    let name = attrs.optional("name").map(str::to_owned);
    let ty = SignatureAttr::parse(attrs.optional("type"));
    let mut values = Vec::new();
    let mut incomplete_value = false;
    let mut docstring = None;
    children(input, tag, self_closing, |input, child, attrs, sc| {
        if local_name(child) == "enumvalue" {
            match enum_value(input, child, &attrs, sc)? {
                Some(value) => values.push(value),
                None => incomplete_value = true,
            }
            Ok(true)
        } else {
            docstring_or_skip(input, child, &attrs, sc, &mut docstring)
        }
    })?;

    let Some(name) = name else {
        return malformed(input, tag, position, "missing attribute `name`");
    };
    let ty = match ty {
        SignatureAttr::Value(ty) => ty,
        SignatureAttr::Missing => {
            return malformed(input, tag, position, "missing attribute `type`");
        }
        SignatureAttr::Invalid => {
            return malformed(input, tag, position, "invalid signature in `type`");
        }
    };
    if incomplete_value {
        let reason = "an enumvalue is missing its `suffix` or `value` attribute";
        return malformed(input, tag, position, reason);
    }

    Ok(Some(TypeDef::Enum(telepathy::Enum {
        name,
        ty,
        values,
        docstring,
    })))
}

/// Parse a `<tp:enumvalue>`: a `suffix`/`value` pair. `None` if either is missing.
fn enum_value<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    attrs: &Attrs<'i>,
    self_closing: bool,
) -> PResult<Option<telepathy::EnumValue>> {
    let suffix = attrs.optional("suffix").map(str::to_owned);
    let value = attrs.optional("value").map(str::to_owned);
    let mut docstring = None;
    children(input, tag, self_closing, |input, child, attrs, sc| {
        docstring_or_skip(input, child, &attrs, sc, &mut docstring)
    })?;

    Ok(suffix
        .zip(value)
        .map(|(suffix, value)| telepathy::EnumValue {
            suffix,
            value,
            docstring,
        }))
}

/// Parse a `<tp:struct>`: a `name` and its `<tp:member>`s.
fn struct_def<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    attrs: &Attrs<'i>,
    self_closing: bool,
) -> PResult<Option<TypeDef>> {
    let position = attrs.offset - 1;
    let name = attrs.optional("name").map(str::to_owned);
    let (members, incomplete_member, docstring) = members(input, tag, self_closing)?;

    let Some(name) = name else {
        return malformed(input, tag, position, "missing attribute `name`");
    };
    if incomplete_member {
        return malformed(input, tag, position, INCOMPLETE_MEMBER);
    }
    if members.is_empty() {
        return malformed(input, tag, position, "no members");
    }

    Ok(Some(TypeDef::Struct(telepathy::Struct {
        name,
        members,
        docstring,
    })))
}

/// Parse a `<tp:mapping>`: a `name` and exactly two `<tp:member>`s (a basic key and a value).
fn mapping_def<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    attrs: &Attrs<'i>,
    self_closing: bool,
) -> PResult<Option<TypeDef>> {
    let position = attrs.offset - 1;
    let name = attrs.optional("name").map(str::to_owned);
    let (members, incomplete_member, docstring) = members(input, tag, self_closing)?;

    let Some(name) = name else {
        return malformed(input, tag, position, "missing attribute `name`");
    };
    if incomplete_member {
        return malformed(input, tag, position, INCOMPLETE_MEMBER);
    }
    let mut members = members.into_iter();
    let (key, value) = match (members.next(), members.next(), members.next()) {
        (Some(key), Some(value), None) => (key, value),
        _ => return malformed(input, tag, position, "expected exactly 2 members"),
    };
    if !telepathy::is_basic(key.ty()) {
        return malformed(input, tag, position, "the key is not a basic type");
    }

    Ok(Some(TypeDef::Mapping(telepathy::Mapping {
        name,
        key,
        value,
        docstring,
    })))
}

const INCOMPLETE_MEMBER: &str =
    "a member is missing its `name` or `type` attribute, or has an invalid signature";

/// Parse the `<tp:member>` children (and docstring) of a `<tp:struct>` or `<tp:mapping>`,
/// returning the members, whether any was incomplete, and the docstring.
#[allow(clippy::type_complexity)]
fn members<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    self_closing: bool,
) -> PResult<(Vec<telepathy::Member>, bool, Option<String>)> {
    let mut members = Vec::new();
    let mut incomplete_member = false;
    let mut docstring = None;
    children(input, tag, self_closing, |input, child, attrs, sc| {
        if local_name(child) == "member" {
            match member(input, child, &attrs, sc)? {
                Some(member) => members.push(member),
                None => incomplete_member = true,
            }
            Ok(true)
        } else {
            docstring_or_skip(input, child, &attrs, sc, &mut docstring)
        }
    })?;

    Ok((members, incomplete_member, docstring))
}

/// Parse a `<tp:member>`: a `name`, a `type` and an optional `tp:type`. `None` if the `name`
/// or a valid `type` is missing.
fn member<'i>(
    input: &mut Input<'i>,
    tag: &'i str,
    attrs: &Attrs<'i>,
    self_closing: bool,
) -> PResult<Option<telepathy::Member>> {
    let name = attrs.optional("name").map(str::to_owned);
    let ty = SignatureAttr::parse(attrs.optional("type"));
    let tp_type = attrs.tp_type().map(str::to_owned);
    let mut docstring = None;
    children(input, tag, self_closing, |input, child, attrs, sc| {
        docstring_or_skip(input, child, &attrs, sc, &mut docstring)
    })?;

    let (Some(name), SignatureAttr::Value(ty)) = (name, ty) else {
        return Ok(None);
    };

    Ok(Some(telepathy::Member {
        name,
        ty,
        tp_type,
        docstring,
    }))
}

/// Consume any content that carries no introspection data: whitespace and text, comments, CDATA
/// sections, processing instructions and markup declarations. Stops at the next element boundary.
fn ignorable<'i>(input: &mut Input<'i>) -> PResult<()> {
    repeat(
        0..,
        alt((
            comment,
            cdata,
            processing_instruction,
            markup_declaration,
            text,
        )),
    )
    .parse_next(input)
}

/// An XML comment: `<!-- … -->`.
fn comment<'i>(input: &mut Input<'i>) -> PResult<()> {
    ("<!--", cut_err((take_until(0.., "-->"), "-->")))
        .void()
        .parse_next(input)
}

/// A CDATA section: `<![CDATA[ … ]]>`.
fn cdata<'i>(input: &mut Input<'i>) -> PResult<()> {
    ("<![CDATA[", cut_err((take_until(0.., "]]>"), "]]>")))
        .void()
        .parse_next(input)
}

/// A processing instruction, e. g. the `<?xml … ?>` declaration.
fn processing_instruction<'i>(input: &mut Input<'i>) -> PResult<()> {
    ("<?", cut_err((take_until(0.., "?>"), "?>")))
        .void()
        .parse_next(input)
}

/// A markup declaration such as `<!DOCTYPE …>`.
///
/// Tried after [`comment`] and [`cdata`], which also open with `<!`.
fn markup_declaration<'i>(input: &mut Input<'i>) -> PResult<()> {
    preceded("<!", cut_err(markup_body)).parse_next(input)
}

/// The body of a markup declaration, up to and including the terminating `>`.
///
/// A `>` inside a quoted literal or an internal subset (`[ … ]`, which may itself contain `>`s)
/// does not terminate the declaration.
fn markup_body<'i>(input: &mut Input<'i>) -> PResult<()> {
    let mut bracket_depth = 0usize;
    loop {
        match any.parse_next(input)? {
            quote @ ('"' | '\'') => {
                (take_until(0.., quote), any).void().parse_next(input)?;
            }
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '>' if bracket_depth == 0 => return Ok(()),
            _ => (),
        }
    }
}

/// A run of character data between tags (carries no introspection data, so ignored).
fn text<'i>(input: &mut Input<'i>) -> PResult<()> {
    take_till(1.., '<').void().parse_next(input)
}

/// A closing tag such as `</node>`, yielding the element name.
fn closing_tag<'i>(input: &mut Input<'i>) -> PResult<&'i str> {
    delimited("</", xml_name, (whitespace, '>')).parse_next(input)
}

/// A start tag such as `<arg name="foo" type="s">`, or the self-closing `<arg … />`, yielding
/// the element name, its [`Attrs`] and whether it is self-closing.
fn start_element<'i>(input: &mut Input<'i>) -> PResult<(&'i str, Attrs<'i>, bool)> {
    let (name, span) = preceded('<', xml_name.with_span()).parse_next(input)?;
    let pairs = attributes(input)?;
    let self_closing =
        preceded(whitespace, alt(("/>".value(true), ">".value(false)))).parse_next(input)?;

    Ok((
        name,
        Attrs {
            element: name,
            offset: span.start,
            pairs,
        },
        self_closing,
    ))
}

/// The attributes of an element: names paired with their unescaped values, in document order.
struct Attrs<'i> {
    /// The element name, for error messages.
    element: &'i str,
    /// The byte offset of the element name, for missing-attribute errors.
    offset: usize,
    pairs: Vec<(&'i str, Cow<'i, str>)>,
}

impl<'i> Attrs<'i> {
    /// The value of `key`, if present.
    fn optional(&self, key: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value.as_ref())
    }

    /// The value of `key`, or a "missing attribute" error anchored at the element.
    fn required(&self, key: &str) -> PResult<&str> {
        self.optional(key).ok_or_else(|| {
            ParseError::xml(
                format!("missing attribute `{key}` on `<{}>`", self.element),
                self.offset,
            )
        })
    }

    /// The required `name` attribute, validated by `parse` (e. g. into an [`InterfaceName`]).
    fn name<T>(&self, parse: impl FnOnce(String) -> std::result::Result<T, Error>) -> PResult<T> {
        parse(self.required("name")?.to_owned()).map_err(ParseError::domain)
    }

    /// The required `type` attribute, parsed as a signature.
    fn signature(&self) -> PResult<Signature> {
        zvariant::Signature::try_from(self.required("type")?.as_bytes())
            .map(Signature)
            .map_err(|e| ParseError::domain(zvariant::Error::from(e).into()))
    }

    /// The value of the Telepathy `tp:type` attribute, if present.
    fn tp_type(&self) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(name, _)| is_tp_type(name))
            .map(|(_, value)| value.as_ref())
    }
}

/// The attributes of an element, unescaped and with duplicates rejected.
fn attributes<'i>(input: &mut Input<'i>) -> PResult<Vec<(&'i str, Cow<'i, str>)>> {
    let raw: Vec<Attribute<'i>> = repeat(0.., attribute).parse_next(input)?;
    let mut pairs: Vec<(&'i str, Cow<'i, str>)> = Vec::with_capacity(raw.len());
    // Real elements carry only a few attributes, for which a linear duplicate scan is cheapest;
    // but that scan is O(n²), so once a (only ever hostile) tag grows past a threshold, track the
    // names in a set to keep the work linear.
    let mut seen: Option<HashSet<&'i str>> = None;
    for attr in raw {
        let duplicate = match seen {
            Some(ref mut seen) => !seen.insert(attr.name),
            None => {
                let duplicate = pairs.iter().any(|(name, _)| *name == attr.name);
                if !duplicate && pairs.len() >= 32 {
                    let mut set: HashSet<&'i str> = pairs.iter().map(|(name, _)| *name).collect();
                    set.insert(attr.name);
                    seen = Some(set);
                }
                duplicate
            }
        };
        if duplicate {
            return Err(ParseError::xml(
                format!("duplicate attribute `{}`", attr.name),
                attr.name_offset,
            ));
        }
        let value = unescape(attr.value)
            .map_err(|(message, at)| ParseError::xml(message, attr.value_offset + at))?;
        pairs.push((attr.name, value));
    }

    Ok(pairs)
}

/// A single attribute with the byte offsets needed to anchor errors precisely.
struct Attribute<'i> {
    name: &'i str,
    name_offset: usize,
    /// The raw (still escaped) value.
    value: &'i str,
    value_offset: usize,
}

/// A single attribute, e. g. `name="foo"`, along with the whitespace XML requires before it.
///
/// The mandatory leading whitespace is what rejects run-together attributes like
/// `name="a"type="s"`.
fn attribute<'i>(input: &mut Input<'i>) -> PResult<Attribute<'i>> {
    let (name, name_span) = preceded(whitespace1, xml_name.with_span()).parse_next(input)?;
    (whitespace, '=', whitespace).parse_next(input)?;
    let (value, value_offset) = quoted_value(input)?;

    Ok(Attribute {
        name,
        name_offset: name_span.start,
        value,
        value_offset,
    })
}

/// An element or attribute name: everything up to a delimiter.
///
/// Names are only tokenized here, not validated against the XML grammar: the parser only ever
/// compares them against the fixed set of introspection names.
fn xml_name<'i>(input: &mut Input<'i>) -> PResult<&'i str> {
    take_while(1.., |c: char| {
        !c.is_ascii_whitespace() && !matches!(c, '=' | '/' | '>' | '<')
    })
    .parse_next(input)
}

/// A quoted attribute value, returning its raw (still escaped) contents and their byte offset.
fn quoted_value<'i>(input: &mut Input<'i>) -> PResult<(&'i str, usize)> {
    alt((
        delimited('"', take_until(0.., '"').with_span(), '"'),
        delimited('\'', take_until(0.., '\'').with_span(), '\''),
    ))
    .map(|(value, span)| (value, span.start))
    .parse_next(input)
}

/// Optional run of XML whitespace.
fn whitespace<'i>(input: &mut Input<'i>) -> PResult<&'i str> {
    take_while(0.., |c: char| c.is_ascii_whitespace()).parse_next(input)
}

/// At least one XML whitespace character.
fn whitespace1<'i>(input: &mut Input<'i>) -> PResult<&'i str> {
    take_while(1.., |c: char| c.is_ascii_whitespace()).parse_next(input)
}

/// Resolve entity and character references in an attribute value and normalize whitespace.
///
/// Per the XML attribute-value normalization rules, literal whitespace characters are replaced
/// with spaces, while whitespace escaped through character references (e.g. `&#10;`) is kept. A
/// literal `\r\n` collapses to a single space, as line-ending normalization (which folds it to a
/// lone `\n`) precedes attribute-value normalization. On failure, the error carries the byte
/// offset of the offending reference within `value`.
fn unescape(value: &str) -> std::result::Result<Cow<'_, str>, (String, usize)> {
    if !value.contains(['&', '\t', '\n', '\r']) {
        return Ok(Cow::Borrowed(value));
    }

    let mut unescaped = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(i) = rest.find(['&', '\t', '\n', '\r']) {
        unescaped.push_str(&rest[..i]);
        let reference = value.len() - rest.len() + i;
        if rest.as_bytes()[i] != b'&' {
            unescaped.push(' ');
            // A `\r\n` pair is a single line ending, so it normalizes to one space, not two.
            let width = if rest.as_bytes()[i] == b'\r' && rest.as_bytes().get(i + 1) == Some(&b'\n')
            {
                2
            } else {
                1
            };
            rest = &rest[i + width..];
            continue;
        }
        rest = &rest[i + 1..];
        let end = rest
            .find(';')
            .ok_or(("unterminated entity reference".to_string(), reference))?;
        let entity = &rest[..end];
        match entity {
            "amp" => unescaped.push('&'),
            "lt" => unescaped.push('<'),
            "gt" => unescaped.push('>'),
            "quot" => unescaped.push('"'),
            "apos" => unescaped.push('\''),
            _ => unescaped.push(char_reference(entity).map_err(|e| (e, reference))?),
        }
        rest = &rest[end + 1..];
    }
    unescaped.push_str(rest);

    Ok(Cow::Owned(unescaped))
}

/// Resolve a numeric character reference (the part of `&#...;` between `&` and `;`).
fn char_reference(entity: &str) -> std::result::Result<char, String> {
    let invalid = || format!("invalid character reference `&{entity};`");

    let code = if let Some(hex) = entity
        .strip_prefix("#x")
        .or_else(|| entity.strip_prefix("#X"))
    {
        if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid());
        }
        u32::from_str_radix(hex, 16).ok()
    } else if let Some(dec) = entity.strip_prefix('#') {
        if dec.is_empty() || !dec.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid());
        }
        dec.parse().ok()
    } else {
        return Err(format!("unknown entity `&{entity};`"));
    };

    code.and_then(char::from_u32)
        .filter(|c| is_xml_char(*c))
        .ok_or_else(invalid)
}

/// Whether `c` is a valid XML 1.0 character (the `Char` production).
fn is_xml_char(c: char) -> bool {
    matches!(
        c,
        '\u{9}' | '\u{A}' | '\u{D}' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
    )
}

/// Escape a string for use as an attribute value.
///
/// Whitespace other than the space character is escaped as character references so that it
/// survives the attribute-value normalization done by parsers.
pub(crate) fn escape(value: &str) -> Cow<'_, str> {
    if !value.contains(['&', '<', '>', '"', '\'', '\t', '\n', '\r']) {
        return Cow::Borrowed(value);
    }

    let mut escaped = String::with_capacity(value.len() + 8);
    for c in value.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            '\t' => escaped.push_str("&#9;"),
            '\n' => escaped.push_str("&#10;"),
            '\r' => escaped.push_str("&#13;"),
            c => escaped.push(c),
        }
    }

    Cow::Owned(escaped)
}
