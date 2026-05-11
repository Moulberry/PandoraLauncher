use enumset::EnumSetType;
use serde::{Deserialize, Serialize};

use crate::{curseforge::CurseforgeModLoaderType, modrinth::ModrinthLoader};

#[derive(EnumSetType, Serialize, Deserialize, Debug, Hash, strum::EnumIter)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    #[serde(alias = "Fabric")]
    Fabric,
    #[serde(alias = "Forge")]
    Forge,
    #[serde(alias = "NeoForge")]
    NeoForge,
    #[serde(other)]
    #[serde(alias = "Vanilla")]
    Vanilla,
}

impl Loader {
    pub fn pretty_name(self) -> &'static str {
        match self {
            Loader::Vanilla => "Vanilla",
            Loader::Fabric => "Fabric",
            Loader::Forge => "Forge",
            Loader::NeoForge => "NeoForge",
        }
    }

    pub fn from_name(str: &str) -> Option<Self> {
        match str {
            "Vanilla" | "vanilla" => Some(Self::Vanilla),
            "Fabric" | "fabric" => Some(Self::Fabric),
            "Forge" | "forge" => Some(Self::Forge),
            "NeoForge" | "neoforge" => Some(Self::NeoForge),
            _ => None,
        }
    }

    pub fn as_modrinth_loader(self) -> ModrinthLoader {
        match self {
            Loader::Vanilla => ModrinthLoader::Unknown,
            Loader::Fabric => ModrinthLoader::Fabric,
            Loader::Forge => ModrinthLoader::Forge,
            Loader::NeoForge => ModrinthLoader::NeoForge,
        }
    }

    pub fn as_curseforge_loader(&self) -> CurseforgeModLoaderType {
        match self {
            Loader::Vanilla => CurseforgeModLoaderType::Any,
            Loader::Fabric => CurseforgeModLoaderType::Fabric,
            Loader::Forge => CurseforgeModLoaderType::Forge,
            Loader::NeoForge => CurseforgeModLoaderType::NeoForge,
        }
    }
}
