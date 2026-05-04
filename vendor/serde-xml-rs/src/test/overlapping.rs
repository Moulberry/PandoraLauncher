use crate::SerdeXml;
use indoc::indoc;
use rstest::{fixture, rstest};
use serde::Deserialize;

mod given_overlapping_sequence_fields {
    use super::*;

    #[derive(Debug, PartialEq, Deserialize)]
    struct Document {
        a: Vec<String>,
        b: Vec<String>,
        c: String,
    }

    #[fixture]
    fn config() -> SerdeXml {
        SerdeXml::new().overlapping_sequences(true)
    }

    #[rstest]
    #[test_log::test]
    fn when_deserialize(config: SerdeXml) {
        let text = indoc!(
            r#"
            <document>
                <a>a1</a>
                <a>a2</a>
                <b>b1</b>
                <a>a3</a>
                <c>c</c>
                <b>b2</b>
                <a>a4</a>
            </document>
            "#
        );

        let document = Document {
            a: vec!["a1", "a2", "a3", "a4"]
                .into_iter()
                .map(ToString::to_string)
                .collect(),
            b: vec!["b1", "b2"]
                .into_iter()
                .map(ToString::to_string)
                .collect(),
            c: "c".to_string(),
        };

        assert_eq!(config.from_str::<Document>(text).unwrap(), document);
    }
}

mod given_out_of_order_tuple {

    use super::*;

    #[derive(Debug, PartialEq, Deserialize)]
    struct Document {
        #[serde(rename = "part")]
        parts: (A, B, C),
        other: A,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct A {
        #[serde(rename = "@a")]
        a: String,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct B {
        #[serde(rename = "@b")]
        b: String,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct C {
        #[serde(rename = "@c")]
        c: String,
    }

    #[fixture]
    fn config() -> SerdeXml {
        SerdeXml::new().overlapping_sequences(true)
    }

    #[rstest]
    #[test_log::test]
    #[ignore = "not sure this is the right design"]
    fn when_deserialize(config: SerdeXml) {
        let text = indoc!(
            r#"
            <document>
                <part a="a1" />
                <part b="b" />
                <other a="a2" />
                <part c="c" />
            </document>
            "#
        );

        let document = Document {
            parts: (
                A {
                    a: "a1".to_string(),
                },
                B { b: "b".to_string() },
                C { c: "c".to_string() },
            ),
            other: A {
                a: "a2".to_string(),
            },
        };

        assert_eq!(config.from_str::<Document>(text).unwrap(), document);
    }
}
