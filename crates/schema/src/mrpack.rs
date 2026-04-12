use std::sync::Arc;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{fabric_mod::Person, modification::ModrinthModpackFileDownload};

fn default_format_version() -> u32 {
    1
}

fn default_game() -> Arc<str> {
    "minecraft".into()
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthIndexJson {
    #[serde(default = "default_format_version")]
    pub format_version: u32,
    #[serde(default = "default_game")]
    pub game: Arc<str>,
    pub version_id: Arc<str>,
    pub name: Arc<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<Arc<str>>,
    pub files: Arc<[ModrinthModpackFileDownload]>,
    pub dependencies: IndexMap<Arc<str>, Arc<str>>,

    // Unofficial
    #[serde(default, deserialize_with = "crate::try_deserialize", skip_serializing)]
    pub authors: Option<Vec<Person>>,
    #[serde(default, deserialize_with = "crate::try_deserialize", skip_serializing)]
    pub author: Option<Person>,
}
