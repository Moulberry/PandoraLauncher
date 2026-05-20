use rstest::rstest;
use serde::Deserialize;
use serde_xml_rs::from_reader;
use std::{fs::File, path::PathBuf};

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename = "methodCall", rename_all = "camelCase")]
struct MethodCall {
    method_name: String,
    params: Params,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename = "methodResponse", rename_all = "camelCase")]
enum MethodResponse {
    Params(Params),
    Fault(Fault),
}

#[derive(Debug, PartialEq, Deserialize)]
struct Params {
    param: Vec<Param>,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Param {
    value: Value,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Value {
    #[serde(rename = "i4", alias = "int")]
    Int(i32),
    Double(f64),
    Boolean(bool),
    #[serde(alias = "#text")]
    String(String),
    #[serde(rename = "datetime.iso8601")]
    DateTime(String),
    Base64(String),
    Array(Array),
    Struct(Struct),
}

#[derive(Debug, PartialEq, Deserialize)]
struct Array {
    data: ArrayData,
}

#[derive(Debug, PartialEq, Deserialize)]
struct ArrayData {
    #[serde(rename = "value")]
    values: Vec<Value>,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Struct {
    #[serde(rename = "member")]
    members: Vec<Member>,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Member {
    name: String,
    value: Value,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Fault {
    value: Value,
}

#[rstest]
#[test_log::test]
fn given_method_call_when_deserialize_ok(#[files("tests/xml_rpc/method_call*.xml")] path: PathBuf) {
    let file = File::open(path).unwrap();
    let _value: MethodCall = from_reader(file).unwrap();
}

#[rstest]
#[test_log::test]
fn given_method_reponse_when_deserialize_ok(
    #[files("tests/xml_rpc/method_response*.xml")] path: PathBuf,
) {
    let file = File::open(path).unwrap();
    let _value: MethodResponse = from_reader(file).unwrap();
}
