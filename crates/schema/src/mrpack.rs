use std::sync::Arc;

use indexmap::IndexMap;
use serde::Deserialize;

use crate::{fabric_mod::Person, modification::ModrinthModpackFileDownload};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthIndexJson {
    pub version_id: Arc<str>,
    pub name: Arc<str>,
    pub files: Arc<[ModrinthModpackFileDownload]>,
    pub dependencies: IndexMap<Arc<str>, Arc<str>>,

    // Unofficial
    #[serde(default, deserialize_with = "crate::try_deserialize")]
    pub authors: Option<Vec<Person>>,
    #[serde(default, deserialize_with = "crate::try_deserialize")]
    pub author: Option<Person>,
}
