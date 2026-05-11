use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentSource {
    #[default]
    Manual,
    ModrinthUnknown,
    ModrinthProject {
        project_id: Arc<str>
    },
    CurseforgeProject {
        project_id: u32,
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentInstallReason {
    Standalone,
    Dependency,
    Modpack,
    Update,
}
