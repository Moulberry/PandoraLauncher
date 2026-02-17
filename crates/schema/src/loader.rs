use enumset::EnumSetType;
use serde::{Deserialize, Serialize};

use crate::modrinth::ModrinthLoader;

#[derive(EnumSetType, Serialize, Deserialize, Debug, Hash, strum::EnumIter)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    #[serde(alias = "Vanilla")]
    Vanilla,
    #[serde(alias = "Fabric")]
    Fabric,
    #[serde(alias = "Forge")]
    Forge,
    #[serde(alias = "NeoForge")]
    NeoForge,
    #[serde(other)]
    Unknown,
}

impl Loader {
    pub fn name(self) -> &'static str {
        match self {
            Loader::Vanilla => "Vanilla",
            Loader::Fabric => "Fabric",
            Loader::Forge => "Forge",
            Loader::NeoForge => "NeoForge",
            Loader::Unknown => "Unknown",
        }
    }

    pub fn from_name(str: &str) -> Self {
        match str {
            "Vanilla" | "vanilla" => Self::Vanilla,
            "Fabric" | "fabric" => Self::Fabric,
            "Forge" | "forge" => Self::Forge,
            "NeoForge" | "neoforge" => Self::NeoForge,
            _ => Self::Unknown,
        }
    }

    pub fn as_modrinth_loader(self) -> ModrinthLoader {
        match self {
            Loader::Vanilla => ModrinthLoader::Unknown,
            Loader::Fabric => ModrinthLoader::Fabric,
            Loader::Forge => ModrinthLoader::Forge,
            Loader::NeoForge => ModrinthLoader::NeoForge,
            Loader::Unknown => ModrinthLoader::Unknown,
        }
    }
}
