use crate::{from_str, to_string};
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod given_map {
    use super::*;

    type Document = BTreeMap<String, i32>;

    #[rstest]
    #[case::many(r#"<a>1</a><b>2</b><c>3</c>"#, vec![("a", 1), ("b", 2), ("c", 3)])]
    #[case::one(r#"<a>1</a>"#, vec![("a", 1)])]
    #[case::none(r#""#, vec![])]
    #[case::text(r#"123"#, vec![("#text", 123)])]
    #[test_log::test]
    fn when_deserialize(#[case] content_text: &str, #[case] content_value: Vec<(&str, i32)>) {
        let text = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><any-element>{content_text}</any-element>"#
        );
        let value = content_value
            .into_iter()
            .map(|(s, i)| (s.to_string(), i))
            .collect();
        assert_eq!(from_str::<Document>(&text).unwrap(), value);
    }

    #[rstest]
    #[case::many(vec![("a", 1), ("b", 2), ("c", 3)])]
    #[case::one(vec![("a", 1)])]
    #[case::none(vec![])]
    #[case::text(vec![("#text", 123)])]
    #[test_log::test]
    fn when_serialize_ko(#[case] content_value: Vec<(&str, i32)>) {
        let value: Document = content_value
            .into_iter()
            .map(|(s, i)| (s.to_string(), i))
            .collect();
        assert!(to_string(&value).is_err());
    }
}

mod given_newtype_map {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document(BTreeMap<String, i32>);

    #[rstest]
    #[case::many(r#"<a>1</a><b>2</b><c>3</c>"#, vec![("a", 1), ("b", 2), ("c", 3)])]
    #[case::one(r#"<a>1</a>"#, vec![("a", 1)])]
    #[case::none(r#""#, vec![])]
    #[case::text(r#"123"#, vec![("#text", 123)])]
    #[test_log::test]
    fn when_deserialize(#[case] content_text: &str, #[case] content_value: Vec<(&str, i32)>) {
        let text =
            format!(r#"<?xml version="1.0" encoding="UTF-8"?><document>{content_text}</document>"#);
        let value = Document(
            content_value
                .into_iter()
                .map(|(s, i)| (s.to_string(), i))
                .collect(),
        );
        assert_eq!(from_str::<Document>(&text).unwrap(), value);
    }

    #[rstest]
    #[case::many(r#"<document><a>1</a><b>2</b><c>3</c></document>"#, vec![("a", 1), ("b", 2), ("c", 3)])]
    #[case::one(r#"<document><a>1</a></document>"#, vec![("a", 1)])]
    #[case::none(r#"<document />"#, vec![])]
    #[case::text(r#"<document>123</document>"#, vec![("#text", 123)])]
    #[test_log::test]
    fn when_serialize(#[case] content_text: &str, #[case] content_value: Vec<(&str, i32)>) {
        let text = format!(r#"<?xml version="1.0" encoding="UTF-8"?>{content_text}"#);
        let value = Document(
            content_value
                .into_iter()
                .map(|(s, i)| (s.to_string(), i))
                .collect(),
        );
        assert_eq!(to_string(&value).unwrap(), text);
    }
}

mod given_struct_with_simple_map {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        // Using a BTreeMap here to keep a predictable order of elements
        #[serde(default)]
        items: BTreeMap<String, i32>,
    }

    #[rstest]
    #[case::many(r#"<a>1</a><b>2</b><c>3</c>"#, vec![("a", 1), ("b", 2), ("c", 3)])]
    #[case::one(r#"<a>1</a>"#, vec![("a", 1)])]
    #[case::none(r#""#, vec![])]
    #[case::text(r#"123"#, vec![("#text", 123)])]
    #[test_log::test]
    fn when_deserialize(#[case] content_text: &str, #[case] content_value: Vec<(&str, i32)>) {
        let text = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><document><items>{content_text}</items></document>"#
        );
        let value = Document {
            items: content_value
                .into_iter()
                .map(|(s, i)| (s.to_string(), i))
                .collect(),
        };
        assert_eq!(from_str::<Document>(&text).unwrap(), value);
    }

    #[rstest]
    #[case::many(r#"<items><a>1</a><b>2</b><c>3</c></items>"#, vec![("a", 1), ("b", 2), ("c", 3)])]
    #[case::one(r#"<items><a>1</a></items>"#, vec![("a", 1)])]
    #[case::none(r#"<items />"#, vec![])]
    #[case::text(r#"<items>123</items>"#, vec![("#text", 123)])]
    #[test_log::test]
    fn when_serialize(#[case] content_text: &str, #[case] content_value: Vec<(&str, i32)>) {
        let text =
            format!(r#"<?xml version="1.0" encoding="UTF-8"?><document>{content_text}</document>"#);
        let value = Document {
            items: content_value
                .into_iter()
                .map(|(s, i)| (s.to_string(), i))
                .collect(),
        };
        assert_eq!(to_string(&value).unwrap(), text);
    }
}

mod given_struct_with_map_of_struct_values {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        // Using a BTreeMap here to keep a predictable order of elements
        #[serde(default)]
        parts: BTreeMap<String, Part>,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Part {
        #[serde(rename = "@id")]
        id: u32,
        title: String,
        body: String,
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document><parts><part1 id="1"><title>Part 1</title><body>body 1</body></part1></parts></document>"#
    }

    #[fixture]
    fn value() -> Document {
        Document {
            parts: vec![(
                "part1".to_string(),
                Part {
                    id: 1,
                    title: "Part 1".to_string(),
                    body: "body 1".to_string(),
                },
            )]
            .into_iter()
            .collect(),
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

mod given_struct_with_map_of_enum_values {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        // Using a BTreeMap here to keep a predictable order of elements
        #[serde(default)]
        parts: BTreeMap<String, Part>,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Part {
        A,
        B(String),
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document><parts><part1><a /></part1><part2><b>b</b></part2></parts></document>"#
    }

    #[fixture]
    fn value() -> Document {
        Document {
            parts: vec![
                ("part1".to_string(), Part::A),
                ("part2".to_string(), Part::B("b".to_string())),
            ]
            .into_iter()
            .collect(),
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
