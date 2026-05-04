use rstest::rstest;
use serde::Deserialize;
use serde_xml_rs::from_reader;
use std::{fs::File, path::PathBuf};

#[derive(Debug, PartialEq, Deserialize)]
struct Document {
    content: String,
}

#[rstest]
#[test_log::test]
fn given_supported_encoding_when_deserialize_then_ok(
    #[files("tests/encodings/*.xml")] path: PathBuf,
) {
    let expected = Document {
        content: "abc".to_string(),
    };
    let file = File::open(path).unwrap();
    let actual: Document = from_reader(file).unwrap();

    assert_eq!(actual, expected);
}
