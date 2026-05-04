use crate::{from_str, to_string};
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};

mod given_struct_with_repeated_simple_element {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        #[serde(rename = "item", default)]
        items: Vec<i32>,
    }

    #[rstest]
    #[case::many(r#"<item>1</item><item>2</item><item>3</item>"#, vec![1, 2, 3])]
    #[case::one(r#"<item>1</item>"#, vec![1])]
    #[case::none(r#""#, vec![])]
    #[test_log::test]
    fn when_deserialize(#[case] content_text: &str, #[case] content_value: Vec<i32>) {
        let text =
            format!(r#"<?xml version="1.0" encoding="UTF-8"?><document>{content_text}</document>"#);
        let value = Document {
            items: content_value,
        };
        assert_eq!(from_str::<Document>(&text).unwrap(), value);
    }

    #[rstest]
    #[case::many(r#"<document><item>1</item><item>2</item><item>3</item></document>"#, vec![1, 2, 3])]
    #[case::one(r#"<document><item>1</item></document>"#, vec![1])]
    #[case::none(r#"<document />"#, vec![])]
    #[test_log::test]
    fn when_serialize(#[case] content_text: &str, #[case] content_value: Vec<i32>) {
        let text = format!(r#"<?xml version="1.0" encoding="UTF-8"?>{content_text}"#);
        let value = Document {
            items: content_value,
        };
        assert_eq!(to_string(&value).unwrap(), text);
    }
}

mod given_struct_with_repeated_struct {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        #[serde(rename = "part", default)]
        parts: Vec<Part>,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Part {
        content: String,
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document><part><content>part 1</content></part><part><content>part 2</content></part></document>"#
    }

    #[fixture]
    fn value() -> Document {
        Document {
            parts: vec![
                Part {
                    content: "part 1".to_string(),
                },
                Part {
                    content: "part 2".to_string(),
                },
            ],
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

mod given_struct_with_repeated_struct_with_attribute {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        #[serde(rename = "part", default)]
        parts: Vec<Part>,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Part {
        #[serde(rename = "@id")]
        id: String,
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document><part id="1" /><part id="2" /></document>"#
    }

    #[fixture]
    fn value() -> Document {
        Document {
            parts: vec![
                Part {
                    id: "1".to_string(),
                },
                Part {
                    id: "2".to_string(),
                },
            ],
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

mod given_struct_with_list_attribute {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        #[serde(rename = "@items", default)]
        items: Vec<i32>,
    }

    #[rstest]
    #[case::many(r#"1 2 3"#, vec![1, 2, 3])]
    #[case::one(r#"1"#, vec![1])]
    #[case::none(r#""#, vec![])]
    #[test_log::test]
    fn when_deserialize(#[case] content_text: &str, #[case] content_value: Vec<i32>) {
        let text =
            format!(r#"<?xml version="1.0" encoding="UTF-8"?><document items="{content_text}" />"#);
        let value = Document {
            items: content_value,
        };
        assert_eq!(from_str::<Document>(&text).unwrap(), value);
    }

    #[rstest]
    #[case::many(r#"1 2 3"#, vec![1, 2, 3])]
    #[case::one(r#"1"#, vec![1])]
    #[case::none(r#""#, vec![])]
    #[test_log::test]
    fn when_serialize(#[case] content_text: &str, #[case] content_value: Vec<i32>) {
        let text =
            format!(r#"<?xml version="1.0" encoding="UTF-8"?><document items="{content_text}" />"#);
        let value = Document {
            items: content_value,
        };
        assert_eq!(to_string(&value).unwrap(), text);
    }
}

mod given_struct_with_plain_text_list {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        #[serde(rename = "#text", default)]
        coordinates: Vec<i32>,
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document>1 0 2 30 -4</document>"#
    }

    #[fixture]
    fn value() -> Document {
        Document {
            coordinates: vec![1, 0, 2, 30, -4],
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

mod given_multiple_root_elements {
    use super::*;
    use crate::Error;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "part")]
    struct Part {
        content: String,
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<part><content>part 1</content></part><part><content>part 2</content></part>"#
    }

    #[rstest]
    #[test_log::test]
    fn when_deserialize_then_ko(text: &str) {
        assert!(matches!(
            from_str::<Vec<Part>>(text),
            Err(Error::Unsupported(_))
        ));
    }
}
