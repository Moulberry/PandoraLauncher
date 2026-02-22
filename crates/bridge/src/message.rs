use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Arc,
};

use enumset::{EnumSet, EnumSetType};
use schema::{
    backend_config::{BackendConfig, SyncTarget}, instance::{
        InstanceConfiguration, InstanceJvmBinaryConfiguration, InstanceJvmFlagsConfiguration,
        InstanceLinuxWrapperConfiguration, InstanceMemoryConfiguration, InstanceSyncConfiguration, InstanceSystemLibrariesConfiguration,
    }, loader::Loader, pandora_update::{UpdateManifest, UpdateManifestExe, UpdatePrompt}, syncing::SyncEntry,
};
use ustr::Ustr;
use uuid::Uuid;

use crate::{
    account::Account,
    game_output::GameOutputLogLevel,
    install::ContentInstall,
    instance::{
        InstanceContentID, InstanceContentSummary, InstanceID, InstanceServerSummary, InstanceStatus,
        InstanceWorldSummary,
    },
    keep_alive::{KeepAlive, KeepAliveHandle},
    meta::{MetadataRequest, MetadataResult},
    modal_action::ModalAction,
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
        icon: Option<EmbeddedOrRaw>,
    },
    DeleteInstance {
        id: InstanceID,
    },
    RenameInstance {
        id: InstanceID,
        name: Ustr,
    },
    SetInstanceMinecraftVersion {
        id: InstanceID,
        version: Ustr
    },
    SetInstanceLoader {
        id: InstanceID,
        loader: Loader
    },
    SetInstancePreferredLoaderVersion {
        id: InstanceID,
        loader_version: Option<&'static str>
    },
    SetInstanceMemory {
        id: InstanceID,
        memory: InstanceMemoryConfiguration,
    },
    SetInstanceJvmFlags {
        id: InstanceID,
        jvm_flags: InstanceJvmFlagsConfiguration,
    },
    SetInstanceJvmBinary {
        id: InstanceID,
        jvm_binary: InstanceJvmBinaryConfiguration,
    },
    SetInstanceSync {
        id: InstanceID,
        sync: InstanceSyncConfiguration,
    },
    SetInstanceLinuxWrapper {
        id: InstanceID,
        linux_wrapper: InstanceLinuxWrapperConfiguration,
    },
    SetInstanceSystemLibraries {
        id: InstanceID,
        system_libraries: InstanceSystemLibrariesConfiguration,
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
    RequestLoadResourcePacks {
        id: InstanceID,
    },
    SetContentEnabled {
        id: InstanceID,
        content_ids: Vec<InstanceContentID>,
        enabled: bool,
    },
    SetContentChildEnabled {
        id: InstanceID,
        content_id: InstanceContentID,
        child_id: Option<Arc<str>>,
        child_name: Option<Arc<str>>,
        child_filename: Arc<str>,
        enabled: bool,
    },
    DeleteContent {
        id: InstanceID,
        content_ids: Vec<InstanceContentID>,
    },
    InstallContent {
        content: ContentInstall,
        modal_action: ModalAction,
    },
    DownloadAllMetadata,
    UpdateCheck {
        instance: InstanceID,
        modal_action: ModalAction
    },
    UpdateContent {
        instance: InstanceID,
        content_id: InstanceContentID,
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
    GetSyncState {
        channel: tokio::sync::oneshot::Sender<SyncState>,
    },
    GetBackendConfiguration {
        channel: tokio::sync::oneshot::Sender<BackendConfig>,
    },
    SetSyncing {
        target: SyncTarget,
        value: bool,
    },
    AddSync {
        entry: SyncEntry,
        id_channel: tokio::sync::oneshot::Sender<u64>,
    },
    ModifySync {
        id: u64,
        new_entry: SyncEntry,
    },
    DeleteSync {
        id: u64,
    },
    CleanupOldLogFiles {
        instance: InstanceID,
    },
    UploadLogFile {
        path: Arc<Path>,
        modal_action: ModalAction,
    },
    AddNewAccount {
        modal_action: ModalAction,
    },
    AddOfflineAccount {
        name: Arc<str>,
        uuid: Uuid
    },
    SelectAccount {
        uuid: Uuid,
    },
    DeleteAccount {
        uuid: Uuid,
    },
    SetOpenGameOutputAfterLaunching {
        value: bool,
    },
    CreateInstanceShortcut {
        id: InstanceID,
        path: PathBuf
    },
    InstallUpdate {
        update: UpdatePrompt,
        modal_action: ModalAction,
    }
}

#[derive(Debug)]
pub enum MessageToFrontend {
    InstanceAdded {
        id: InstanceID,
        name: Ustr,
        icon: Option<Arc<[u8]>>,
        dot_minecraft_folder: Arc<Path>,
        configuration: InstanceConfiguration,
        worlds_state: Arc<AtomicBridgeDataLoadState>,
        servers_state: Arc<AtomicBridgeDataLoadState>,
        mods_state: Arc<AtomicBridgeDataLoadState>,
        resource_packs_state: Arc<AtomicBridgeDataLoadState>,
    },
    InstanceRemoved {
        id: InstanceID,
    },
    InstanceModified {
        id: InstanceID,
        name: Ustr,
        icon: Option<Arc<[u8]>>,
        dot_minecraft_folder: Arc<Path>,
        configuration: InstanceConfiguration,
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
        mods: Arc<[InstanceContentSummary]>,
    },
    InstanceResourcePacksUpdated {
        id: InstanceID,
        resource_packs: Arc<[InstanceContentSummary]>,
    },
    CreateGameOutputWindow {
        id: usize,
        keep_alive: KeepAlive,
    },
    AddGameOutput {
        id: usize,
        time: i64,
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
    UpdateAvailable {
        update: UpdatePrompt,
    },
}

#[derive(Debug, Default)]
pub struct LogFiles {
    pub paths: Vec<Arc<Path>>,
    pub total_gzipped_size: usize,
}

#[derive(Debug, Default)]
pub struct SyncState {
    pub sync_folder: Option<Arc<Path>>,
    pub want_sync: EnumSet<SyncTarget>,
    pub total: usize,
    pub synced: enum_map::EnumMap<SyncTarget, usize>,
    pub cannot_sync: enum_map::EnumMap<SyncTarget, usize>,
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

#[derive(Debug, Clone)]
pub enum EmbeddedOrRaw {
    Embedded(Arc<str>),
    Raw(Arc<[u8]>),
}
