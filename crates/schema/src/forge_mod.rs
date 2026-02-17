use std::sync::Arc;

use serde::Deserialize;

use crate::fabric_mod::Person;

#[derive(Deserialize, Debug)]
pub struct ModsToml {
    pub mods: Vec<ModsTomlMod>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModsTomlMod {
    pub mod_id: Arc<str>,
    pub display_name: Option<Arc<str>>,
    pub logo_file: Option<Arc<str>>,
    pub version: Option<Arc<str>>,
    pub authors: Option<Arc<str>>,
}

#[derive(Deserialize, Debug)]
pub struct JarJarMetadata {
    pub jars: Vec<JarJarMetadataJar>
}

#[derive(Deserialize, Debug)]
pub struct JarJarMetadataJar {
    pub path: Arc<str>,
}

#[derive(Deserialize, Debug)]
#[serde(transparent)]
pub struct McModInfo(pub Vec<McModInfoMod>);

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct McModInfoMod {
    pub modid: Arc<str>,
    pub name: Arc<str>,
    pub logo_file: Option<Arc<str>>,
    pub version: Option<Arc<str>>,
    pub author_list: Option<Vec<Person>>,
}
