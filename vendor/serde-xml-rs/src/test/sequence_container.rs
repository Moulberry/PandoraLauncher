use crate::from_str;
use rstest::rstest;
use serde::Deserialize;

#[derive(Debug, PartialEq, Deserialize)]
struct Document {
    body: Body,
}

#[derive(Debug, PartialEq, serde_query::Deserialize)]
struct Body {
    #[query(".parts.part")]
    parts: Vec<Part>,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Part {
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "#text", default)]
    content: String,
}

#[rstest]
#[test_log::test]
fn test() {
    let text = r#"
        <document>
            <body>
                <parts>
                    <part id="123"></part>
                    <part id="124">abc</part>
                </parts>
            </body>
        </document>
        "#;

    let document: Document = from_str(text).unwrap();
    assert_eq!(document.body.parts.len(), 2);
    assert_eq!(document.body.parts[0].id, "123".to_string());
    assert_eq!(document.body.parts[1].id, "124".to_string());
}
