use std::{collections::{BTreeMap, BTreeSet}, sync::Arc};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AuxiliaryContentMeta {
    #[serde(default, skip_serializing_if = "crate::skip_if_default", deserialize_with = "crate::try_deserialize")]
    pub applied_overrides: AuxAppliedOverrides,
    #[serde(default, skip_serializing_if = "crate::skip_if_default", deserialize_with = "crate::try_deserialize")]
    pub disabled_children: AuxDisabledChildren,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuxAppliedOverrides {
    pub filename_to_hash: BTreeMap<Arc<str>, Arc<str>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuxDisabledChildren {
    // For files that are enabled by default
    pub disabled_ids: BTreeSet<Arc<str>>,
    pub disabled_names: BTreeSet<Arc<str>>,
    pub disabled_filenames: BTreeSet<Arc<str>>,
    // For files that are disabled by default
    pub enabled_ids: BTreeSet<Arc<str>>,
    pub enabled_names: BTreeSet<Arc<str>>,
    pub enabled_filenames: BTreeSet<Arc<str>>,
}

impl AuxDisabledChildren {
    pub fn is_enabled(&self, disabled_default: bool, id: Option<&str>, name: Option<&str>, filename: &str) -> bool {
        if disabled_default {
            if let Some(id) = id && self.enabled_ids.contains(id) {
                return true;
            }
            if let Some(name) = name && self.enabled_names.contains(name) {
                return true;
            }
            self.enabled_filenames.contains(filename)
        } else {
            if let Some(id) = id && self.disabled_ids.contains(id) {
                return false;
            }
            if let Some(name) = name && self.disabled_names.contains(name) {
                return false;
            }
            !self.disabled_filenames.contains(filename)
        }
    }
}
