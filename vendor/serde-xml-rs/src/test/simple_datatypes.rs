use crate::{from_str, to_string};
use rstest::rstest;
use serde::{Deserialize, Serialize};

mod given_simple_type {
    use super::*;

    #[rstest]
    #[case::string("<bla>This is a String</bla>", "This is a String".to_string())]
    #[case::string("<bla></bla>", "".to_string())]
    #[case::string("<bla>    </bla>", "".to_string())]
    #[case::string("<bla>&lt;boom/&gt;</bla>", "<boom/>".to_string())]
    #[case::string("<bla>&#9835;</bla>", "♫".to_string())]
    #[case::string("<bla>&#x266B;</bla>", "♫".to_string())]
    #[case::string("<bla>♫<![CDATA[<cookies/>]]>♫</bla>", "♫<cookies/>♫".to_string())]
    #[case::i64("<bla>0</bla>", 0i64)]
    #[case::i64("<bla>-2</bla>", -2i64)]
    #[case::i64("<bla>-1234</bla>", -1234i64)]
    #[case::i64("<bla> -1234 </bla>", -1234i64)]
    #[case::u64("<bla>0</bla>", 0u64)]
    #[case::u64("<bla>1234</bla>", 1234u64)]
    #[case::u64("<bla> 1234 </bla>", 1234u64)]
    #[case::bool("<bla>true</bla>", true)]
    #[case::bool("<bla>false</bla>", false)]
    #[case::unit("<bla/>", ())]
    #[case::f64("<bla>3.0</bla>", 3.0f64)]
    #[case::f64("<bla>3.1</bla>", 3.1f64)]
    #[case::f64("<bla>-1.2</bla>", -1.2f64)]
    #[case::f64("<bla>0.4</bla>", 0.4f64)]
    #[case::f64("<bla>0.4e5</bla>", 0.4e5f64)]
    #[case::f64("<bla>0.4e15</bla>", 0.4e15f64)]
    #[case::f64_precision_troubles("<bla>0.4e-01</bla>", 0.4e-01f64)]
    #[case::f64("<bla> 0.4e-01 </bla>", 0.4e-01f64)]
    #[case::option("<bla/>", Some("".to_string()))]
    #[case::option("<bla></bla>", Some("".to_string()))]
    #[case::option("<bla> </bla>", Some("".to_string()))]
    #[case::option("<bla>42</bla>", Some("42".to_string()))]
    #[test_log::test]
    fn when_deserialize_then_ok<T, 'de>(#[case] document: &str, #[case] expected: T)
    where
        T: std::fmt::Debug + PartialEq + Deserialize<'de>,
    {
        let actual: T = from_str(document).unwrap();
        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::string("This is a String".to_string())]
    #[case::string("".to_string())]
    #[case::i64(0i64)]
    #[case::u64(0u64)]
    #[case::bool(true)]
    #[case::bool(false)]
    #[case::unit(())]
    #[case::f64(3.0f64)]
    #[case::option(Some("".to_string()))]
    #[case::option(Some(42))]
    #[test_log::test]
    fn when_serialize_then_ko<T>(#[case] value: T)
    where
        T: std::fmt::Debug + PartialEq + Serialize,
    {
        assert!(to_string(&value).is_err());
    }
}

mod given_newtype_simple_type {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename = "document")]
    struct Document<T>(T);

    #[rstest]
    #[test_log::test]
    #[case::string("abc".to_string(), "abc")]
    #[case::string("".to_string(), "")]
    #[case::string("<>&".to_string(), "&lt;&gt;&amp;")]
    #[case::string("♫".to_string(), "♫")]
    #[case::byte_buf(serde_bytes::ByteBuf::from("abc"), "abc")]
    #[case::char('a', "a")]
    #[case::bool_true(true, "true")]
    #[case::bool_false(false, "false")]
    #[case::u8(1u8, "1")]
    #[case::i32(1i32, "1")]
    #[case::f32(1.4f32, "1.4")]
    #[case::option_string(Some("abc".to_string()), "abc")]
    #[case::option_i32(Some(1i32), "1")]
    fn when_serialize_then_ok<'de, T>(#[case] content_value: T, #[case] content_text: &str)
    where
        T: std::fmt::Debug + PartialEq + Serialize + Deserialize<'de>,
    {
        let text = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><document>{}</document>"#,
            content_text
        );
        let value = Document(content_value);

        assert_eq!(to_string(&value).unwrap(), text);
    }

    #[rstest]
    #[case::string("abc".to_string(), "abc")]
    #[case::string("".to_string(), "")]
    #[case::string("".to_string(), "      ")]
    #[case::string("<>&".to_string(), "&lt;&gt;&amp;")]
    #[case::string("♫".to_string(), "♫")]
    #[case::byte_buf(serde_bytes::ByteBuf::from("abc"), "abc")]
    #[case::char('a', "a")]
    #[case::bool_true(true, "true")]
    #[case::bool_true(true, "1")]
    #[case::bool_false(false, "false")]
    #[case::bool_false(false, "0")]
    #[case::u8(1u8, "1")]
    #[case::i32(1i32, "1")]
    #[case::f32(1.4f32, "1.4")]
    #[case::option_string(Some("abc".to_string()), "abc")]
    #[case::option_i32(Some(1i32), "1")]
    #[test_log::test]
    fn when_deserialize_then_ok<'de, T>(#[case] content_value: T, #[case] content_text: &str)
    where
        T: std::fmt::Debug + PartialEq + Serialize + Deserialize<'de>,
    {
        let text = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><document>{}</document>"#,
            content_text
        );
        let value = Document(content_value);

        assert_eq!(from_str::<Document<T>>(&text).unwrap(), value);
    }

    #[rstest]
    #[case::not_a_bool(true, "verum")]
    #[test_log::test]
    fn when_deserialize_then_ko<'de, T>(#[case] _content_value: T, #[case] content_text: &str)
    where
        T: std::fmt::Debug + PartialEq + Serialize + Deserialize<'de>,
    {
        let text = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><document>{}</document>"#,
            content_text
        );
        assert!(from_str::<Document<T>>(&text).is_err());
    }
    #[rstest]
    #[case::unit(())]
    #[case::option_none(Option::<String>::None)]
    #[test_log::test]
    fn given_empty_value_when_serialize_then_empty_tag<'de, T>(#[case] content_value: T)
    where
        T: std::fmt::Debug + PartialEq + Serialize + Deserialize<'de>,
    {
        let text = r#"<?xml version="1.0" encoding="UTF-8"?><document />"#;
        let value = Document(content_value);

        assert_eq!(to_string(&value).unwrap(), text);
    }

    #[rstest]
    #[case::unit(())]
    #[case::option_none(Option::<String>::None)]
    #[test_log::test]
    fn given_empty_tag_when_deserialize_then_empty_value<'de, T>(#[case] content_value: T)
    where
        T: std::fmt::Debug + PartialEq + Serialize + Deserialize<'de>,
    {
        let text = r#"<?xml version="1.0" encoding="UTF-8"?><document />"#;
        let value = Document(content_value);

        assert_eq!(from_str::<Document<T>>(text).unwrap(), value);
    }
}
