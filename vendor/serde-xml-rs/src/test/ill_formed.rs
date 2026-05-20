use crate::{from_str, Error};
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename = "document")]
struct Document {
    a: String,
    b: String,
    c: String,
}

#[rstest]
#[case::unfinished(r#"<document><a>a</a><b>b</b><c>c</c>"#)]
#[case::ill_formed_element(r#"<\u{0}:/"#)]
#[test_log::test]
fn given_ill_formed_document_when_deserialize_then_ko(#[case] text: &str) {
    assert!(matches!(from_str::<Document>(text), Err(Error::Reader(_))));
}

#[rstest]
#[case::attributes_instead_of_elements(r#"<document a="a" b="b" c="c" />"#)]
#[test_log::test]
fn given_mismatching_document_when_deserialize_then_ko(#[case] text: &str) {
    assert!(matches!(from_str::<Document>(text), Err(Error::Custom(_))));
}
