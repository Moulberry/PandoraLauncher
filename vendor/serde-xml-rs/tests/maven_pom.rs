use rstest::rstest;
use serde::Deserialize;
use serde_xml_rs::from_reader;
use std::{collections::BTreeMap, fs::File};

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename = "project", rename_all = "camelCase")]
struct Project {
    model_version: String,
    parent: Option<Parent>,
    group_id: Option<String>,
    artifact_id: String,
    version: Option<String>,
    name: Option<String>,
    description: Option<String>,
    url: Option<String>,
    licenses: Option<Licenses>,
    developers: Option<Developers>,
    scm: Option<Scm>,
    properties: Option<BTreeMap<String, String>>,
    dependencies: Option<Dependencies>,
    build: Option<Build>,
    dependency_management: Option<DependencyManagement>,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Parent {
    group_id: String,
    artifact_id: String,
    version: String,
    relative_path: Option<String>,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Licenses {
    #[serde(rename = "license")]
    licenses: String,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Developers {
    #[serde(rename = "developer")]
    developers: String,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Scm {
    connection: Option<String>,
    developer_connection: Option<String>,
    tag: Option<String>,
    url: Option<String>,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Dependencies {
    #[serde(rename = "dependency")]
    dependencies: Vec<Dependency>,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Dependency {
    group_id: String,
    artifact_id: String,
    version: Option<String>,
    //#[serde(with = "text_enum", default)]
    scope: Option<DependencyScope>,
    optional: Option<bool>,
}

pub mod text_enum {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    pub struct Text<T> {
        #[serde(rename = "#text")]
        pub text: T,
    }

    pub fn serialize<S, T>(value: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
    {
        value
            .as_ref()
            .map(|x| Text { text: x })
            .serialize(serializer)
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        let value = Option::<Text<T>>::deserialize(deserializer)?;
        Ok(value.map(|x| x.text))
    }
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", variant_identifier)]
enum DependencyScope {
    Compile,
    Import,
    Provided,
    Runtime,
    System,
    Test,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Build {}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DependencyManagement {
    dependencies: Dependencies,
}

#[rstest]
#[test_log::test]
fn when_deserialize() {
    let file = File::open("tests/maven_pom/pom.xml").unwrap();
    let _value: Project = from_reader(file).unwrap();
}
