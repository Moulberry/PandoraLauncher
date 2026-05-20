use crate::from_str;
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Serialize, Deserialize, Debug)]
struct Envelope {
    subject: String,
}

#[fixture]
fn value() -> Envelope {
    Envelope {
        subject: "Reference rates".to_string(),
    }
}

#[rstest]
#[case(
    r#"<?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE Envelope>
            <Envelope>
            <subject>Reference rates</subject>
            </Envelope>"#
)]
#[case(
    r#"<?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE Envelope[]>
            <Envelope>
            <subject>Reference rates</subject>
            </Envelope>"#
)]
#[case(
    r#"<?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE Envelope [
                <!ELEMENT subject (#PCDATA)>
            ] >
            <Envelope>
            <subject>Reference rates</subject>
            </Envelope>"#
)]
#[test_log::test]
fn given_document_with_doctype_when_deserialize_then_ok(#[case] text: &str, value: Envelope) {
    assert_eq!(from_str::<Envelope>(text).unwrap(), value);
}
