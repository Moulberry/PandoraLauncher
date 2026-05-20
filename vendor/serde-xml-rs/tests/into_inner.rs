use rstest::rstest;
use serde::Serialize;
use serde_xml_rs::Serializer;
use std::io::Write;

#[derive(Serialize)]
#[serde(rename = "root")]
struct Document {
    value: String,
}

#[rstest]
#[test_log::test]
fn test_serialize_and_get_buffer_back() {
    let buffer = Vec::new();
    let mut serializer = Serializer::new_from_writer(buffer);

    let doc = Document {
        value: "test".to_string(),
    };

    doc.serialize(&mut serializer).unwrap();

    let recovered_buffer = serializer.into_inner();

    let xml_string = String::from_utf8(recovered_buffer).unwrap();
    assert_eq!(
        xml_string,
        r#"<?xml version="1.0" encoding="UTF-8"?><root><value>test</value></root>"#
    );
}

#[rstest]
#[test_log::test]
fn test_continue_writing_after_serialization() {
    let buffer = Vec::new();
    let mut serializer = Serializer::new_from_writer(buffer);

    let doc = Document {
        value: "first".to_string(),
    };

    doc.serialize(&mut serializer).unwrap();

    let mut recovered_buffer = serializer.into_inner();
    recovered_buffer.write_all(b"\n<!-- comment -->").unwrap();

    let xml_string = String::from_utf8(recovered_buffer).unwrap();
    assert_eq!(
        xml_string,
        r#"<?xml version="1.0" encoding="UTF-8"?><root><value>first</value></root>
<!-- comment -->"#
    );
}
