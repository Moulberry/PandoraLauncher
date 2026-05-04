use crate::from_str;
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};

mod given_root_struct {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        a: String,
        z: String,
    }

    #[fixture]
    fn value() -> Document {
        Document {
            a: "a".to_string(),
            z: "z".to_string(),
        }
    }

    #[rstest]
    #[case::ignore_simple_element(r#"<document><a>a</a><b>b</b><z>z</z></document>"#)]
    #[case::ignore_attribute(r#"<document b="b"><a>a</a><z>z</z></document>"#)]
    #[case::ignore_nested_elements(
        r#"<document><a>a</a><b><c></c><d><e>eeee</e></d></b><z>z</z></document>"#
    )]
    #[test_log::test]
    fn when_deserialize(#[case] text: &str, value: Document) {
        let text = format!(r#"<?xml version="1.0" encoding="UTF-8"?>{}"#, text);
        assert_eq!(from_str::<Document>(&text).unwrap(), value);
    }
}
