use std::{collections::HashSet, path::Path, sync::Arc};

use schema::{auxiliary::AuxDisabledChildren, content::ContentSource, modification::ModrinthModpackFileDownload};

use crate::safe_path::SafePath;

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
}

#[derive(Debug, Clone)]
pub struct InstanceWorldSummary {
    pub title: Arc<str>,
    pub subtitle: Arc<str>,
    pub level_path: Arc<Path>,
    pub last_played: i64,
    pub png_icon: Option<Arc<[u8]>>,
}

#[derive(Debug, Clone)]
pub struct InstanceServerSummary {
    pub name: Arc<str>,
    pub ip: Arc<str>,
    pub png_icon: Option<Arc<[u8]>>,
}

#[derive(Debug, Clone)]
pub struct InstanceContentSummary {
    pub content_summary: Arc<ContentSummary>,
    pub id: InstanceContentID,
    pub filename: Arc<str>,
    pub lowercase_search_keys: Arc<[Arc<str>]>,
    pub filename_hash: u64,
    pub path: Arc<Path>,
    pub enabled: bool,
    pub content_source: ContentSource,
    pub disabled_children: Arc<AuxDisabledChildren>,
}

#[derive(Debug, Clone)]
pub struct ContentSummary {
    pub id: Option<Arc<str>>,
    pub hash: [u8; 20],
    pub name: Option<Arc<str>>,
    pub version_str: Arc<str>,
    pub authors: Arc<str>,
    pub png_icon: Option<Arc<[u8]>>,
    pub update_status: Arc<AtomicContentUpdateStatus>,
    pub extra: ContentType,
}

#[derive(Debug, Clone)]
pub enum ContentType {
    Fabric,
    LegacyForge,
    Forge,
    NeoForge,
    JavaModule,
    ModrinthModpack {
        downloads: Arc<[ModrinthModpackFileDownload]>,
        summaries: Arc<[Option<Arc<ContentSummary>>]>,
        overrides: Arc<[(SafePath, Arc<[u8]>)]>,
    },
    ResourcePack,
}


#[atomic_enum::atomic_enum]
#[derive(PartialEq, Eq)]
pub enum ContentUpdateStatus {
    Unknown,
    ManualInstall,
    ErrorNotFound,
    ErrorInvalidHash,
    AlreadyUpToDate,
    Modrinth,
}

impl ContentUpdateStatus {
    pub fn can_update(&self) -> bool {
        match self {
            ContentUpdateStatus::Modrinth => true,
            _ => false,
        }
    }
}
