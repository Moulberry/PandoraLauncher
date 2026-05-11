use std::{path::Path, sync::Arc, time::Duration};

use indexmap::IndexMap;
use once_cell::sync::Lazy;
use schema::{auxiliary::AuxDisabledChildren, content::ContentSource, curseforge::{CurseforgeModpackFile, CurseforgeModpackMinecraft}, loader::Loader, server_status::ServerStatus, text_component::FlatTextComponent, unique_bytes::UniqueBytes};

use crate::{safe_path::SafePath};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InstanceID {
    pub index: usize,
    pub generation: usize,
}

impl InstanceID {
    pub fn dangling() -> Self {
        Self {
            index: usize::MAX,
            generation: usize::MAX,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InstanceContentID {
    pub index: usize,
    pub generation: usize,
}

impl InstanceContentID {
    pub fn dangling() -> Self {
        Self {
            index: usize::MAX,
            generation: usize::MAX,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstanceStatus {
    NotRunning,
    Launching,
    Running,
    Stopping,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InstancePlaytime {
    pub total_secs: u64,
    pub current_session_secs: u64,
    pub last_played_unix_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct InstanceWorldSummary {
    pub title: Arc<str>,
    pub subtitle: Arc<str>,
    pub level_path: Arc<Path>,
    pub last_played: i64,
    pub png_icon: Option<UniqueBytes>,
}

#[derive(Debug, Clone)]
pub struct InstanceServerSummary {
    pub name: Arc<str>,
    pub ip: Arc<str>,
    pub png_icon: Option<UniqueBytes>,
    pub pinging: bool,
    pub status: Option<Arc<ServerStatus>>,
    pub ping: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct InstanceContentSummary {
    pub content_summary: Arc<ContentSummary>,
    pub id: InstanceContentID,
    pub filename: Arc<str>,
    pub lowercase_search_keys: Arc<[Arc<str>]>,
    pub filename_hash: u64,
    pub modified_unix_ms: u64,
    pub path: Arc<Path>,
    pub can_toggle: bool,
    pub enabled: bool,
    pub content_source: ContentSource,
    pub update: ContentUpdateContext,
    pub disabled_children: Arc<AuxDisabledChildren>,
}

#[derive(Debug, Clone)]
pub struct ContentSummary {
    pub id: Option<Arc<str>>,
    pub hash: [u8; 20],
    pub filesize: Option<u64>,
    pub name: Option<Arc<str>>,
    pub version_str: Arc<str>,
    pub rich_description: Option<Arc<FlatTextComponent>>,
    pub authors: Arc<str>,
    pub png_icon: Option<UniqueBytes>,
    pub extra: ContentType,
}

#[derive(enum_map::Enum, Debug, strum::EnumIter, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContentFolder {
    Mods,
    ResourcePacks,
    Shaders,
}

impl ContentFolder {
    pub fn folder_name(self) -> &'static str {
        match self {
            ContentFolder::Mods => "mods",
            ContentFolder::ResourcePacks => "resourcepacks",
            ContentFolder::Shaders => "shaderpacks",
        }
    }
}

impl ContentSummary {
    pub fn is_unknown(summary: &Arc<Self>) -> bool {
        Arc::ptr_eq(summary, &*UNKNOWN_CONTENT_SUMMARY)
    }
}

pub static UNKNOWN_CONTENT_SUMMARY: Lazy<Arc<ContentSummary>> = Lazy::new(|| {
    Arc::new(ContentSummary {
        id: None,
        hash: [0_u8; 20],
        filesize: None,
        name: None,
        authors: "".into(),
        version_str: "unknown".into(),
        rich_description: None,
        png_icon: None,
        extra: ContentType::Unknown,
    })
});

#[derive(Debug, Clone)]
pub enum ModpackFilePath {
    Path(SafePath),
    Filename(SafePath),
}

impl ModpackFilePath {
    pub fn as_str(&self) -> &str {
        match self {
            ModpackFilePath::Path(safe_path) => safe_path.as_str(),
            ModpackFilePath::Filename(safe_path) => safe_path.as_str(),
        }
    }

    pub fn to_path(&self, summary: Option<&ContentSummary>) -> Option<SafePath> {
        match self {
            ModpackFilePath::Path(safe_path) => Some(safe_path.clone()),
            ModpackFilePath::Filename(filename) => {
                let folder = summary?.extra.content_folder()?;
                Some(SafePath::new(folder)?.join(&filename))
            },
        }
    }

    pub fn file_name(&self) -> Option<&str> {
        match self {
            ModpackFilePath::Path(safe_path) => safe_path.file_name(),
            ModpackFilePath::Filename(safe_path) => safe_path.file_name(),
        }
    }

    pub fn extension(&self) -> Option<&str> {
        match self {
            ModpackFilePath::Path(safe_path) => safe_path.extension(),
            ModpackFilePath::Filename(safe_path) => safe_path.extension(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModpackFile {
    pub source: ModpackFileSource,
    pub path: ModpackFilePath,
    pub hash: [u8; 20],
    pub summary: Option<Arc<ContentSummary>>,
    pub default_disabled: bool,
    pub disabled_third_party_downloads: bool,
}

impl ModpackFile {
    pub fn path(&self) -> Option<SafePath> {
        self.path.to_path(self.summary.as_deref())
    }
}

#[derive(Debug, Clone)]
pub enum ModpackFileSource {
    DownloadUrl {
        url: Arc<str>,
        size: usize,
    },
    DownloadCurseforge {
        file_id: u32,
    },
    Builtin {
        bytes: Arc<[u8]>,
    }
}

#[derive(Debug, Clone)]
pub enum ContentType {
    Unknown,
    Fabric,
    LegacyForge,
    Forge,
    NeoForge,
    JavaModule,
    ModrinthModpack {
        files: Arc<[ModpackFile]>,
        dependencies: IndexMap<Arc<str>, Arc<str>>,
    },
    CurseforgeModpack {
        unknown_files: Arc<[CurseforgeModpackFile]>,
        files: Arc<[ModpackFile]>,
        minecraft: CurseforgeModpackMinecraft,
    },
    ResourcePack,
    ShaderPack,
}

impl ContentType {
    pub fn modpack_files(&self) -> Option<&Arc<[ModpackFile]>> {
        match self {
            ContentType::ModrinthModpack { files, .. } => Some(files),
            ContentType::CurseforgeModpack { files, .. } => Some(files),
            _ => None,
        }
    }

    pub fn content_folder(&self) -> Option<&'static str> {
        match self {
            Self::Fabric | Self::Forge | Self::LegacyForge | Self::NeoForge | Self::JavaModule | Self::ModrinthModpack { .. } | Self::CurseforgeModpack { .. } => {
                Some("mods")
            },
            ContentType::ResourcePack => {
                Some("resourcepacks")
            },
            ContentType::ShaderPack => {
                Some("shaderpacks")
            },
            ContentType::Unknown => {
                None
            }
        }
    }

    pub fn is_strict_minecraft_version(&self) -> bool {
        match self {
            Self::ResourcePack => false,
            _ => true,
        }
    }

    pub fn is_strict_loader(&self) -> bool {
        match self {
            Self::Fabric => true,
            Self::LegacyForge => true,
            Self::Forge => true,
            Self::NeoForge => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentUpdateStatus {
    Unknown,
    ManualInstall,
    ErrorNotFound,
    ErrorInvalidHash,
    AlreadyUpToDate,
    Modrinth,
    Curseforge
}

impl ContentUpdateStatus {
    pub fn can_update(&self) -> bool {
        match self {
            ContentUpdateStatus::Modrinth => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ContentUpdateContext {
    status: ContentUpdateStatus,
    for_loader: Loader,
    for_version: &'static str,
}

impl ContentUpdateContext {
    pub fn new(status: ContentUpdateStatus, for_loader: Loader, for_version: &'static str) -> Self {
        Self { status, for_loader, for_version }
    }

    pub fn status_if_matches(&self, loader: Loader, version: &'static str) -> ContentUpdateStatus {
        if loader == self.for_loader && version == self.for_version {
            self.status
        } else {
            ContentUpdateStatus::Unknown
        }
    }

    pub fn can_update(&self, loader: Loader, version: &'static str) -> bool {
        self.for_loader == loader && self.for_version == version && self.status.can_update()
    }
}
