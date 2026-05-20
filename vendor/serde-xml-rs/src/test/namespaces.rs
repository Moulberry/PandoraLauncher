use crate::SerdeXml;
use indoc::indoc;
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};
use xml::EmitterConfig;

mod given_root_element_default_ns {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        content: String,
    }

    #[fixture]
    fn config() -> SerdeXml {
        SerdeXml::new()
            .emitter(EmitterConfig::new().perform_indent(true))
            .default_namespace("document")
    }

    #[fixture]
    fn text() -> &'static str {
        indoc!(
            r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <document xmlns="document">
              <content>abc</content>
            </document>"#
        )
    }

    #[fixture]
    fn value() -> Document {
        Document {
            content: "abc".to_string(),
        }
    }

    #[rstest]
    #[test_log::test]
    fn when_deserialize(config: SerdeXml, text: &str, value: Document) {
        assert_eq!(config.from_str::<Document>(text).unwrap(), value);
    }

    #[rstest]
    #[test_log::test]
    fn when_serialize(config: SerdeXml, text: &str, value: Document) {
        assert_eq!(config.to_string(&value).unwrap(), text);
    }
}

mod given_root_element_ns {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "d:document")]
    struct Document {
        #[serde(rename = "d:content")]
        content: String,
    }

    #[fixture]
    fn text() -> &'static str {
        indoc!(
            r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <d:document xmlns:d="document">
              <d:content>abc</d:content>
            </d:document>"#
        )
    }

    #[fixture]
    fn value() -> Document {
        Document {
            content: "abc".to_string(),
        }
    }

    #[fixture]
    fn config() -> SerdeXml {
        SerdeXml::new()
            .emitter(EmitterConfig::new().perform_indent(true))
            .namespace("d", "document")
    }

    #[rstest]
    #[test_log::test]
    fn when_deserialize(config: SerdeXml, text: &str, value: Document) {
        assert_eq!(config.from_str::<Document>(text).unwrap(), value);
    }

    #[rstest]
    #[test_log::test]
    fn when_serialize(config: SerdeXml, text: &str, value: Document) {
        assert_eq!(config.to_string(&value).unwrap(), text);
    }
}

mod given_root_element_attribute_ns {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "d:document")]
    struct Document {
        #[serde(rename = "@d:content")]
        content: String,
    }

    #[fixture]
    fn text() -> &'static str {
        indoc!(
            r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <d:document xmlns:d="document" d:content="abc" />"#
        )
    }

    #[fixture]
    fn value() -> Document {
        Document {
            content: "abc".to_string(),
        }
    }

    #[fixture]
    fn config() -> SerdeXml {
        SerdeXml::new()
            .emitter(EmitterConfig::new().perform_indent(true))
            .namespace("d", "document")
    }

    #[rstest]
    #[test_log::test]
    fn when_deserialize(config: SerdeXml, text: &str, value: Document) {
        assert_eq!(config.from_str::<Document>(text).unwrap(), value);
    }

    #[rstest]
    #[test_log::test]
    fn when_serialize(config: SerdeXml, text: &str, value: Document) {
        assert_eq!(config.to_string(&value).unwrap(), text);
    }
}

mod given_enum_ns {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "d:document")]
    struct Document {
        #[serde(rename = "d:content")]
        content: Content,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    enum Content {
        #[serde(rename = "d:a")]
        A,
        #[serde(rename = "d:b")]
        B(String),
        #[serde(rename = "d:c")]
        C { field: i32 },
    }

    #[fixture]
    fn text() -> &'static str {
        indoc!(
            r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <d:document xmlns:d="document">
              <d:content>
                <d:a />
              </d:content>
            </d:document>"#
        )
    }

    #[fixture]
    fn value() -> Document {
        Document {
            content: Content::A,
        }
    }

    #[fixture]
    fn config() -> SerdeXml {
        SerdeXml::new()
            .emitter(EmitterConfig::new().perform_indent(true))
            .namespace("d", "document")
    }

    #[rstest]
    #[test_log::test]
    fn when_deserialize(config: SerdeXml, text: &str, value: Document) {
        assert_eq!(config.from_str::<Document>(text).unwrap(), value);
    }

    #[rstest]
    #[test_log::test]
    fn when_serialize(config: SerdeXml, text: &str, value: Document) {
        assert_eq!(config.to_string(&value).unwrap(), text);
    }
}
