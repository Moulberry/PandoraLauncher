use std::{
    collections::BTreeMap, ffi::OsString, path::{Path, PathBuf}, sync::{Arc, atomic::AtomicU8}
};

use schema::{
    backend_config::{BackendConfig, ProxyConfig}, instance::{
        InstanceConfiguration, InstanceJvmBinaryConfiguration, InstanceJvmFlagsConfiguration,
        InstanceLinuxWrapperConfiguration, InstanceMemoryConfiguration, InstanceSystemLibrariesConfiguration, InstanceWrapperCommandConfiguration,
    }, loader::Loader, minecraft_profile::{MinecraftProfileCape, SkinVariant}, pandora_update::UpdatePrompt, unique_bytes::UniqueBytes
};
use ustr::Ustr;
use uuid::Uuid;

use crate::{
    account::Account, game_output::GameOutputLogLevel, import::{ImportFromOtherLauncherJob, OtherLauncher}, install::ContentInstall, instance::{
        ContentFolder, InstanceContentID, InstanceContentSummary, InstanceID, InstancePlaytime, InstanceServerSummary, InstanceStatus, InstanceWorldSummary
    }, keep_alive::KeepAliveHandle, meta::{MetadataRequest, MetadataResult}, modal_action::ModalAction,
};

#[derive(Debug)]
#[derive(Default)]
pub struct BackendConfigWithPassword {
    pub config: BackendConfig,
    pub proxy_password: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Zip,
    Modrinth,
    Curseforge,
}

#[derive(Debug, Clone)]
pub struct ExportModrinthOptions {
    pub name: Arc<str>,
    pub version: Arc<str>,
    pub summary: Option<Arc<str>>,
}

#[derive(Debug, Clone)]
pub struct ExportCurseforgeOptions {
    pub name: Arc<str>,
    pub version: Arc<str>,
    pub author: Option<Arc<str>>,
    pub recommended_ram: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub include_saves: bool,
    pub include_mods: bool,
    pub include_resourcepacks: bool,
    pub include_configs: bool,
    pub include_logs: bool,
    pub include_cache: bool,
    pub include_synced: bool,
    pub modrinth: ExportModrinthOptions,
    pub curseforge: ExportCurseforgeOptions,
}

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
    ExportInstance {
        id: InstanceID,
        format: ExportFormat,
        options: ExportOptions,
        output: PathBuf,
        modal_action: ModalAction,
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
    SetInstancePreferredAccount {
    	id: InstanceID,
     	account: Option<Uuid>,
    },
    SetInstancePreferredLoaderVersion {
        id: InstanceID,
        loader_version: Option<&'static str>
    },
    SetInstanceDisableFileSyncing {
        id: InstanceID,
        disable_file_syncing: bool,
    },
    SetInstanceSandboxing {
        id: InstanceID,
        sandbox: bool,
    },
    SetInstanceMemory {
        id: InstanceID,
        memory: InstanceMemoryConfiguration,
    },
    SetInstanceWrapperCommand {
        id: InstanceID,
        wrapper_command: InstanceWrapperCommandConfiguration,
    },
    SetInstanceJvmFlags {
        id: InstanceID,
        jvm_flags: InstanceJvmFlagsConfiguration,
    },
    SetInstanceJvmBinary {
        id: InstanceID,
        jvm_binary: InstanceJvmBinaryConfiguration,
    },
    SetInstanceLinuxWrapper {
        id: InstanceID,
        linux_wrapper: InstanceLinuxWrapperConfiguration,
    },
    SetInstanceSystemLibraries {
        id: InstanceID,
        system_libraries: InstanceSystemLibrariesConfiguration,
    },
    SetInstanceIcon {
        id: InstanceID,
        icon: Option<EmbeddedOrRaw>,
    },
    KillInstance {
        id: InstanceID,
    },
    StartInstanceByName {
        name: String,
        quick_play: Option<QuickPlayLaunch>
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
    ReorderServers {
        id: InstanceID,
        from_index: usize,
        to_index: usize,
    },
    RequestLoadContentFolder {
        id: InstanceID,
        content_folder: ContentFolder,
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
        disabled_default: bool,
        enabled: bool,
    },
    DownloadContentChildren {
        id: InstanceID,
        content_id: InstanceContentID,
        modal_action: ModalAction,
    },
    DeleteContent {
        id: InstanceID,
        content_ids: Vec<InstanceContentID>,
    },
    InstallContent {
        content: ContentInstall,
        modal_action: ModalAction,
    },
    CreateInstanceFromFile {
        file: PathBuf,
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
    GetImportFromOtherLauncherJob {
        channel: tokio::sync::oneshot::Sender<Option<ImportFromOtherLauncherJob>>,
        launcher: OtherLauncher,
        path: Arc<Path>,
    },
    GetSyncState {
        channel: tokio::sync::oneshot::Sender<SyncState>,
    },
    GetBackendConfiguration {
        channel: tokio::sync::oneshot::Sender<BackendConfigWithPassword>,
    },
    SetSyncing {
        target: Arc<str>,
        is_file: bool,
        value: bool,
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
    SetProxyConfiguration {
        config: ProxyConfig,
        password: Option<String>,
    },
    CreateInstanceShortcut {
        id: InstanceID,
        path: PathBuf
    },
    RelocateInstance {
        id: InstanceID,
        path: PathBuf
    },
    InstallUpdate {
        update: UpdatePrompt,
        modal_action: ModalAction,
    },
    ImportFromOtherLauncher {
        launcher: OtherLauncher,
        import_job: ImportFromOtherLauncherJob,
        modal_action: ModalAction,
    },
    GetAccountSkin {
        account: Uuid,
        result: tokio::sync::oneshot::Sender<AccountSkinResult>
    },
    SetAccountSkin {
        account: Uuid,
        skin: UniqueBytes,
        variant: SkinVariant,
    },
    GetAccountCapes {
        account: Uuid,
        result: tokio::sync::oneshot::Sender<AccountCapesResult>,
    },
    SetAccountCape {
        account: Uuid,
        cape: Option<Uuid>,
    },
    RequestSkinLibrary,
    RemoveFromSkinLibrary{
        skin: UniqueBytes,
    },
    AddToSkinLibrary {
        source: UrlOrFile,
    },
    CopyPlayerSkin {
        username: Arc<str>,
    },
    Login {
        account: Uuid,
        modal_action: ModalAction,
    },
    Quit,
}

#[derive(Debug)]
pub enum MessageToFrontend {
    InstanceAdded {
        id: InstanceID,
        name: Ustr,
        icon: Option<UniqueBytes>,
        root_path: Arc<Path>,
        dot_minecraft_folder: Arc<Path>,
        configuration: InstanceConfiguration,
        playtime: InstancePlaytime,
        worlds_state: BridgeDataLoadState,
        servers_state: BridgeDataLoadState,
        content_states: enum_map::EnumMap<ContentFolder, BridgeDataLoadState>,
    },
    InstanceRemoved {
        id: InstanceID,
    },
    InstanceModified {
        id: InstanceID,
        name: Ustr,
        icon: Option<UniqueBytes>,
        root_path: Arc<Path>,
        dot_minecraft_folder: Arc<Path>,
        configuration: InstanceConfiguration,
        playtime: InstancePlaytime,
        status: InstanceStatus,
    },
    InstancePlaytimeUpdated {
        id: InstanceID,
        playtime: InstancePlaytime,
    },
    InstanceWorldsUpdated {
        id: InstanceID,
        worlds: Arc<[InstanceWorldSummary]>,
    },
    InstanceServersUpdated {
        id: InstanceID,
        servers: Arc<[InstanceServerSummary]>,
    },
    InstanceContentUpdated {
        id: InstanceID,
        content_folder: ContentFolder,
        content: Arc<[InstanceContentSummary]>,
    },
    CreateGameOutputWindow {
        receiver: tokio::sync::mpsc::UnboundedReceiver<GameOutputMsg>
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
    Quit,
    CloseModal,
    MoveInstanceToTop {
        id: InstanceID,
    },
    MetadataResult {
        request: MetadataRequest,
        result: Result<MetadataResult, Arc<str>>,
        keep_alive_handle: Option<KeepAliveHandle>,
    },
    SkinLibraryUpdated {
        skin_library: SkinLibrary,
    },
    UpdateAvailable {
        update: UpdatePrompt,
    },
    OpenOrFocusMainWindow,
}

#[derive(Debug, Default)]
pub struct LogFiles {
    pub paths: Vec<Arc<Path>>,
    pub total_gzipped_size: usize,
}

#[derive(Debug)]
pub struct SyncTargetState {
    pub enabled: bool,
    pub is_file: bool,
    pub sync_count: usize,
    pub cannot_sync_count: usize,
}

#[derive(Debug)]
pub struct SyncState {
    pub sync_folder: Arc<Path>,
    pub targets: BTreeMap<Arc<str>, SyncTargetState>,
    pub total_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BridgeNotificationType {
    Success,
    Info,
    Error,
    Warning,
}

#[derive(Clone, Debug)]
pub struct BridgeDataLoadState(Arc<AtomicU8>);

impl Default for BridgeDataLoadState {
    fn default() -> Self {
        Self(Arc::new(AtomicU8::new(BridgeDataLoadState::UNLOADED)))
    }
}

impl BridgeDataLoadState {
    const LOADING: u8 = 1;
    const OBSERVED: u8 = 2;
    const DIRTY: u8 = 4;
    const UNLOADED: u8 = !Self::LOADING;

    pub fn should_load(&self) -> bool {
        // Must be observed and dirty, but not loading
        let value = self.0.load(std::sync::atomic::Ordering::Acquire);
        (value == Self::OBSERVED | Self::DIRTY) || (value == Self::UNLOADED)
    }

    pub fn is_not_unloaded(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Acquire) != Self::UNLOADED
    }

    pub fn set_observed(&self) {
        self.0.fetch_or(Self::OBSERVED, std::sync::atomic::Ordering::AcqRel);
    }

    pub fn set_dirty(&self) {
        self.0.fetch_or(Self::DIRTY, std::sync::atomic::Ordering::AcqRel);
    }

    pub fn load_started(&self) {
        self.0.store(Self::LOADING, std::sync::atomic::Ordering::Release);
    }

    pub fn load_finished(&self) {
        self.0.fetch_and(!Self::LOADING, std::sync::atomic::Ordering::AcqRel);
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
    Raw(UniqueBytes),
}

#[derive(Debug, Clone)]
pub enum AccountSkinResult {
    Success {
        skin: Option<UniqueBytes>,
        variant: SkinVariant,
    },
    NeedsLogin,
    UnableToLoadSkin,
}

#[derive(Debug, Clone)]
pub enum AccountCapesResult {
    Success {
        capes: Vec<MinecraftProfileCape>,
    },
    NeedsLogin,
}

#[derive(Clone, Debug)]
pub struct SkinLibrary {
    pub state: BridgeDataLoadState,
    pub skins: Arc<[UniqueBytes]>,
    pub folder: Arc<Path>
}

pub enum UrlOrFile {
    Url {
        url: Arc<str>,
    },
    File {
        path: PathBuf,
    }
}

pub struct GameOutputMsg {
    pub time: i64,
    pub level: GameOutputLogLevel,
    pub text: Arc<[Arc<str>]>,
}
