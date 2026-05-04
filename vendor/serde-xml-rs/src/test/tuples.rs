use crate::{from_str, to_string};
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};

mod given_root_tuple {
    use super::*;

    type Document = (i32, i32, i32);

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><any-element>1 0 -4</any-element>"#
    }

    #[fixture]
    fn value() -> Document {
        (1, 0, -4)
    }

    #[rstest]
    #[test_log::test]
    fn when_deserialize(text: &str, value: Document) {
        assert_eq!(from_str::<Document>(text).unwrap(), value);
    }

    #[rstest]
    #[test_log::test]
    fn when_serialize_ko(value: Document) {
        assert!(to_string(&value).is_err());
    }
}

mod given_simple_tuple_struct {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document(i32, i32, i32);

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document>1 0 -4</document>"#
    }

    #[fixture]
    fn value() -> Document {
        Document(1, 0, -4)
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

mod given_struct_with_tuple_field {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        coordinates: (i32, i32, i32),
    }

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document><coordinates>1 0 -4</coordinates></document>"#
    }

    #[fixture]
    fn value() -> Document {
        Document {
            coordinates: (1, 0, -4),
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

mod given_struct_with_tuple_struct_field {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        coordinates: Coordinates,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Coordinates(i32, i32, i32);

    #[fixture]
    fn text() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?><document><coordinates>1 0 -4</coordinates></document>"#
    }

    #[fixture]
    fn value() -> Document {
        Document {
            coordinates: Coordinates(1, 0, -4),
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
