use std::{path::Path, sync::Arc, time::Duration};

use indexmap::IndexMap;
use once_cell::sync::Lazy;
use schema::{auxiliary::AuxDisabledChildren, content::ContentSource, curseforge::{CachedCurseforgeFileInfo, CurseforgeModpackFile, CurseforgeModpackMinecraft}, loader::Loader, modification::ModrinthModpackFileDownload, server_status::ServerStatus, text_component::FlatTextComponent, unique_bytes::UniqueBytes};

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
    pub name: Option<Arc<str>>,
    pub version_str: Arc<str>,
    pub rich_description: Option<Arc<FlatTextComponent>>,
    pub authors: Arc<str>,
    pub png_icon: Option<UniqueBytes>,
    pub extra: ContentType,
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
        name: None,
        authors: "".into(),
        version_str: "unknown".into(),
        rich_description: None,
        png_icon: None,
        extra: ContentType::Unknown,
    })
});

#[derive(Debug, Clone)]
pub enum ContentType {
    Unknown,
    Fabric,
    LegacyForge,
    Forge,
    NeoForge,
    JavaModule,
    ModrinthModpack {
        downloads: Arc<[ModrinthModpackFileDownload]>,
        summaries: Arc<[Option<Arc<ContentSummary>>]>,
        overrides: Arc<[(SafePath, Arc<[u8]>)]>,
        dependencies: IndexMap<Arc<str>, Arc<str>>,
    },
    CurseforgeModpack {
        files: Arc<[CurseforgeModpackFile]>,
        summaries: Arc<[(Option<Arc<ContentSummary>>, Option<CachedCurseforgeFileInfo>)]>,
        overrides: Arc<[(SafePath, Arc<[u8]>)]>,
        minecraft: CurseforgeModpackMinecraft,
    },
    ResourcePack,
}

impl ContentType {
    pub fn content_folder(&self) -> Option<&'static str> {
        match self {
            Self::Fabric | Self::Forge | Self::LegacyForge | Self::NeoForge | Self::JavaModule | Self::ModrinthModpack { .. } | Self::CurseforgeModpack { .. } => {
                Some("mods")
            },
            ContentType::ResourcePack => {
                Some("resourcepacks")
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
