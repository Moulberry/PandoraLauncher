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
    pub disabled_ids: BTreeSet<Arc<str>>,
    pub disabled_names: BTreeSet<Arc<str>>,
    pub disabled_filenames: BTreeSet<Arc<str>>,
}
