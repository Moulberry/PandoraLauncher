use enumset::{EnumSet, EnumSetType};
use serde::{Deserialize, Serialize};
use crate::instance::{InstanceMemoryConfiguration, InstanceJvmFlagsConfiguration, InstanceJvmBinaryConfiguration};

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct BackendConfig {
    #[serde(default, skip_serializing_if = "crate::skip_if_default", deserialize_with = "crate::try_deserialize")]
    pub sync_targets: EnumSet<SyncTarget>,
    #[serde(default, skip_serializing_if = "crate::skip_if_default", deserialize_with = "crate::try_deserialize")]
    pub dont_open_game_output_when_launching: bool,
    #[serde(default)]
    pub global_memory_enabled: bool,
    #[serde(default)]
    pub global_memory: Option<InstanceMemoryConfiguration>,
    #[serde(default)]
    pub global_jvm_flags_enabled: bool,
    #[serde(default)]
    pub global_jvm_flags: Option<InstanceJvmFlagsConfiguration>,
    #[serde(default)]
    pub global_jvm_binary_enabled: bool,
    #[serde(default)]
    pub global_jvm_binary: Option<InstanceJvmBinaryConfiguration>,
}

#[derive(Debug, enum_map::Enum, EnumSetType, strum::EnumIter)]
pub enum SyncTarget {
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

impl SyncTarget {
    pub fn get_folder(self) -> Option<&'static str> {
        match self {
            SyncTarget::Options => None,
            SyncTarget::Servers => None,
            SyncTarget::Commands => None,
            SyncTarget::Hotbars => None,
            SyncTarget::Saves => Some("saves"),
            SyncTarget::Config => Some("config"),
            SyncTarget::Screenshots => Some("screenshots"),
            SyncTarget::Resourcepacks => Some("resourcepacks"),
            SyncTarget::Shaderpacks => Some("shaderpacks"),
            SyncTarget::Flashback => Some("flashback"),
            SyncTarget::DistantHorizons => Some("Distant_Horizons_server_data"),
            SyncTarget::Voxy => Some(".voxy"),
            SyncTarget::XaerosMinimap => Some("xaero"),
            SyncTarget::Bobby => Some(".bobby"),
            SyncTarget::Litematic => Some("schematics"),
        }
    }
}
