use serde::de::{Deserialize, Deserializer, SeqAccess};
use serde_json::json;
use serde_untagged::UntaggedEnumVisitor;

#[test]
fn test_string_or_array_string() {
    #[derive(PartialEq, Debug)]
    enum Value {
        Single(String),
        Multiple(Vec<String>),
    }

    impl<'de> Deserialize<'de> for Value {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            UntaggedEnumVisitor::new()
                .string(|string| Ok(Value::Single(string.to_owned())))
                .seq(|mut seq| {
                    let mut array = Vec::new();
                    while let Some(element) = seq.next_element()? {
                        array.push(element);
                    }
                    Ok(Value::Multiple(array))
                })
                .deserialize(deserializer)
        }
    }

    let j = r#" "..." "#;
    let v: Value = serde_json::from_str(j).unwrap();
    assert_eq!(v, Value::Single("...".to_owned()));

    let j = r#" ["a","z"] "#;
    let v: Value = serde_json::from_str(j).unwrap();
    assert_eq!(v, Value::Multiple(vec!["a".to_owned(), "z".to_owned()]));
}

#[test]
fn test_borrowed() {
    #[derive(PartialEq, Debug)]
    enum Value<'de> {
        Single(&'de str),
        Multiple(Vec<&'de str>),
    }

    impl<'de> Deserialize<'de> for Value<'de> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            UntaggedEnumVisitor::new()
                .borrowed_str(|string| Ok(Value::Single(string)))
                .seq(|seq| seq.deserialize().map(Value::Multiple))
                .deserialize(deserializer)
        }
    }

    let j = &r#" "..." "#.to_owned();
    let v: Value = serde_json::from_str(j).unwrap();
    assert_eq!(v, Value::Single("..."));

    let j = &r#" ["a","z"] "#.to_owned();
    let v: Value = serde_json::from_str(j).unwrap();
    assert_eq!(v, Value::Multiple(vec!["a", "z"]));
}

#[test]
fn test_contains_map_key() {
    #[derive(PartialEq, Debug)]
    enum Response {
        Success(serde_json::Value),
        Failure(String),
    }

    impl<'de> Deserialize<'de> for Response {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            UntaggedEnumVisitor::new()
                .map(|map| {
                    let value: serde_json::Value = map.deserialize()?;
                    if let Ok(failure) = String::deserialize(&value["failure"]) {
                        Ok(Response::Failure(failure))
                    } else {
                        Ok(Response::Success(value))
                    }
                })
                .deserialize(deserializer)
        }
    }

    let j = &r#" {"failure":"..."} "#.to_owned();
    let v: Response = serde_json::from_str(j).unwrap();
    assert_eq!(v, Response::Failure("...".to_owned()));

    let j = &r#" {"ok":200} "#.to_owned();
    let v: Response = serde_json::from_str(j).unwrap();
    assert_eq!(v, Response::Success(json!({"ok":200})));
}

#[test]
fn test_expecting() {
    let error = UntaggedEnumVisitor::new()
        .seq(|_seq| Ok(()))
        .deserialize(&serde_json::Value::Null)
        .unwrap_err();
    let expected_message = "invalid type: null, expected an array";
    assert_eq!(error.to_string(), expected_message);

    let error = UntaggedEnumVisitor::new()
        .seq(|_seq| Ok(()))
        .bool(|_bool| Ok(()))
        .deserialize(&serde_json::Value::Null)
        .unwrap_err();
    let expected_message = "invalid type: null, expected a boolean or array";
    assert_eq!(error.to_string(), expected_message);

    let error = UntaggedEnumVisitor::new()
        .seq(|_seq| Ok(()))
        .bool(|_bool| Ok(()))
        .i8(|_int| Ok(()))
        .deserialize(&serde_json::Value::Null)
        .unwrap_err();
    let expected_message = "invalid type: null, expected a boolean, integer or array";
    assert_eq!(error.to_string(), expected_message);

    let error = UntaggedEnumVisitor::new()
        .seq(|_seq| Ok(()))
        .bool(|_bool| Ok(()))
        .i8(|_int| Ok(()))
        .i16(|_int| Ok(()))
        .deserialize(&serde_json::Value::Null)
        .unwrap_err();
    let expected_message = "invalid type: null, expected a boolean, integer or array";
    assert_eq!(error.to_string(), expected_message);

    let error = UntaggedEnumVisitor::<()>::new()
        .deserialize(&serde_json::Value::Null)
        .unwrap_err();
    let expected_message = "invalid type: null, expected unspecified";
    assert_eq!(error.to_string(), expected_message);

    let ty = "T";
    let error = UntaggedEnumVisitor::<()>::new()
        .expecting(format_args!("foo of type {ty}"))
        .deserialize(&serde_json::Value::Null)
        .unwrap_err();
    let expected_message = "invalid type: null, expected foo of type T";
    assert_eq!(error.to_string(), expected_message);
}
