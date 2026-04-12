use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::modrinth::{ModrinthHashes, ModrinthSideRequirement};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthModpackFileDownload {
    pub path: Arc<str>,
    pub hashes: ModrinthHashes,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<ModrinthEnv>,
    pub downloads: Arc<[Arc<str>]>,
    pub file_size: usize,
}

fn default_required_side() -> ModrinthSideRequirement {
    ModrinthSideRequirement::Required
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct ModrinthEnv {
    pub client: ModrinthSideRequirement,
    #[serde(default = "default_required_side")]
    pub server: ModrinthSideRequirement,
}
