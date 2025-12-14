use std::{ffi::OsString, path::{Path, PathBuf}, sync::Arc};

use schema::loader::Loader;
use ustr::Ustr;
use uuid::Uuid;

use crate::{
    account::Account, game_output::GameOutputLogLevel, install::ContentInstall, instance::{
        InstanceID, InstanceModID, InstanceModSummary, InstanceServerSummary, InstanceStatus, InstanceWorldSummary,
    }, keep_alive::{KeepAlive, KeepAliveHandle}, meta::{MetadataRequest, MetadataResult}, modal_action::ModalAction, serial::Serial
};

#[derive(Debug)]
pub enum MessageToBackend {
    RequestMetadata {
        request: MetadataRequest,
        force_reload: bool,
    },
    CreateInstance {
        name: Ustr,
        version: Ustr,
        loader: Loader,
    },
    DeleteInstance {
        id: InstanceID,
    },
    RenameInstance {
        id: InstanceID,
        name: Ustr,
    },
    KillInstance {
        id: InstanceID,
    },
    StartInstance {
        id: InstanceID,
        quick_play: Option<QuickPlayLaunch>,
        modal_action: ModalAction,
    },
    RequestLoadWorlds {
        id: InstanceID,
    },
    RequestLoadServers {
        id: InstanceID,
    },
    RequestLoadMods {
        id: InstanceID,
    },
    SetModEnabled {
        id: InstanceID,
        mod_id: InstanceModID,
        enabled: bool,
    },
    SetModChildEnabled {
        id: InstanceID,
        mod_id: InstanceModID,
        path: Arc<str>,
        enabled: bool,
    },
    DeleteMod {
        id: InstanceID,
        mod_id: InstanceModID,
    },
    UpdateAccountHeadPng {
        uuid: Uuid,
        head_png: Arc<[u8]>,
        head_png_32x: Arc<[u8]>,
    },
    InstallContent {
        content: ContentInstall,
        modal_action: ModalAction,
    },
    DownloadAllMetadata,
    UpdateCheck { instance: InstanceID, modal_action: ModalAction },
    UpdateMod {
        instance: InstanceID,
        mod_id: InstanceModID,
        modal_action: ModalAction,
    },
    Sleep5s,
    ReadLog {
        path: Arc<Path>,
        send: tokio::sync::mpsc::Sender<Arc<str>>
    },
    GetLogFiles {
        instance: InstanceID,
        channel: tokio::sync::oneshot::Sender<LogFiles>,
    },
    CleanupOldLogFiles {
        instance: InstanceID,
    },
    UploadLogFile {
        path: Arc<Path>,
        modal_action: ModalAction,
    }
}

#[derive(Debug)]
pub enum MessageToFrontend {
    InstanceAdded {
        id: InstanceID,
        name: Ustr,
        version: Ustr,
        loader: Loader,
        worlds_state: Arc<AtomicBridgeDataLoadState>,
        servers_state: Arc<AtomicBridgeDataLoadState>,
        mods_state: Arc<AtomicBridgeDataLoadState>,
    },
    InstanceRemoved {
        id: InstanceID,
    },
    InstanceModified {
        id: InstanceID,
        name: Ustr,
        version: Ustr,
        loader: Loader,
        status: InstanceStatus,
    },
    InstanceWorldsUpdated {
        id: InstanceID,
        worlds: Arc<[InstanceWorldSummary]>,
    },
    InstanceServersUpdated {
        id: InstanceID,
        servers: Arc<[InstanceServerSummary]>,
    },
    InstanceModsUpdated {
        id: InstanceID,
        mods: Arc<[InstanceModSummary]>,
    },
    CreateGameOutputWindow {
        id: usize,
        keep_alive: KeepAlive,
    },
    AddGameOutput {
        id: usize,
        time: i64,
        thread: Arc<str>,
        level: GameOutputLogLevel,
        text: Arc<[Arc<str>]>,
    },
    AddNotification {
        notification_type: BridgeNotificationType,
        message: Arc<str>,
    },
    AccountsUpdated {
        accounts: Arc<[Account]>,
        selected_account: Option<Uuid>,
    },
    Refresh,
    CloseModal,
    MoveInstanceToTop {
        id: InstanceID,
    },
    MetadataResult {
        request: MetadataRequest,
        result: Result<MetadataResult, Arc<str>>,
        keep_alive_handle: Option<KeepAliveHandle>,
    },
}

#[derive(Debug)]
pub struct LogFiles {
    pub paths: Vec<Arc<Path>>,
    pub total_gzipped_size: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BridgeNotificationType {
    Success,
    Info,
    Error,
    Warning,
}

#[atomic_enum::atomic_enum]
#[derive(PartialEq, Eq)]
pub enum BridgeDataLoadState {
    Unloaded,
    LoadingDirty,
    LoadedDirty,
    Loading,
    Loaded,
}

impl BridgeDataLoadState {
    pub fn should_send_load_request(self) -> bool {
        match self {
            BridgeDataLoadState::Unloaded => true,
            BridgeDataLoadState::LoadingDirty => false,
            BridgeDataLoadState::LoadedDirty => true,
            BridgeDataLoadState::Loading => false,
            BridgeDataLoadState::Loaded => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickPlayLaunch {
    Singleplayer(OsString),
    Multiplayer(OsString),
    Realms(OsString),
}
