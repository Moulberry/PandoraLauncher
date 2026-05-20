use crate::{from_str, to_string};
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename = "document")]
struct Document;

#[fixture]
fn text() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?><document />"#
}

#[fixture]
fn value() -> Document {
    Document
}

#[rstest]
#[test_log::test]
fn when_deserialize_then_empty_value(text: &str, value: Document) {
    assert_eq!(from_str::<Document>(text).unwrap(), value);
}

#[rstest]
#[test_log::test]
fn when_serialize_then_empty_value(text: &str, value: Document) {
    assert_eq!(to_string(&value).unwrap(), text);
}
