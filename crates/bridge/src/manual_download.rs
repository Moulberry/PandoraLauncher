use std::{path::PathBuf, sync::Arc};

use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ManualCurseforgeDownload {
    pub project_id: u32,
    pub file_id: u32,
    pub name: Arc<str>,
    pub filename: Arc<str>,
    pub sha1: [u8; 20],
    pub size: u64,
    pub page_url: Arc<str>,
}

#[derive(Debug)]
pub struct ManualCurseforgeDownloadRequest {
    pub session_id: Uuid,
    pub files: Arc<[ManualCurseforgeDownload]>,
    pub progress: tokio::sync::mpsc::UnboundedReceiver<[u8; 20]>,
    pub completion: tokio::sync::oneshot::Receiver<()>,
}

#[derive(Clone, Debug)]
pub struct ManualCurseforgeDownloadStart {
    pub session_id: Uuid,
    pub directory: PathBuf,
}
