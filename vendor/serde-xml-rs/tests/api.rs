use indoc::indoc;
pub use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};
use serde_xml_rs::{to_string, SerdeXml};
use xml::{EmitterConfig, ParserConfig};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename = "document")]
struct Document {
    content: Content,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Content {
    text: String,
}

mod given_custom_event_writer {
    use super::*;

    #[fixture]
    fn value() -> Document {
        Document {
            content: Content {
                text: "content text".into(),
            },
        }
    }

    #[rstest]
    #[test_log::test]
    fn should_accept_custom_event_writer(value: Document) {
        let mut output = Vec::new();
        let writer = EmitterConfig::new()
            .perform_indent(true)
            .write_document_declaration(false)
            .create_writer(&mut output);
        let mut s = serde_xml_rs::ser::Serializer::new(writer);

        value.serialize(&mut s).unwrap();
        let actual = String::from_utf8(output).unwrap();

        assert_eq!(
            actual,
            indoc!(
                r#"<document>
                  <content>
                    <text>content text</text>
                  </content>
                </document>"#
            )
        );
    }
}

mod given_whitespace_preserving_parser_config {
    use super::*;

    #[fixture]
    fn text() -> &'static str {
        indoc!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <document>
            <content>
                <text>    content text  </text>
            </content>
            </document>"#
        )
    }

    #[fixture]
    fn value() -> Document {
        Document {
            content: Content {
                text: "    content text  ".into(),
            },
        }
    }

    #[rstest]
    #[test_log::test]
    fn when_deserialize_then_preserve_whitespace(text: &str, value: Document) {
        let config = SerdeXml::new().parser(
            ParserConfig::new()
                .trim_whitespace(false)
                .whitespace_to_characters(false),
        );
        let mut deserializer = serde_xml_rs::Deserializer::from_config(config, text.as_bytes());

        assert_eq!(Document::deserialize(&mut deserializer).unwrap(), value);
    }

    #[rstest]
    #[test_log::test]
    fn when_serialize_then_only_preserve_whitespace_inside_elements(value: Document) {
        let text = r#"<?xml version="1.0" encoding="UTF-8"?><document><content><text>    content text  </text></content></document>"#;
        assert_eq!(to_string(&value).unwrap(), text);
    }
}
