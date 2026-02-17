use std::{collections::HashMap, path::PathBuf, sync::Arc};

use serde::Deserialize;

#[derive(Debug, Clone)]
pub enum UpdateInstallType {
    AppImage(PathBuf),
    Executable,
    App(PathBuf),
}

impl UpdateInstallType {
    pub fn key(&self) -> &'static str {
        match self {
            UpdateInstallType::AppImage(..) => "appimage",
            UpdateInstallType::Executable => "executable",
            UpdateInstallType::App(..) => "app",
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpdatePrompt {
    pub old_version: Arc<str>,
    pub new_version: Arc<str>,
    pub install_type: UpdateInstallType,
    pub exe: UpdateManifestExe,
}

#[derive(Deserialize, Debug, Clone)]
pub struct UpdateManifest {
    pub version: Arc<str>,
    pub downloads: UpdateManifestArchs
}

#[derive(Deserialize, Debug, Clone)]
pub struct UpdateManifestArchs {
    #[serde(flatten)]
    pub archs: HashMap<Arc<str>, UpdateManifestExes>
}

#[derive(Deserialize, Debug, Clone)]
pub struct UpdateManifestExes {
    #[serde(flatten)]
    pub exes: HashMap<Arc<str>, UpdateManifestExe>
}

#[derive(Deserialize, Debug, Clone)]
pub struct UpdateManifestExe {
    pub download: Arc<str>,
    pub size: usize,
    pub sha1: Arc<str>,
    pub sig: Arc<str>,
}
