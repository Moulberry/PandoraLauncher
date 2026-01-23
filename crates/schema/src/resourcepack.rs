use std::sync::Arc;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct PackMcmeta {
    pub pack: PackMcmetaPack,
}
#[derive(Deserialize, Debug)]
pub struct PackMcmetaPack {
    pub description: Arc<str>,
}
