use crate::{from_str, to_string};
use rstest::rstest;
use serde::{Deserialize, Serialize};

mod given_root_choice {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document", rename_all = "kebab-case")]
    enum Document {
        Unit,
        Newtype(String),
        EmptyStruct {},
        Tuple(i32, i32),
        Struct {
            a: String,
            b: i32,
        },
        StructWithAttribute {
            #[serde(rename = "@id")]
            id: i32,
        },
    }

    #[rstest]
    #[test_log::test]
    #[case::unit(r#"<unit />"#, Document::Unit)]
    #[case::newtype(r#"<newtype>abc</newtype>"#, Document::Newtype("abc".to_string()))]
    #[case::empty_struct(r#"<empty-struct />"#, Document::EmptyStruct {  })]
    #[case::tuple(r#"<tuple>1 0</tuple>"#, Document::Tuple(1, 0))]
    #[case::struct_(r#"<struct><a>abc</a><b>123</b></struct>"#, Document::Struct { a: "abc".to_string(), b: 123 })]
    #[case::struct_with_attribute(r#"<struct-with-attribute id="123" />"#, Document::StructWithAttribute { id: 123 })]
    fn when_deserialize(#[case] content_text: &str, #[case] value: Document) {
        let text =
            format!(r#"<?xml version="1.0" encoding="UTF-8"?><document>{content_text}</document>"#);
        assert_eq!(from_str::<Document>(&text).unwrap(), value);
    }

    #[rstest]
    #[test_log::test]
    #[case::unit(r#"<unit />"#, Document::Unit)]
    #[case::newtype(r#"<newtype>abc</newtype>"#, Document::Newtype("abc".to_string()))]
    #[case::empty_struct(r#"<empty-struct />"#, Document::EmptyStruct {  })]
    #[case::tuple(r#"<tuple>1 0</tuple>"#, Document::Tuple(1, 0))]
    #[case::struct_(r#"<struct><a>abc</a><b>123</b></struct>"#, Document::Struct { a: "abc".to_string(), b: 123 })]
    #[case::struct_with_attribute(r#"<struct-with-attribute id="123" />"#, Document::StructWithAttribute { id: 123 })]
    fn when_serialize(#[case] content_text: &str, #[case] value: Document) {
        let text =
            format!(r#"<?xml version="1.0" encoding="UTF-8"?><document>{content_text}</document>"#);
        assert_eq!(to_string(&value).unwrap(), text);
    }
}

mod given_child_element_choice {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        content: Content,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Content {
        Unit,
        Newtype(String),
        EmptyStruct {},
        Tuple(i32, i32),
        Struct {
            a: String,
            b: i32,
        },
        StructWithAttribute {
            #[serde(rename = "@id")]
            id: i32,
        },
    }

    #[rstest]
    #[test_log::test]
    #[case::unit(r#"<unit />"#, Content::Unit)]
    #[case::newtype(r#"<newtype>abc</newtype>"#, Content::Newtype("abc".to_string()))]
    #[case::empty_struct(r#"<empty-struct />"#, Content::EmptyStruct {  })]
    #[case::tuple(r#"<tuple>1 0</tuple>"#, Content::Tuple(1, 0))]
    #[case::struct_(r#"<struct><a>abc</a><b>123</b></struct>"#, Content::Struct { a: "abc".to_string(), b: 123 })]
    #[case::struct_with_attribute(r#"<struct-with-attribute id="123" />"#, Content::StructWithAttribute { id: 123 })]
    fn when_deserialize(#[case] content_text: &str, #[case] content_value: Content) {
        let text = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><document><content>{content_text}</content></document>"#
        );
        let value = Document {
            content: content_value,
        };
        assert_eq!(from_str::<Document>(&text).unwrap(), value);
    }

    #[rstest]
    #[test_log::test]
    #[case::unit(r#"<unit />"#, Content::Unit)]
    #[case::newtype(r#"<newtype>abc</newtype>"#, Content::Newtype("abc".to_string()))]
    #[case::empty_struct(r#"<empty-struct />"#, Content::EmptyStruct {  })]
    #[case::tuple(r#"<tuple>1 0</tuple>"#, Content::Tuple(1, 0))]
    #[case::struct_(r#"<struct><a>abc</a><b>123</b></struct>"#, Content::Struct { a: "abc".to_string(), b: 123 })]
    #[case::struct_with_attribute(r#"<struct-with-attribute id="123" />"#, Content::StructWithAttribute { id: 123 })]
    fn when_serialize(#[case] content_text: &str, #[case] content_value: Content) {
        let text = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><document><content>{content_text}</content></document>"#
        );
        let value = Document {
            content: content_value,
        };
        assert_eq!(to_string(&value).unwrap(), text);
    }
}

mod given_attribute_unit_choices {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        #[serde(rename = "@content")]
        content: Content,
    }
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Content {
        A,
        B,
        C,
    }

    #[rstest]
    #[test_log::test]
    #[case::a(r#"a"#, Content::A)]
    #[case::b(r#"b"#, Content::B)]
    #[case::c(r#"c"#, Content::C)]
    fn when_deserialize(#[case] content_text: &str, #[case] content_value: Content) {
        let text = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><document content="{content_text}" />"#
        );
        let value = Document {
            content: content_value,
        };
        assert_eq!(from_str::<Document>(&text).unwrap(), value);
    }

    #[rstest]
    #[test_log::test]
    #[case::a(r#"a"#, Content::A)]
    #[case::b(r#"b"#, Content::B)]
    #[case::c(r#"c"#, Content::C)]
    fn when_serialize(#[case] content_text: &str, #[case] content_value: Content) {
        let text = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><document content="{content_text}" />"#
        );
        let value = Document {
            content: content_value,
        };
        assert_eq!(to_string(&value).unwrap(), text);
    }
}

mod given_unit_choices_as_string {
    use super::*;

    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(rename = "document")]
    struct Document {
        #[serde(rename = "content")]
        content: Content,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(rename_all = "kebab-case", variant_identifier)]
    enum Content {
        A,
        B,
        C,
    }

    #[rstest]
    #[test_log::test]
    #[case::a(r#"a"#, Content::A)]
    #[case::b(r#"b"#, Content::B)]
    #[case::c(r#"c"#, Content::C)]
    fn when_deserialize(#[case] content_text: &str, #[case] content_value: Content) {
        let text = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><document><content>{}</content></document>"#,
            content_text
        );
        let value = Document {
            content: content_value,
        };
        assert_eq!(from_str::<Document>(&text).unwrap(), value);
    }
}
