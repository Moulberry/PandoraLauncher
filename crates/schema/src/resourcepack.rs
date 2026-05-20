use serde::Deserialize;

use crate::text_component::{FlatTextComponent, deserialize_flat_text_component_json};

#[derive(Deserialize, Debug)]
pub struct PackMcmeta {
    pub pack: PackMcmetaPack,
}

#[derive(Deserialize, Debug)]
pub struct PackMcmetaPack {
    /// Description can be a string or a text component (object/array) in newer pack formats
    #[serde(deserialize_with = "deserialize_flat_text_component_json")]
    pub description: FlatTextComponent,
}
