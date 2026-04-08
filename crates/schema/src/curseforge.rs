use std::sync::Arc;

use serde::{Deserialize, Serialize};
use strum::EnumIter;
use ustr::Ustr;

use crate::loader::Loader;

pub const CURSEFORGE_SEARCH_URL: &str = "https://api.curseforge.com/v1/mods/search";
pub const MINECRAFT_GAME_ID: u32 = 432;

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgeSearchRequest {
    pub class_id: u32,
    #[serde(skip_serializing_if = "crate::skip_if_none")]
    pub category_ids: Option<Arc<str>>,
    #[serde(skip_serializing_if = "crate::skip_if_none")]
    pub game_version: Option<Ustr>,
    #[serde(skip_serializing_if = "crate::skip_if_none")]
    pub search_filter: Option<Arc<str>>,
    #[serde(skip_serializing_if = "crate::skip_if_none")]
    pub mod_loader_types: Option<Arc<str>>,
    pub sort_field: u32,
    pub index: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgeGetModFilesRequest {
    pub mod_id: u32,
    #[serde(skip_serializing_if = "crate::skip_if_none")]
    pub game_version: Option<Ustr>,
    #[serde(skip_serializing_if = "crate::skip_if_none")]
    pub mod_loader_type: Option<u32>,
    #[serde(skip_serializing_if = "crate::skip_if_none")]
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgeGetFilesRequest {
    pub file_ids: Vec<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CurseforgeSearchResult {
    pub data: Arc<[CurseforgeHit]>,
    pub pagination: CurseforgePagination,
}

#[derive(Debug, Deserialize)]
pub struct CurseforgeGetModFilesResult {
    pub data: Arc<[CurseforgeFile]>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgePagination {
    pub index: u32,
    pub page_size: u32,
    pub result_count: u32,
    pub total_count: u64,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgeHit {
    pub id: u32,
    pub game_id: u32,
    pub name: Arc<str>,
    pub slug: Arc<str>,
    pub summary: Arc<str>,
    pub download_count: u64,
    pub class_id: Option<u32>,
    pub logo: Option<CurseforgeModAsset>,
    pub authors: Arc<[CurseforgeModAuthor]>,
    pub categories: Arc<[CurseforgeCategory]>,
    pub latest_files_indexes: Arc<[FileIndex]>
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileIndex {
    pub game_version: Ustr,
    pub file_id: u32,
    pub mod_loader: Option<u32>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgeModAsset {
    pub thumbnail_url: Arc<str>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgeModAuthor {
    pub id: u32,
    pub name: Arc<str>,
    pub url: Arc<str>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgeCategory {
    pub name: Arc<str>,
    pub is_class: bool,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgeFile {
    pub id: u32,
    pub mod_id: u32,
    pub file_name: Arc<str>,
    pub release_type: u32,
    pub file_length: u64,
    pub hashes: Arc<[CurseforgeFileHash]>,
    pub download_url: Option<Arc<str>>,
    pub dependencies: Arc<[CurseforgeFileDependency]>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgeFileDependency {
    pub mod_id: u32,
    pub relation_type: u32,
}

pub const CURSEFORGE_RELATION_TYPE_REQUIRED_DEPENDENCY: u32 = 3;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgeFileHash {
    pub value: Arc<str>,
    pub algo: u32,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CurseforgeReleaseType {
    Release = 1,
    Beta = 2,
    Alpha = 3,
    #[default]
    Other = 0,
}

impl CurseforgeReleaseType {
    pub fn from_u32(value: u32) -> Self {
        match value {
            1 => Self::Release,
            2 => Self::Beta,
            3 => Self::Alpha,
            _ => Self::Other,
        }
    }
}

#[derive(enumset::EnumSetType, Default, Debug, Hash, PartialOrd, Ord)]
#[repr(u32)]
pub enum CurseforgeModLoaderType {
    Forge = 1,
    Cauldron = 2,
    LiteLoader = 3,
    Fabric = 4,
    Quilt = 5,
    NeoForge = 6,
    #[default]
    Any = 0,
}

impl CurseforgeModLoaderType {
    pub fn from_u32(value: u32) -> Self {
        match value {
            1 => Self::Forge,
            2 => Self::Cauldron,
            3 => Self::LiteLoader,
            4 => Self::Fabric,
            5 => Self::Quilt,
            6 => Self::NeoForge,
            _ => Self::Any,
        }
    }
}

impl CurseforgeModLoaderType {
    pub fn pretty_name(self) -> &'static str {
        match self {
            Self::Forge => "Forge",
            Self::Cauldron => "Cauldron",
            Self::LiteLoader => "LiteLoader",
            Self::Fabric => "Fabric",
            Self::Quilt => "Quilt",
            Self::NeoForge => "NeoForge",
            Self::Any => "Any",
        }
    }

    pub fn from_name(str: &str) -> Self {
        match str {
            "Forge" | "forge" => Self::Forge,
            "Cauldron" | "cauldron" => Self::Cauldron,
            "LiteLoader" | "liteloader" => Self::LiteLoader,
            "Fabric" | "fabric" => Self::Fabric,
            "Quilt" | "quilt" => Self::Quilt,
            "NeoForge" | "neoforge" => Self::NeoForge,
            _ => Self::Any,
        }
    }
}

#[derive(Default, Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, EnumIter)]
#[repr(u32)]
pub enum CurseforgeSortField {
    #[default]
    Popularity = 2,
    Downloads = 6,
    LastUpdated = 3,
    Name = 4,
    Author = 5,
}

impl CurseforgeSortField {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Popularity => "popularity",
            Self::Downloads => "downloads",
            Self::LastUpdated => "updated",
            Self::Name => "name",
            Self::Author => "author",
        }
    }
}

#[derive(Default, Serialize, Deserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[serde(rename_all = "lowercase")]
#[repr(u32)]
pub enum CurseforgeClassId {
    BukkitPlugin = 5,
    Mod = 6,
    Resourcepack = 12,
    World = 17,
    Modpack = 4471,
    Customization = 4546,
    BedrockAddon = 4559,
    Shader = 6552,
    Datapack = 6945,
    #[default]
    #[serde(other)]
    Other = 0,
}

impl CurseforgeClassId {
    pub fn from_u32(value: u32) -> Self {
        match value {
            5 => Self::BukkitPlugin,
            6 => Self::Mod,
            12 => Self::Resourcepack,
            17 => Self::World,
            4471 => Self::Modpack,
            4546 => Self::Customization,
            4559 => Self::BedrockAddon,
            6552 => Self::Shader,
            6945 => Self::Datapack,
            _ => Self::Other,
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgeModpackManifestJson {
    pub minecraft: CurseforgeModpackMinecraft,
    pub version: Arc<str>,
    pub name: Option<Arc<str>>,
    pub files: Arc<[CurseforgeModpackFile]>,
    pub author: Option<Arc<str>>,
    pub overrides: Option<Arc<str>>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgeModpackMinecraft {
    pub version: Option<Arc<str>>,
    pub mod_loaders: Arc<[CurseforgeModpackModLoader]>,
    pub recommended_ram: Option<u32>,
}

impl CurseforgeModpackMinecraft {
    pub fn get_loader(&self) -> Option<Loader> {
        self.mod_loaders.iter()
            .find(|loader| loader.primary)
            .or_else(|| self.mod_loaders.first())
            .and_then(|loader| {
                if loader.id.starts_with("forge-") {
                    Some(Loader::Forge)
                } else if loader.id.starts_with("neoforge-") {
                    Some(Loader::NeoForge)
                } else if loader.id.starts_with("fabric-") {
                    Some(Loader::Fabric)
                } else {
                    None
                }
            })
    }
}

#[derive(Deserialize, Debug)]
pub struct CurseforgeModpackModLoader {
    pub id: Arc<str>,
    pub primary: bool,
}

#[derive(Deserialize, Debug)]
pub struct CurseforgeModpackFile {
    #[serde(rename = "projectID")]
    pub project_id: u32,
    #[serde(rename = "fileID")]
    pub file_id: u32,
    pub required: bool,
}

#[derive(Clone, Debug)]
pub struct CachedCurseforgeFileInfo {
    pub hash: [u8; 20],
    pub filename: Arc<str>,
    pub disabled_third_party_downloads: bool,
}
