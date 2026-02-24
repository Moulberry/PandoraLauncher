use std::{collections::BTreeSet, sync::Arc};

use enumset::{EnumSet, EnumSetType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct BackendConfig {
    #[serde(default, skip_serializing_if = "is_default_sync_targets", deserialize_with = "try_deserialize_sync_targets")]
    pub sync_targets: SyncTargets,
    #[serde(default, skip_serializing_if = "crate::skip_if_default", deserialize_with = "crate::try_deserialize")]
    pub dont_open_game_output_when_launching: bool,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct SyncTargets {
    pub files: BTreeSet<Arc<str>>,
    pub folders: BTreeSet<Arc<str>>,
}

fn is_default_sync_targets(sync_targets: &SyncTargets) -> bool {
    sync_targets.files.is_empty() && sync_targets.folders.is_empty()
}

fn try_deserialize_sync_targets<'de, D>(deserializer: D) -> Result<SyncTargets, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;

    // Migration from previous bitset
    if let serde_json::Value::Number(_) = value {
        let Ok(legacy) = EnumSet::<LegacySyncTarget>::deserialize(value) else {
            return Ok(SyncTargets::default());
        };

        let mut targets = SyncTargets::default();
        for legacy_target in legacy {
            let (name, file) = legacy_target.get_new_target();
            if file {
                targets.files.insert(name.into());
            } else {
                targets.folders.insert(name.into());
            }
        }
        return Ok(targets);
    }

    Ok(SyncTargets::deserialize(value).unwrap_or_default())
}

#[derive(Debug, enum_map::Enum, EnumSetType, strum::EnumIter)]
enum LegacySyncTarget {
    Options = 0,
    Servers = 1,
    Commands = 2,
    Hotbars = 13,
    Saves = 3,
    Config = 4,
    Screenshots = 5,
    Resourcepacks = 6,
    Shaderpacks = 7,
    Flashback = 8,
    DistantHorizons = 9,
    Voxy = 10,
    XaerosMinimap = 11,
    Bobby = 12,
    Litematic = 14,
}

impl LegacySyncTarget {
    pub fn get_new_target(self) -> (&'static str, bool) {
        match self {
            LegacySyncTarget::Options => ("options.txt", true),
            LegacySyncTarget::Servers => ("servers.dat", true),
            LegacySyncTarget::Commands => ("command_history.txt", true),
            LegacySyncTarget::Hotbars => ("hotbar.nbt", true),
            LegacySyncTarget::Saves => ("saves", false),
            LegacySyncTarget::Config => ("config", false),
            LegacySyncTarget::Screenshots => ("screenshots", false),
            LegacySyncTarget::Resourcepacks => ("resourcepacks", false),
            LegacySyncTarget::Shaderpacks => ("shaderpacks", false),
            LegacySyncTarget::Flashback => ("flashback", false),
            LegacySyncTarget::DistantHorizons => ("Distant_Horizons_server_data", false),
            LegacySyncTarget::Voxy => (".voxy", false),
            LegacySyncTarget::XaerosMinimap => ("xaero", false),
            LegacySyncTarget::Bobby => (".bobby", false),
            LegacySyncTarget::Litematic => ("schematics", false),
        }
    }
}
