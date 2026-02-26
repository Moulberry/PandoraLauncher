use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ustr::Ustr;

pub const MODRINTH_SEARCH_URL: &str = "https://api.modrinth.com/v2/search";

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ModrinthSearchRequest {
    pub query: Option<Arc<str>>,
    pub facets: Option<Arc<str>>,
    pub index: ModrinthSearchIndex,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ModrinthProjectVersionsRequest {
    pub project_id: Arc<str>,
    pub game_versions: Option<Arc<[Arc<str>]>>,
    pub loaders: Option<Arc<[ModrinthLoader]>>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ModrinthSearchIndex {
    Relevance,
    Downloads,
    Follows,
    Newest,
    Updated,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthSearchResult {
    pub hits: Arc<[ModrinthHit]>,
    pub offset: usize,
    pub limit: usize,
    pub total_hits: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthHit {
    // pub slug: Option<Arc<str>>,
    pub title: Option<Arc<str>>,
    pub description: Option<Arc<str>>,
    // pub categories: Option<Arc<[Arc<str>]>>,
    pub client_side: Option<ModrinthSideRequirement>,
    pub server_side: Option<ModrinthSideRequirement>,
    pub project_type: ModrinthProjectType,
    pub downloads: usize,
    pub icon_url: Option<Arc<str>>,
    // pub color: Option<u32>,
    // pub thread_id: Option<Arc<str>>,
    // pub monetization_status: Option<ModrinthMonetizationStatus>,
    pub project_id: Arc<str>,
    pub author: Arc<str>,
    pub display_categories: Option<Arc<[Ustr]>>,
    // pub versions: Arc<[Arc<str>]>,
    // pub follows: usize,
    // pub date_created: DateTime<Utc>,
    // pub date_modified: DateTime<Utc>,
    // pub latest_version: Option<Arc<str>>,
    // pub license: Arc<str>,
    // pub gallery: Option<Arc<[Arc<str>]>>,
    // pub featured_gallery: Option<Arc<str>>,
}

#[derive(PartialEq, Eq, Debug, Copy, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModrinthSideRequirement {
    Required,
    Optional,
    Unsupported,
    #[serde(other)]
    Unknown,
}

#[derive(Default, Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModrinthProjectType {
    Mod,
    Modpack,
    Resourcepack,
    Shader,
    #[serde(other)]
    #[default]
    Other,
}

impl ModrinthProjectType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ModrinthProjectType::Mod => "mod",
            ModrinthProjectType::Modpack => "modpack",
            ModrinthProjectType::Resourcepack => "resourcepack",
            ModrinthProjectType::Shader => "shader",
            ModrinthProjectType::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthProjectVersionsResult(pub Arc<[ModrinthProjectVersion]>);

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthProjectVersion {
    pub game_versions: Option<Arc<[Ustr]>>,
    pub loaders: Option<Arc<[ModrinthLoader]>>,
    pub id: Arc<str>,
    pub project_id: Arc<str>,
    pub name: Option<Arc<str>>,
    pub version_number: Option<Arc<str>>,
    pub dependencies: Option<Vec<ModrinthDependency>>,
    pub version_type: Option<ModrinthVersionType>,
    pub status: Option<ModrinthVersionStatus>,
    pub files: Arc<[ModrinthFile]>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthDependency {
    pub version_id: Option<Arc<str>>,
    pub project_id: Option<Arc<str>>,
    pub file_name: Option<Arc<str>>,
    pub dependency_type: ModrinthDependencyType,
}

#[derive(PartialEq, Eq, Debug, Copy, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ModrinthDependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

#[derive(enumset::EnumSetType, Debug, Deserialize, Serialize, Hash, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ModrinthLoader {
    // Mods
    Fabric,
    Forge,
    NeoForge,
    // Resourcepacks
    Minecraft,
    // Shaders
    Iris,
    Optifine,
    Canvas,
    // Other
    #[serde(other)]
    Unknown,
}

impl ModrinthLoader {
    pub fn install_directory(self) -> Option<&'static str> {
        match self {
            ModrinthLoader::Fabric | ModrinthLoader::Forge | ModrinthLoader::NeoForge => Some("mods"),
            ModrinthLoader::Minecraft => Some("resourcepacks"),
            ModrinthLoader::Iris | ModrinthLoader::Optifine => Some("shaderpacks"),
            ModrinthLoader::Canvas => Some("resourcepacks"),
            ModrinthLoader::Unknown => None,
        }
    }

    pub fn pretty_name(self) -> &'static str {
        match self {
            Self::Fabric => "Fabric",
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
            Self::Minecraft => "Minecraft",
            Self::Iris => "Iris",
            Self::Optifine => "Optifine",
            Self::Canvas => "Canvas",
            Self::Unknown => "Unknown",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Fabric => "fabric",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
            Self::Minecraft => "minecraft",
            Self::Iris => "iris",
            Self::Optifine => "optifine",
            Self::Canvas => "canvas",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_name(str: &str) -> Self {
        match str {
            "Fabric" | "fabric" => Self::Fabric,
            "Forge" | "forge" => Self::Forge,
            "NeoForge" | "neoforge" => Self::NeoForge,
            "Minecraft" | "minecraft" => Self::Minecraft,
            "Iris" | "iris" => Self::Iris,
            "Optifine" | "optifine" => Self::Optifine,
            "Canvas" | "canvas" => Self::Canvas,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Copy, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModrinthVersionType {
    Release,
    Beta,
    Alpha,
    #[serde(other)]
    Other,
}

#[derive(Debug, Copy, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModrinthVersionStatus {
    Listed,
    Archived,
    Draft,
    Unlisted,
    Scheduled,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthFile {
    pub hashes: ModrinthHashes,
    pub url: Arc<str>,
    pub filename: Arc<str>,
    pub primary: bool,
    pub size: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthHashes {
    pub sha1: Arc<str>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthVersionFileUpdateResult(pub ModrinthProjectVersion);

pub const MODRINTH_PROJECT_URL: &str = "https://api.modrinth.com/v2/project";

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ModrinthProjectRequest {
    pub project_id: Arc<str>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthProjectResult {
    pub id: Arc<str>,
    pub slug: Option<Arc<str>>,
    pub title: Option<Arc<str>>,
    pub description: Option<Arc<str>>,
    pub body: Option<Arc<str>>,
    pub project_type: ModrinthProjectType,
    pub downloads: usize,
    pub followers: usize,
    pub icon_url: Option<Arc<str>>,
    pub client_side: Option<ModrinthSideRequirement>,
    pub server_side: Option<ModrinthSideRequirement>,
    pub categories: Option<Arc<[ustr::Ustr]>>,
    pub additional_categories: Option<Arc<[ustr::Ustr]>>,
    pub game_versions: Option<Arc<[Arc<str>]>>,
    pub loaders: Option<Arc<[ModrinthLoader]>>,
    pub license: Option<ModrinthLicense>,
    pub source_url: Option<Arc<str>>,
    pub issues_url: Option<Arc<str>>,
    pub wiki_url: Option<Arc<str>>,
    pub discord_url: Option<Arc<str>>,
    pub gallery: Option<Arc<[ModrinthGalleryImage]>>,
    pub published: Option<Arc<str>>,
    pub updated: Option<Arc<str>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthLicense {
    pub id: Arc<str>,
    pub name: Arc<str>,
    pub url: Option<Arc<str>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthGalleryImage {
    pub url: Arc<str>,
    pub featured: bool,
    pub title: Option<Arc<str>>,
    pub description: Option<Arc<str>>,
}