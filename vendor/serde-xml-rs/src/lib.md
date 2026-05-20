XML is a flexible markup language that is still used for sharing data between applications or
for writing configuration files.

Serde XML provides a way to convert between text and strongly-typed Rust data structures.

# Caveats

The Serde framework was mainly designed with formats such as JSON or YAML in mind.
As opposed to XML, these formats have the advantage of a stricter syntax which makes it
possible to know what type a field is without relying on an accompanying schema,
and disallows repeating the same tag multiple times in the same object.

For example, encoding the following document in YAML is not trivial.

```xml
<document>
  <header>A header</header>
  <section>First section</section>
  <section>Second section</section>
  <sidenote>A sidenote</sidenote>
  <section>Third section</section>
  <sidenote>Another sidenote</sidenote>
  <section>Fourth section</section>
  <footer>The footer</footer>
</document>
```

One possibility is the following YAML document.

```yaml
- header: A header
- section: First section
- section: Second section
- sidenote: A sidenote
- section: Third section
- sidenote: Another sidenote
- section: Fourth section
- footer: The footer
```

Other notable differences:
- XML requires a named root node.
- XML has a namespace system.
- XML distinguishes between attributes, child tags and contents.
- In XML, the order of nodes is sometimes important.

# Basic example

```rust
use serde::{Deserialize, Serialize};
use serde_xml_rs::{from_str, to_string};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Item {
    name: String,
    source: String,
}

let src = r#"<?xml version="1.0" encoding="UTF-8"?><Item><name>Banana</name><source>Store</source></Item>"#;
let should_be = Item {
    name: "Banana".to_string(),
    source: "Store".to_string(),
};

let item: Item = from_str(src).unwrap();
assert_eq!(item, should_be);

let reserialized_item = to_string(&item).unwrap();
assert_eq!(src, reserialized_item);
```

# Correspondence between XML and Rust

## Document root

As stated above, XML documents must have one and only one root element.
This puts a constraint on the range of types that can be supported at the root,
especially during serialization when a name has to be given to the root element.

In order to support serialization and deserialization, the root Rust type, that is the type of the value passed to `to_string` or returned by `from_str`, must be one of:
- a struct
- a newtype struct
- a unit struct (not very interesting)
- an enum

<table>
<thead>
<tr><th>XML</th><th>Rust</th></tr>
</thead>
<tbody>
<tr>
<td>

```xml
<Document>
    ...
</Document>
```

</td>
<td>

```ignore
struct Document { ... }
```
```ignore
struct Document(...);
```
```ignore
enum Document { ... }
```

</td>
</tr>
</tbody>
</table>

Other types must be encapsulated in order to be serialized, because the name of the struct or enum provides the name of the root element for the XML document.

The deserializer supports more Rust types directly:
- primitives (`bool`, `char`, integers, floats)
- options
- unit (`()`)

Sequences, tuples and maps are not supported as root types for the moment, but could be in the future.

## Strings and byte arrays

<table>
<thead>
<tr><th>XML</th><th>Rust</th></tr>
</thead>
<tbody>
<tr>
<td>

```xml
<Document>
    Some text
</Document>
```
```xml
<Document>
    <![CDATA[Some text]]>
</Document>
```
```xml
<Document>
    Some <![CDATA[text]]>
</Document>
```

</td>
<td>

```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document>Some text</Document>"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document(String);

let value = Document("Some text".to_string());

# assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);
# assert_eq!(serde_xml_rs::to_string(&value).unwrap(), text);
```
```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document>Some text</Document>"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document(#[serde(with = "serde_bytes")] Vec<u8>);

let value = Document("Some text".as_bytes().to_vec());

# assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);
# assert_eq!(serde_xml_rs::to_string(&value).unwrap(), text);
```

</td>
</tr>
</tbody>
</table>

Borrowed strings are not supported.

## Primitive types

<table>
<thead>
<tr><th>XML</th><th>Rust</th></tr>
</thead>
<tbody>
<tr><th colspan="2">unit</th></tr>
<tr>
<td>

```xml
<Document></Document>
```
```xml
<Document />
```

</td>
<td>

```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document />"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document(());

let value = Document(());

# assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);
# assert_eq!(serde_xml_rs::to_string(&value).unwrap(), text);
```

</td>
</tr>
<tr><th colspan="2">boolean</th></tr>
<tr>
<td>

```xml
<Document>true</Document>
```
```xml
<Document>1</Document>
```

</td>
<td>

```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document>true</Document>"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document(bool);

let value = Document(true);

# assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);
# assert_eq!(serde_xml_rs::to_string(&value).unwrap(), text);
```

</td>
</tr>
<tr><th colspan="2">char</th></tr>
<tr>
<td>

```xml
<Document>a</Document>
```

</td>
<td>

```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document>a</Document>"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document(char);

let value = Document('a');

# assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);
# assert_eq!(serde_xml_rs::to_string(&value).unwrap(), text);
```

</td>
</tr>
<tr>
<td>

```xml
<Document>false</Document>
```
```xml
<Document>0</Document>
```

</td>
<td>

```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document>false</Document>"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
# struct Document(bool);

let value = Document(false);

# assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);
# assert_eq!(serde_xml_rs::to_string(&value).unwrap(), text);
```

</td>
</tr>
<tr><th colspan="2">integers</th></tr>
<tr>
<td>

```xml
<Document>123</Document>
```

</td>
<td>

```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document>123</Document>"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document(i32);

let value = Document(123);

# assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);
# assert_eq!(serde_xml_rs::to_string(&value).unwrap(), text);
```

</td>
</tr>
<tr><th colspan="2">floats</th></tr>
<tr>
<td>

```xml
<Document>123</Document>
```
```xml
<Document>123.0</Document>
```

</td>
<td>

```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document>123</Document>"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document(f32);

let value = Document(123.0);

# assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);
# assert_eq!(serde_xml_rs::to_string(&value).unwrap(), text);
```

</td>
</tr>
</tbody>
</table>

## Child elements

Rust structs can be used to (de)serialize the contents of XML elements:
child elements, attributes, and text.
The name of the struct field must match the name of the corresponding child element.

<table>
<thead>
<tr><th>XML</th><th>Rust</th></tr>
</thead>
<tbody>
<tr>
<td>

```xml
<Document>
  <a>abc</a>
  <b>123</b>
  <c />
</Document>
```
```xml
<Document>
  <b>123</b>
  <c />
  <a>abc</a>
</Document>
```

</td>
<td>

```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document><a>abc</a><b>123</b><c /></Document>"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document {
    a: String,
    b: i32,
    c: (),
}

let value = Document {
    a: "abc".to_string(),
    b: 123,
    c: (),
};

# assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);
# assert_eq!(serde_xml_rs::to_string(&value).unwrap(), text);
```

</td>
</tr>
</tbody>
</table>

## Attributes

Fields that deserialize to and serialize from attributes must have a name starting with `@`.

<table>
<thead>
<tr><th>XML</th><th>Rust</th></tr>
</thead>
<tbody>
<tr>
<td>

```xml
<Document a="abc" b="123" c="" />
```
```xml
<Document c="" b="123" a="abc" />
```

</td>
<td>

```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document a="abc" b="123" c="" />"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document {
    #[serde(rename = "@a")]
    a: String,
    #[serde(rename = "@b")]
    b: i32,
    #[serde(rename = "@c")]
    c: (),
}

let value = Document {
    a: "abc".to_string(),
    b: 123,
    c: (),
};

# assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);
# assert_eq!(serde_xml_rs::to_string(&value).unwrap(), text);
```

</td>
</tr>
</tbody>
</table>

For serialization to work, all attributes must be declared before any child elements.

```rust
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document {
    #[serde(rename = "@a")]
    a: String,
    #[serde(rename = "b")] // This child element appears before an attribute
    b: i32,
    #[serde(rename = "@c")]
    c: (),
}

let value = Document {
    a: "abc".to_string(),
    b: 123,
    c: (),
};

assert!(serde_xml_rs::to_string(&value).is_err()); // ERROR !
```

## Elements with attributes and text content

When an element (root or child) that contains both attributes and text content,
the struct type must have a field named `#text`.

Currently, mixed content with child elements and text is not supported.

<table>
<thead>
<tr><th>XML</th><th>Rust</th></tr>
</thead>
<tbody>
<tr>
<td>

```xml
<Document id="123">abc</Document>
```

</td>
<td>

```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document id="123">abc</Document>"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document {
    #[serde(rename = "@id")]
    id: i32,
    #[serde(rename = "#text")]
    content: String,
}

let value = Document {
    id: 123,
    content: "abc".to_string(),
};

# assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);
# assert_eq!(serde_xml_rs::to_string(&value).unwrap(), text);
```

</td>
</tr>
</tbody>
</table>

## Repeated tags and sequences

Repeated tags are handled by fields with a type of `Vec<...>`.
The name of the field must correspond to the name of the tag that is repeated.
All of the repeated tags must be consecutive, unless the [overlapping sequences](crate::config::SerdeXml::overlapping_sequences()) option is activated.

<table>
<thead>
<tr><th>XML</th><th>Rust</th></tr>
</thead>
<tbody>
<tr>
<td>

```xml
<Document>
  <item>item1</item>
  <item>item2</item>
  <item>item3</item>
</Document>
```

</td>
<td>

```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document><item>item1</item><item>item2</item><item>item3</item></Document>"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document {
    #[serde(rename = "item")]
    items: Vec<String>,
}

let value = Document {
    items: vec![
        "item1".to_string(),
        "item2".to_string(),
        "item3".to_string(),
    ],
};

# assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);
# assert_eq!(serde_xml_rs::to_string(&value).unwrap(), text);
```

</td>
</tr>
</tbody>
</table>

## Choices and enums

Enums can be used to represent different options.
The name of the variant must match the name of the child element.
Variants are handled much like their struct counterparts (unit, newtype, struct).

<table>
<thead>
<tr><th>XML</th><th>Rust</th></tr>
</thead>
<tbody>
<tr>
<td>

```xml
<Document>
  <message><quit /></message>
</Document>
```
```xml
<Document>
  <message>
    <move>
      <x>1</x>
      <y>2</y>
    </move>
  </message>
</Document>
```
```xml
<Document>
  <message><write>a message</write></message>
</Document>
```
```xml
<Document>
  <message><change-color rgb="25 24 0" /></message>
</Document>
```

</td>
<td>

```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document><message><move><x>1</x><y>2</y></move></message></Document>"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document {
    message: Message,
}

# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Message {
    Quit,
    Move { x: i32, y: i32 },
    Write(String),
    ChangeColor {
        #[serde(rename = "@rgb")]
        rgb: (i32, i32, i32)
    },
}

let value = Document {
    message: Message::Move { x: 1, y: 2 },
};

# assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);
# assert_eq!(serde_xml_rs::to_string(&value).unwrap(), text);
```

</td>
</tr>
</tbody>
</table>

## Enums in attribute values

Only unit variants can be used for attribute values.

<table>
<thead>
<tr><th>XML</th><th>Rust</th></tr>
</thead>
<tbody>
<tr>
<td>

```xml
<Document><card rank="K" suit="♣" /></Document>
```

</td>
<td>

```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document><card rank="K" suit="♣" /></Document>"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document {
    card: Card
}

# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Card {
    #[serde(rename = "@rank")]
    rank: Rank,
    #[serde(rename = "@suit")]
    suit: Suit,
}

# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
enum Rank {
    #[serde(rename = "2")] _2,
    // ...
    K,
    A,
}

# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
enum Suit {
    #[serde(rename = "♣")] Clubs,
    // ...
#    Diamonds, Hearts, Spades
}

let value = Document {
    card: Card { suit: Suit::Clubs, rank: Rank::K },
};

# assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);
# assert_eq!(serde_xml_rs::to_string(&value).unwrap(), text);
```

</td>
</tr>
</tbody>
</table>

## Sequences of choices and `#content`

Sequences of choices can be handled various ways:
- Without any configuration: a field named `item` with an enum type will be mapped to repeated element `<item>` containing a child element designating an enum variant and any parameters (`<item><variant-name>content</variant-name></item>`).
- Using a field named `#content`: any child elements are treated as enum variants and are collected into the vector.
- Container tag using an intermediate struct: `<items><item>...</item><item>...</item>...</items>`. The ergonomics of this option may be improved in the future. In the meantime, look at [serde-query](https://docs.rs/serde-query/latest/serde_query/).

<table>
<thead>
<tr><th>XML</th><th>Rust</th></tr>
</thead>
<tbody>
<tr>
<td>

```xml
<Document>
  <message><quit /></message>
  <message><change-color rgb="0 0 255" /></message>
</Document>
```

</td>
<td>

```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document><message><quit /></message><message><change-color rgb="0 0 255" /></message></Document>"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Message {
    Quit,
    Move { x: i32, y: i32 },
    Write(String),
    ChangeColor {
        #[serde(rename = "@rgb")]
        rgb: (i32, i32, i32)
    },
}

# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document {
    #[serde(rename = "message")]
    messages: Vec<Message>,
}

let value = Document {
    messages: vec![Message::Quit, Message::ChangeColor { rgb: (0, 0, 255) }],
};

# assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);
# assert_eq!(serde_xml_rs::to_string(&value).unwrap(), text);
```

</td>
</tr>
<tr><th colspan="2">

Using `#content`

</th></tr>
<tr>
<td>

```xml
<Document>
  <quit />
  <change-color rgb="0 0 255" />
</Document>
```

</td>
<td>

```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document><quit /><change-color rgb="0 0 255" /></Document>"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
# #[derive(Serialize, Deserialize)]
# #[serde(rename_all = "kebab-case")]
# enum Message {
#     Quit,
#     Move { x: i32, y: i32 },
#     Write(String),
#     ChangeColor {
#         #[serde(rename = "@rgb")]
#         rgb: (i32, i32, i32)
#     },
# }
#
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document {
    #[serde(rename = "#content")]
    messages: Vec<Message>,
}

let value = Document {
    messages: vec![Message::Quit, Message::ChangeColor { rgb: (0, 0, 255) }],
};

# assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);
# assert_eq!(serde_xml_rs::to_string(&value).unwrap(), text);
```

</td>
</tr>
<tr><th colspan="2">Container element</th></tr>
<tr>
<td>

```xml
<Document>
  <messages>
    <message><quit /></message>
    <message><change-color rgb="0 0 255" /></message>
  </messages>
</Document>
```

</td>
<td>

```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document><messages><message><quit /></message><message><change-color rgb="0 0 255" /></message></messages></Document>"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
# #[derive(Serialize, Deserialize)]
# #[serde(rename_all = "kebab-case")]
# enum Message {
#     Quit,
#     Move { x: i32, y: i32 },
#     Write(String),
#     ChangeColor {
#         #[serde(rename = "@rgb")]
#         rgb: (i32, i32, i32)
#     },
# }
#
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Messages {
    #[serde(rename = "message")]
    messages: Vec<Message>
}

# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document {
    messages: Messages,
}

let value = Document {
    messages: Messages {
        messages: vec![
            Message::Quit,
            Message::ChangeColor { rgb: (0, 0, 255) },
        ],
    },
};

# assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);
# assert_eq!(serde_xml_rs::to_string(&value).unwrap(), text);
```

</td>
</tr>
</tbody>
</table>

## XML Namespaces

Any XML namespaces declared in a document are mapped to a prefix.
That prefix can then appears in the names of attributes and elements.
The prefix must also appear in the names of the corresponding Rust fields.

- **Deserialization:** Only prefixes matter. Any `xmlns...` attributes are ignored.
- **Serialization:** The mapping between prefixes and namespace URI must be provided ([see SerdeXml::namespace](crate::SerdeXml::namespace())). All namespaces are declared in the root element.

<table>
<thead>
<tr><th>XML</th><th>Rust</th></tr>
</thead>
<tbody>
<tr>
<td>

```xml
<Document xmlns="urn:example:default" xmlns:a="urn:example:a">
  <a:a>abc</a:a>
  <b>123</b>
  <c a:id="456" />
</Document>
```

</td>
<td>

```rust
# let text = r#"<?xml version="1.0" encoding="UTF-8"?><Document xmlns="urn:example:default" xmlns:a="urn:example:a"><a:a>abc</a:a><b>123</b><c a:id="456" /></Document>"#;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct Document {
    #[serde(rename = "a:a")]
    a: String,
    b: i32,
    c: C,
}

# #[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
struct C {
    #[serde(rename = "@a:id")]
    id: i32,
}

let value = Document {
    a: "abc".to_string(),
    b: 123,
    c: C { id: 456 },
};

assert_eq!(serde_xml_rs::from_str::<Document>(text).unwrap(), value);

let config = serde_xml_rs::SerdeXml::new()
    .default_namespace("urn:example:default")
    .namespace("a", "urn:example:a");
assert_eq!(config.to_string(&value).unwrap(), text);
```

</td>
</tr>
</tbody>
</table>

# Custom EventReader

```rust
use serde::{Deserialize, Serialize};
use serde_xml_rs::{from_str, to_string, de::Deserializer};
use xml::reader::{EventReader, ParserConfig};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Item {
    name: String,
    source: String,
}

let src = r#"<Item><name>  Banana  </name><source>Store</source></Item>"#;
let should_be = Item {
    name: "  Banana  ".to_string(),
    source: "Store".to_string(),
};

let config = ParserConfig::new()
    .trim_whitespace(false)
    .whitespace_to_characters(true);
let event_reader = EventReader::new_with_config(src.as_bytes(), config);
let item = Item::deserialize(&mut Deserializer::new(event_reader)).unwrap();
assert_eq!(item, should_be);

```

# Supported Encodings

This crate relies on the `xml` crate for parsing XML.
It therefore supports the same encodings, namely:
- UTF-8 and UTF-16 (minimum requirement for the XML standard)
  - The UTF-16 file must contain a byte-order mark
- ISO-8859-1
- ASCII
