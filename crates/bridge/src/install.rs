use std::{path::{Path, PathBuf}, sync::Arc};

use schema::{content::{ContentInstallReason, ContentSource}, loader::Loader};
use ustr::Ustr;

use crate::{instance::{InstanceID, ModpackFilePath}, safe_path::SafePath};

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
    pub loader: Loader,
    pub minecraft_version: Ustr,
    pub files: Arc<[ContentInstallFile]>,
}

#[derive(Debug, Clone)]
pub enum ContentInstallPath {
    Raw(Arc<Path>),
    Safe(SafePath),
    ModpackFilePath(ModpackFilePath),
    Automatic,
}

#[derive(Debug, Clone)]
pub struct ContentInstallFile {
    pub replace_old: Option<Arc<Path>>,
    pub path: ContentInstallPath,
    pub download: ContentDownload,
    pub content_source: ContentSource,
    pub reason: ContentInstallReason,
}

#[derive(Debug, Clone)]
pub enum ContentDownload {
    Modrinth {
        project_id: Arc<str>,
        version_id: Option<Arc<str>>,
        install_dependencies: bool,
    },
    Curseforge {
        project_id: u32,
        install_dependencies: bool,
    },
    Url {
        url: Arc<str>,
        sha1: [u8; 20],
        size: usize,
    },
    File {
        path: PathBuf,
    }
}
