use std::{path::{Path, PathBuf}, sync::Arc};

use schema::{content::ContentSource, loader::Loader};

use crate::{instance::InstanceID, safe_path::SafePath};

#[derive(Debug, Clone)]
pub enum InstallTarget {
    Instance(InstanceID),
    Library,
    NewInstance {
        name: Option<Arc<str>>,
    },
}

#[derive(Debug, Clone)]
pub struct ContentInstall {
    pub target: InstallTarget,
    pub loader_hint: Loader,
    pub version_hint: Option<Arc<str>>,
    pub files: Arc<[ContentInstallFile]>,
}

#[derive(Debug, Clone)]
pub enum ContentInstallPath {
    Raw(Arc<Path>),
    Safe(SafePath),
    Automatic,
}

#[derive(Debug, Clone)]
pub struct ContentInstallFile {
    pub replace_old: Option<Arc<Path>>,
    pub path: ContentInstallPath,
    pub download: ContentDownload,
    pub content_source: ContentSource,
}

#[derive(Debug, Clone)]
pub enum ContentDownload {
    Modrinth {
        project_id: Arc<str>,
        version_id: Option<Arc<str>>,
    },
    Url {
        url: Arc<str>,
        sha1: Arc<str>,
        size: usize,
    },
    File {
        path: PathBuf,
    }
}
