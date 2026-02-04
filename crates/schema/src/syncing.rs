use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncLink {
    pub source: Box<Path>,
    pub target: Box<Path>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SyncType {
    Symlink(SymlinkSync),
    CopySave(CopySaveSync),
    CopyDelete(CopyDeleteSync),
    Children(ChildrenSync),
    CustomScript(CustomScriptSync),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncEntry {
    enabled: bool,
    sync: SyncType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SymlinkSync {
    pub link: SyncLink
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CopySaveSync {
    pub link: SyncLink
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CopyDeleteSync {
    pub link: SyncLink
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChildrenSync {
    pub link: SyncLink,
    pub keep_name: bool
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CustomScriptSync {
    pub link: SyncLink
}
