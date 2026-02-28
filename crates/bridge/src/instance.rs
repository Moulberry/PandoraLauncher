use std::{collections::{HashMap, HashSet}, path::Path, sync::Arc};

use indexmap::IndexMap;
use schema::{auxiliary::AuxDisabledChildren, content::ContentSource, loader::Loader, modification::ModrinthModpackFileDownload};
use ustr::Ustr;

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
    pub update: ContentUpdateContext,
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
        dependencies: IndexMap<Arc<str>, Arc<str>>,
    },
    ResourcePack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy)]
pub struct ContentUpdateContext {
    status: ContentUpdateStatus,
    for_loader: Loader,
    for_version: Ustr,
}

impl ContentUpdateContext {
    pub fn new(status: ContentUpdateStatus, for_loader: Loader, for_version: Ustr) -> Self {
        Self { status, for_loader, for_version }
    }

    pub fn status_if_matches(&self, loader: Loader, version: Ustr) -> ContentUpdateStatus {
        if loader == self.for_loader && version == self.for_version {
            self.status
        } else {
            ContentUpdateStatus::Unknown
        }
    }

    pub fn can_update(&self, loader: Loader, version: Ustr) -> bool {
        self.for_loader == loader && self.for_version == version && self.status.can_update()
    }
}
