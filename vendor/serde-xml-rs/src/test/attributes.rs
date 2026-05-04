use crate::{from_str, to_string};
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};

mod given_struct_with_single_attribute {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        #[serde(rename = "@content")]
        content: String,
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document content="abc" />"#
    }

    #[fixture]
    fn value() -> Document {
        Document {
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

mod given_struct_with_multiple_attributes {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        #[serde(rename = "@a")]
        a: String,
        #[serde(rename = "@b")]
        b: String,
        #[serde(rename = "@c")]
        c: String,
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document a="a" b="b" c="c" />"#
    }

    #[fixture]
    fn value() -> Document {
        Document {
            a: "a".to_string(),
            b: "b".to_string(),
            c: "c".to_string(),
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

mod given_struct_with_attributes_followed_by_elements {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        #[serde(rename = "@a")]
        a: String,
        #[serde(rename = "@b")]
        b: String,
        c: String,
        d: String,
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document a="a" b="b"><c>c</c><d>d</d></document>"#
    }

    #[fixture]
    fn value() -> Document {
        Document {
            a: "a".to_string(),
            b: "b".to_string(),
            c: "c".to_string(),
            d: "d".to_string(),
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

mod given_struct_with_interleved_elements_and_attributes {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        #[serde(rename = "@a")]
        a: String,
        b: String,
        #[serde(rename = "@c")]
        c: String,
        d: String,
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document a="a" c="c"><b>b</b><d>d</d></document>"#
    }

    #[fixture]
    fn value() -> Document {
        Document {
            a: "a".to_string(),
            b: "b".to_string(),
            c: "c".to_string(),
            d: "d".to_string(),
        }
    }

    #[rstest]
    #[test_log::test]
    fn when_deserialize_ok(text: &str, value: Document) {
        assert_eq!(from_str::<Document>(text).unwrap(), value);
    }

    #[rstest]
    #[test_log::test]
    fn when_serialize_then_error(value: Document) {
        assert!(to_string(&value).is_err());
    }
}

mod given_nested_structs {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        part: Part,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    struct Part {
        #[serde(rename = "@content")]
        content: String,
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document><part content="abc" /></document>"#
    }

    #[fixture]
    fn value() -> Document {
        Document {
            part: Part {
                content: "abc".to_string(),
            },
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

mod given_struct_with_simple_datatype_attribute {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document<T> {
        #[serde(rename = "@content")]
        content: T,
    }

    #[rstest]
    #[case::unit((), "")]
    #[case::string("abc".to_string(), "abc")]
    #[case::char('a', "a")]
    #[case::bool_true(true, "true")]
    #[case::bool_false(false, "false")]
    #[case::u8(1u8, "1")]
    #[case::i32(1i32, "1")]
    #[case::f32(1.4f32, "1.4")]
    #[test_log::test]
    fn when_serialize_then_ok<'de, T>(#[case] content_value: T, #[case] content_text: &str)
    where
        T: std::fmt::Debug + PartialEq + Serialize + Deserialize<'de>,
    {
        let text = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><document content="{}" />"#,
            content_text
        );
        let value = Document {
            content: content_value,
        };

        assert_eq!(to_string(&value).unwrap(), text);
    }

    #[rstest]
    #[case::unit((), "")]
    #[case::string("abc".to_string(), "abc")]
    #[case::char('a', "a")]
    #[case::bool_true(true, "true")]
    #[case::bool_true(true, "1")]
    #[case::bool_false(false, "false")]
    #[case::bool_false(false, "0")]
    #[case::u8(1u8, "1")]
    #[case::i32(1i32, "1")]
    #[case::f32(1.4f32, "1.4")]
    #[test_log::test]
    fn when_deserialize_then_ok<'de, T>(#[case] content_value: T, #[case] content_text: &str)
    where
        T: std::fmt::Debug + PartialEq + Serialize + Deserialize<'de>,
    {
        let text = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><document content="{}" />"#,
            content_text
        );
        let value = Document {
            content: content_value,
        };

        assert_eq!(from_str::<Document<T>>(&text).unwrap(), value);
    }
}

mod given_nested_struct_with_simple_datatype_attribute {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document<T> {
        content: Content<T>,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Content<T> {
        #[serde(rename = "@a")]
        a: T,
    }

    #[rstest]
    #[case::unit((), "")]
    #[case::string("abc".to_string(), "abc")]
    #[case::char('a', "a")]
    #[case::bool_true(true, "true")]
    #[case::bool_false(false, "false")]
    #[case::u8(1u8, "1")]
    #[case::i32(1i32, "1")]
    #[case::f32(1.4f32, "1.4")]
    #[test_log::test]
    fn when_serialize_then_ok<'de, T>(#[case] content_value: T, #[case] content_text: &str)
    where
        T: std::fmt::Debug + PartialEq + Serialize + Deserialize<'de>,
    {
        let text = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><document><content a="{}" /></document>"#,
            content_text
        );
        let value = Document {
            content: Content { a: content_value },
        };

        assert_eq!(to_string(&value).unwrap(), text);
    }

    #[rstest]
    #[case::unit((), "")]
    #[case::string("abc".to_string(), "abc")]
    #[case::char('a', "a")]
    #[case::bool_true(true, "true")]
    #[case::bool_true(true, "1")]
    #[case::bool_false(false, "false")]
    #[case::bool_false(false, "0")]
    #[case::u8(1u8, "1")]
    #[case::i32(1i32, "1")]
    #[case::f32(1.4f32, "1.4")]
    #[test_log::test]
    fn when_deserialize_then_ok<'de, T>(#[case] content_value: T, #[case] content_text: &str)
    where
        T: std::fmt::Debug + PartialEq + Serialize + Deserialize<'de>,
    {
        let text = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><document><content a="{}" /></document>"#,
            content_text
        );
        let value = Document {
            content: Content { a: content_value },
        };

        assert_eq!(from_str::<Document<T>>(&text).unwrap(), value);
    }
}

mod given_struct_with_newtype_struct_attribute {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        #[serde(rename = "@content")]
        content: Content,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Content(String);

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document content="abc" />"#
    }

    #[fixture]
    fn value() -> Document {
        Document {
            content: Content("abc".to_string()),
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

mod given_struct_with_unit_struct_attribute {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        #[serde(rename = "@content")]
        content: Content,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Content;

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document content="" />"#
    }

    #[fixture]
    fn value() -> Document {
        Document { content: Content }
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

mod given_struct_with_tuple_attribute {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        #[serde(rename = "@content")]
        content: (String, i32),
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document content="abc 123" />"#
    }

    #[fixture]
    fn value() -> Document {
        Document {
            content: ("abc".to_string(), 123),
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

mod given_struct_with_tuple_struct_attribute {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        #[serde(rename = "@content")]
        content: Content,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Content(String, i32);

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document content="abc 123" />"#
    }

    #[fixture]
    fn value() -> Document {
        Document {
            content: Content("abc".to_string(), 123),
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
