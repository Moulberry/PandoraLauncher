use crate::{from_str, to_string};
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};

mod given_struct_with_single_attribute_and_text {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        #[serde(rename = "@id")]
        id: u32,
        #[serde(rename = "#text", default)]
        content: String,
    }

    mod and_non_empty_content {
        use super::*;

        #[fixture]
        fn text() -> &'static str {
            r#"<?xml version="1.0" encoding="UTF-8"?><document id="123">abc</document>"#
        }

        #[fixture]
        fn value() -> Document {
            Document {
                id: 123,
                content: "abc".to_string(),
            }
        }

        #[rstest]
        #[test_log::test]
        fn when_deserialize(text: &str, value: Document) {
            assert_eq!(from_str::<Document>(text).unwrap(), value);
        }

        #[rstest]
        #[test_log::test]
        fn when_serialize(text: &str, value: Document) {
            assert_eq!(to_string(&value).unwrap(), text);
        }
    }

    mod and_empty_content {
        use super::*;

        #[fixture]
        fn text() -> &'static str {
            r#"<?xml version="1.0" encoding="UTF-8"?><document id="123"></document>"#
        }

        #[fixture]
        fn value() -> Document {
            Document {
                id: 123,
                content: "".to_string(),
            }
        }

        #[rstest]
        #[test_log::test]
        fn when_deserialize(text: &str, value: Document) {
            assert_eq!(from_str::<Document>(text).unwrap(), value);
        }

        #[rstest]
        #[test_log::test]
        fn when_serialize(text: &str, value: Document) {
            assert_eq!(to_string(&value).unwrap(), text);
        }
    }
}
