use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentSource {
    #[default]
    Manual,
    ModrinthUnknown,
    ModrinthProject {
        project: Arc<str>
    }
}
