use crate::{from_reader, to_writer};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Event {
    #[serde(rename = "@type")]
    r#type: String,
    #[serde(rename = "@timestamp")]
    timestamp: String,
    #[serde(rename = "#text")]
    message: String,
}

#[rstest]
#[test_log::test]
fn given_input_with_multiple_root_elements_when_deserializing_muliple_times_then_ok() {
    let text = r#"
        <event type="a" timestamp="2000-01-01T00:00Z">an event</event>
        <event type="b" timestamp="2000-01-01T00:01Z">another event</event>
        "#;

    let mut r = Cursor::new(text);
    from_reader::<Event, _>(&mut r).unwrap();
    from_reader::<Event, _>(&mut r).unwrap();
}

#[rstest]
#[test_log::test]
fn given_multiple_values_when_serializing_multiple_times_then_ok() {
    let mut output = Vec::new();
    let event1 = Event {
        r#type: "a".to_string(),
        timestamp: "2000-01-01T00:00Z".to_string(),
        message: "an event".to_string(),
    };
    let event2 = Event {
        r#type: "b".to_string(),
        timestamp: "2000-01-01T00:01Z".to_string(),
        message: "another event".to_string(),
    };
    to_writer(&mut output, &event1).unwrap();
    to_writer(&mut output, &event2).unwrap();
}
