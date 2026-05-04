use crate::{from_str, to_string};
use rstest::rstest;
use serde::{Deserialize, Serialize};

mod given_child_element_choice_sequence {
    use rstest::fixture;

    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        #[serde(rename = "#content")]
        contents: Vec<Content>,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Content {
        Unit,
        Newtype(String),
        EmptyStruct {},
        Struct {
            a: String,
            b: i32,
        },
        StructWithAttribute {
            #[serde(rename = "@id")]
            id: i32,
        },
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document><unit /><newtype>abc</newtype><empty-struct /><struct><a>abc</a><b>123</b></struct><struct-with-attribute id="123" /></document>"#
    }

    #[fixture]
    fn value() -> Document {
        Document {
            contents: vec![
                Content::Unit,
                Content::Newtype("abc".to_string()),
                Content::EmptyStruct {},
                Content::Struct {
                    a: "abc".to_string(),
                    b: 123,
                },
                Content::StructWithAttribute { id: 123 },
            ],
        }
    }

    #[rstest]
    #[test_log::test]
    fn when_deserialize(text: &str, value: Document) {
        assert_eq!(from_str::<Document>(&text).unwrap(), value);
    }

    #[rstest]
    #[test_log::test]
    fn when_serialize(text: &str, value: Document) {
        assert_eq!(to_string(&value).unwrap(), text);
    }
}

mod given_child_element_choice_sequence_with_other_elements {
    use rstest::fixture;

    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        header: String,
        #[serde(rename = "#content")]
        contents: Vec<Content>,
        footer: String,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Content {
        Unit,
        Newtype(String),
        EmptyStruct {},
        Struct {
            a: String,
            b: i32,
        },
        StructWithAttribute {
            #[serde(rename = "@id")]
            id: i32,
        },
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document><header>header</header><unit /><newtype>abc</newtype><footer>footer</footer></document>"#
    }

    #[fixture]
    fn value() -> Document {
        Document {
            header: "header".to_string(),
            contents: vec![Content::Unit, Content::Newtype("abc".to_string())],
            footer: "footer".to_string(),
        }
    }

    #[rstest]
    #[test_log::test]
    fn when_deserialize(text: &str, value: Document) {
        assert_eq!(from_str::<Document>(&text).unwrap(), value);
    }

    #[rstest]
    #[test_log::test]
    fn when_serialize(text: &str, value: Document) {
        assert_eq!(to_string(&value).unwrap(), text);
    }
}

mod given_nested_enum_sequences {
    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(rename = "document")]
    pub struct Document {
        #[serde(rename = "#content")]
        pub definitions: Vec<Definition>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub enum Definition {
        #[serde(rename = "messageDefinition")]
        Message(MessageDefinition),

        #[serde(rename = "enumerationDefinition")]
        Enum(EnumerationDefinition),
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct MessageDefinition {
        #[serde(rename = "@name")]
        pub name: String,

        #[serde(rename = "#content")]
        pub fields: Vec<Field>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct EnumerationDefinition {
        #[serde(rename = "@name")]
        pub name: String,

        #[serde(rename = "entry")]
        pub entries: Vec<EnumerationVariant>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct EnumerationVariant {
        #[serde(rename = "@value")]
        pub value: u16,
        #[serde(rename = "@name")]
        pub name: String,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub enum Field {
        #[serde(rename = "field")]
        Field {
            #[serde(rename = "@name")]
            name: String,
        },
    }

    #[rstest]
    #[case(
        r#"
        <document>
            <enumerationDefinition name="some enum">
                <entry value="0" name="first" />
                <entry value="1" name="second" />
            </enumerationDefinition>
            <messageDefinition name="msg name">
                <field name="msg_field1" />
                <field name="msg_field2" />
                <field name="msg_field3" />
            </messageDefinition>
        </document>"#
    )]
    #[case(
        r#"
        <document>
            <messageDefinition name="msg name">
                <field name="msg_field1" />
                <field name="msg_field2" />
                <field name="msg_field3" />
            </messageDefinition>
            <enumerationDefinition name="some enum">
                <entry value="0" name="first" />
                <entry value="1" name="second" />
            </enumerationDefinition>
        </document>"#
    )]
    #[test_log::test]
    fn vec_of_struct_first_parseable(#[case] text: &str) {
        assert!(from_str::<Document>(text).is_ok())
    }
}
