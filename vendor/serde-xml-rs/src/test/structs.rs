use crate::{from_str, to_string};
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};

mod given_struct_with_single_field {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        content: String,
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document><content>abc</content></document>"#
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

mod given_struct_with_multiple_fields {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        a: String,
        b: String,
        c: String,
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document><a>a</a><b>b</b><c>c</c></document>"#
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
        content: String,
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document><part><content>abc</content></part></document>"#
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
