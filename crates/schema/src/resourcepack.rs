use serde::Deserialize;

use crate::text_component::FlatTextComponent;

#[derive(Deserialize, Debug)]
pub struct PackMcmeta {
    pub pack: PackMcmetaPack,
}
#[derive(Deserialize, Debug)]
pub struct PackMcmetaPack {
    #[serde(deserialize_with = "crate::text_component::deserialize_flat_text_component_json")]
    pub description: FlatTextComponent,
}
