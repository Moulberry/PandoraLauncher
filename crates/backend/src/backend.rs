use std::{
    collections::{HashMap, HashSet}, io::Cursor, path::{Path, PathBuf}, sync::Arc, time::{Duration, SystemTime}
};

use auth::{
    authenticator::{Authenticator, MsaAuthorizationError, XboxAuthenticateError},
    credentials::{AccountCredentials, AUTH_STAGE_COUNT},
    models::{MinecraftAccessToken, MinecraftProfileResponse, SkinState},
    secret::{PlatformSecretStorage, SecretStorageError},
    serve_redirect::{self, ProcessAuthorizationError},
};
use base64::Engine;
use bridge::{
    handle::{BackendHandle, BackendReceiver, FrontendHandle}, install::{ContentDownload, ContentInstall, ContentInstallFile, ContentInstallPath}, instance::{ContentType, InstanceContentSummary, InstanceID, InstanceServerSummary, InstanceWorldSummary}, message::{EmbeddedOrRaw, MessageToFrontend}, modal_action::{ModalAction, ModalActionVisitUrl, ProgressTracker, ProgressTrackerFinishType}, safe_path::SafePath
};
use image::ImageFormat;
use indexmap::IndexSet;
use parking_lot::RwLock;
use reqwest::{StatusCode, redirect::Policy};
use rustc_hash::{FxHashMap, FxHashSet};
use schema::{auxiliary::AuxiliaryContentMeta, backend_config::{BackendConfig, SyncTargets, ProxyConfig}, instance::InstanceConfiguration, loader::Loader, modrinth::ModrinthSideRequirement};
use serde::Deserialize;
use sha1::{Digest, Sha1};
use tokio::sync::{mpsc::Receiver, OnceCell};
use ustr::Ustr;
use uuid::Uuid;

use crate::{
    account::{BackendAccountInfo, MinecraftLoginInfo}, directories::LauncherDirectories, id_slab::IdSlab, instance::{Instance, ContentFolder}, launch::Launcher, metadata::{items::MinecraftVersionManifestMetadataItem, manager::MetadataManager}, mod_metadata::ModMetadataManager, persistent::Persistent
};

fn build_http_clients(user_agent: &str, proxy_config: &ProxyConfig, proxy_password: Option<&str>) -> (reqwest::Client, reqwest::Client) {
    let proxy_url = proxy_config.to_url(proxy_password);

    let mut http_builder = reqwest::ClientBuilder::new()
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(15))
        .redirect(Policy::none())
        .use_rustls_tls()
        .user_agent(user_agent);

    let mut redirecting_builder = reqwest::ClientBuilder::new()
        .use_rustls_tls()
        .user_agent(user_agent);

    if let Some(proxy_url) = &proxy_url {
        if let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
            let proxy = proxy.no_proxy(reqwest::NoProxy::from_env());
            http_builder = http_builder.proxy(proxy.clone());
            redirecting_builder = redirecting_builder.proxy(proxy);
            log::info!("Proxy configured: {}://{}:{}", proxy_config.protocol.scheme(), proxy_config.host, proxy_config.port);
        } else {
            log::warn!("Failed to parse proxy URL, proceeding without proxy");
        }
    }

    let http_client = http_builder.build().expect("Failed to build HTTP client");
    let redirecting_http_client = redirecting_builder.build().expect("Failed to build redirecting HTTP client");

    (http_client, redirecting_http_client)
}

pub fn start(launcher_dir: PathBuf, send: FrontendHandle, self_handle: BackendHandle, recv: BackendReceiver) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime");

    let user_agent = if let Some(version) = option_env!("PANDORA_RELEASE_VERSION") {
        format!("PandoraLauncher/{version} (https://github.com/Moulberry/PandoraLauncher)")
    } else {
        "PandoraLauncher/dev (https://github.com/Moulberry/PandoraLauncher)".to_string()
    };

    let directories = Arc::new(LauncherDirectories::new(launcher_dir));

    let mut config: Persistent<BackendConfig> = Persistent::load(directories.config_json.clone());
    let proxy_config = config.get().proxy.clone();
    let proxy_password: Option<String> = if proxy_config.enabled && proxy_config.auth_enabled {
        runtime.block_on(async {
            match PlatformSecretStorage::new().await {
                Ok(storage) => match storage.read_proxy_password().await {
                    Ok(password) => password,
                    Err(e) => {
                        log::warn!("Failed to read proxy password from keyring: {:?}", e);
                        None
                    }
                },
                Err(e) => {
                    log::warn!("Failed to initialize secret storage: {:?}", e);
                    None
                }
            }
        })
    } else {
        None
    };

    let (http_client, redirecting_http_client) = build_http_clients(&user_agent, &proxy_config, proxy_password.as_deref());

    let meta = Arc::new(MetadataManager::new(
        http_client.clone(),
        directories.metadata_dir.clone(),
    ));

    let (watcher_tx, watcher_rx) = tokio::sync::mpsc::channel::<notify_debouncer_full::DebounceEventResult>(64);
    let watcher = notify_debouncer_full::new_debouncer(Duration::from_millis(100), None, move |event| {
        let _ = watcher_tx.blocking_send(event);
    }).unwrap();

    let mod_metadata_manager = ModMetadataManager::load(directories.content_meta_dir.clone(), directories.content_library_dir.clone());

    let state_instances = BackendStateInstances {
        instances: IdSlab::default(),
        instances_generation: 0,
        reload_immediately: Default::default(),
    };

    let mut state_file_watching = BackendStateFileWatching {
        watcher,
        watching: HashMap::new(),
        symlink_src_to_links: Default::default(),
        symlink_link_to_src: Default::default(),
    };

    // Create initial directories
    let _ = std::fs::create_dir_all(&directories.instances_dir);
    state_file_watching.watch_filesystem(directories.root_launcher_dir.clone(), WatchTarget::RootDir);

    // Load accounts
    let account_info = Persistent::load(directories.accounts_json.clone());

    let mut state = BackendState {
        self_handle,
        send: send.clone(),
        http_client,
        redirecting_http_client,
        meta: Arc::clone(&meta),
        instance_state: Arc::new(RwLock::new(state_instances)),
        file_watching: Arc::new(RwLock::new(state_file_watching)),
        directories: Arc::clone(&directories),
        launcher: Launcher::new(meta, directories, send),
        mod_metadata_manager: Arc::new(mod_metadata_manager),
        account_info: Arc::new(RwLock::new(account_info)),
        config: Arc::new(RwLock::new(config)),
        secret_storage: Arc::new(OnceCell::new()),
        head_cache: Default::default(),
    };

    log::debug!("Doing initial backend load");

    runtime.block_on(async {
        state.send.send(state.account_info.write().get().create_update_message());
        state.load_all_instances().await;
    });

    runtime.spawn(state.start(recv, watcher_rx));

    std::mem::forget(runtime);
}

#[derive(Debug, Clone, Copy)]
pub enum WatchTarget {
    RootDir,
    InstancesDir,
    InvalidInstanceDir,
    InstanceDir { id: InstanceID },
    InstanceDotMinecraftDir { id: InstanceID },
    InstanceWorldDir { id: InstanceID },
    InstanceSavesDir { id: InstanceID },
    ServersDat { id: InstanceID },
    InstanceContentDir { id: InstanceID, folder: ContentFolder },
}

pub struct BackendStateInstances {
    pub instances: IdSlab<Instance>,
    pub instances_generation: usize,
    pub reload_immediately: FxHashSet<(InstanceID, ContentFolder)>,
}

pub struct BackendStateFileWatching {
    watcher: notify_debouncer_full::Debouncer<notify::RecommendedWatcher, notify_debouncer_full::RecommendedCache>,
    watching: HashMap<Arc<Path>, WatchTarget>,
    symlink_src_to_links: HashMap<Arc<Path>, IndexSet<Arc<Path>>>,
    symlink_link_to_src: HashMap<Arc<Path>, Arc<Path>>,
}

#[derive(Clone)]
pub struct BackendState {
    pub self_handle: BackendHandle,
    pub send: FrontendHandle,
    pub http_client: reqwest::Client,
    pub redirecting_http_client: reqwest::Client,
    pub meta: Arc<MetadataManager>,
    pub instance_state: Arc<RwLock<BackendStateInstances>>,
    pub file_watching: Arc<RwLock<BackendStateFileWatching>>,
    pub directories: Arc<LauncherDirectories>,
    pub launcher: Launcher,
    pub mod_metadata_manager: Arc<ModMetadataManager>,
    pub account_info: Arc<RwLock<Persistent<BackendAccountInfo>>>,
    pub config: Arc<RwLock<Persistent<BackendConfig>>>,
    pub secret_storage: Arc<OnceCell<Result<PlatformSecretStorage, SecretStorageError>>>,
    pub head_cache: Arc<RwLock<FxHashMap<Arc<str>, HeadCacheEntry>>>
}

pub enum HeadCacheEntry {
    Pending {
        accounts: Vec<Uuid>,
    },
    Success {
        head: Arc<[u8]>,
    },
    Failed,
}

impl BackendState {
    async fn start(self, recv: BackendReceiver, watcher_rx: Receiver<notify_debouncer_full::DebounceEventResult>) {
        log::info!("Starting backend");

        tokio::task::spawn(crate::update::check_for_updates(self.redirecting_http_client.clone(), self.send.clone()));

        // Pre-fetch version manifest
        self.meta.load(&MinecraftVersionManifestMetadataItem).await;

        self.handle(recv, watcher_rx).await;
    }

    pub async fn load_all_instances(&mut self) {
        log::info!("Loading all instances");

        let mut paths_with_time = Vec::new();

        self.file_watching.write().watch_filesystem(self.directories.instances_dir.clone(), WatchTarget::InstancesDir);
        for entry in std::fs::read_dir(&self.directories.instances_dir).unwrap() {
            let Ok(entry) = entry else {
                log::warn!("Error reading directory in instances folder: {:?}", entry.unwrap_err());
                continue;
            };

            let path = entry.path();

            let mut time = SystemTime::UNIX_EPOCH;
            if let Ok(metadata) = path.metadata() {
                if let Ok(created) = metadata.created() {
                    time = time.max(created);
                }
                if let Ok(modified) = metadata.modified() {
                    time = time.max(modified);
                }
            }

            // options.txt exists in every minecraft version, so we use its
            // modified time to determine the latest instance as well
            let mut options_txt = path.join(".minecraft");
            options_txt.push("options.txt");
            if let Ok(metadata) = options_txt.metadata() {
                if let Ok(created) = metadata.created() {
                    time = time.max(created);
                }
                if let Ok(modified) = metadata.modified() {
                    time = time.max(modified);
                }
            }

            paths_with_time.push((path, time));
        }

        paths_with_time.sort_by_key(|(_, time)| *time);
        for (path, _) in paths_with_time {
            let success = self.load_instance_from_path(&path, true, false);
            if !success {
                self.file_watching.write().watch_filesystem(path.into(), WatchTarget::InvalidInstanceDir);
            }
        }
    }

    pub fn remove_instance(&mut self, id: InstanceID) {
        log::info!("Removing instance {id:?}");

        let mut instance_state = self.instance_state.write();

        if let Some(instance) = instance_state.instances.remove(id) {
            self.send.send(MessageToFrontend::InstanceRemoved { id });
            self.send.send_info(format!("Instance '{}' removed", instance.name));
        }
    }

    pub fn load_instance_from_path(&mut self, path: &Path, mut show_errors: bool, show_success: bool) -> bool {
        let instance = Instance::load_from_folder(&path);

        let instance_id = {
            let mut instance_state_guard = self.instance_state.write();
            let instance_state = &mut *instance_state_guard;

            let Ok(mut instance) = instance else {
                instance_state.instances.retain_mut(|existing| {
                    if &*existing.root_path == path {
                        self.send.send(MessageToFrontend::InstanceRemoved { id: existing.id});
                        show_errors = true;
                        false
                    } else {
                        true
                    }
                });

                if show_errors {
                    let error = instance.unwrap_err();
                    self.send.send_error(format!("Unable to load instance from {:?}:\n{}", &path, &error));
                    log::error!("Error loading instance: {:?}", &error);
                }

                return false;
            };

            for existing in instance_state.instances.iter_mut() {
                if &*existing.root_path != path {
                    continue;
                }

                existing.copy_basic_attributes_from(instance);

                let _ = self.send.send(existing.create_modify_message());

                if show_success {
                    self.send.send_info(format!("Instance '{}' updated", existing.name));
                }

                return true;
            }

            let generation = instance_state.instances_generation;
            instance_state.instances_generation = instance_state.instances_generation.wrapping_add(1);

            let instance = instance_state.instances.insert(move |index| {
                let instance_id = InstanceID {
                    index,
                    generation,
                };
                instance.id = instance_id;
                instance
            });

            if show_success {
                self.send.send_success(format!("Instance '{}' created", instance.name));
            }
            let message = MessageToFrontend::InstanceAdded {
                id: instance.id,
                name: instance.name,
                icon: instance.icon.clone(),
                dot_minecraft_folder: instance.dot_minecraft_path.clone(),
                configuration: instance.configuration.get().clone(),
                worlds_state: Arc::clone(&instance.worlds_state),
                servers_state: Arc::clone(&instance.servers_state),
                mods_state: Arc::clone(&instance.content_state[ContentFolder::Mods].load_state),
                resource_packs_state: Arc::clone(&instance.content_state[ContentFolder::ResourcePacks].load_state),
            };
            self.send.send(message);

            instance.id
        };

        self.file_watching.write().watch_filesystem(path.into(), WatchTarget::InstanceDir { id: instance_id });
        true
    }

    async fn handle(mut self, mut backend_recv: BackendReceiver, mut watcher_rx: Receiver<notify_debouncer_full::DebounceEventResult>) {
        let mut interval = tokio::time::interval(Duration::from_millis(1000));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        tokio::pin!(interval);

        loop {
            tokio::select! {
                message = backend_recv.recv() => {
                    if let Some(message) = message {
                        self.handle_message(message).await;
                    } else {
                        log::info!("Backend receiver has shut down");
                        break;
                    }
                },
                instance_change = watcher_rx.recv() => {
                    if let Some(instance_change) = instance_change {
                        self.handle_filesystem(instance_change).await;
                    } else {
                        log::info!("Backend filesystem has shut down");
                        break;
                    }
                },
                _ = interval.tick() => {
                    self.handle_tick().await;
                }
            }
        }
    }

    async fn handle_tick(&mut self) {
        self.meta.expire().await;

        let mut instance_state = self.instance_state.write();
        for instance in instance_state.instances.iter_mut() {
            if let Some(child) = &mut instance.child
                && !matches!(child.try_wait(), Ok(None))
            {
                log::debug!("Child process is no longer alive");
                instance.child = None;
                self.send.send(instance.create_modify_message());
            }
        }
    }

    pub async fn login(
        &self,
        credentials: &mut AccountCredentials,
        login_tracker: &ProgressTracker,
        modal_action: &ModalAction,
    ) -> Result<(MinecraftProfileResponse, MinecraftAccessToken), LoginError> {
        log::info!("Starting login");

        let mut authenticator = Authenticator::new(self.http_client.clone());

        login_tracker.set_total(AUTH_STAGE_COUNT as usize + 1);
        login_tracker.notify();

        let mut last_auth_stage = None;
        let mut allow_backwards = true;
        loop {
            if modal_action.has_requested_cancel() {
                return Err(LoginError::CancelledByUser);
            }

            let stage_with_data = credentials.stage();
            let stage = stage_with_data.stage();

            login_tracker.set_count(stage as usize + 1);
            login_tracker.notify();

            if let Some(last_stage) = last_auth_stage {
                if stage > last_stage {
                    allow_backwards = false;
                } else if stage < last_stage && !allow_backwards {
                    log::error!(
                        "Stage {:?} went backwards from {:?} when going backwards isn't allowed. This is most likely a bug with the auth flow!",
                        stage, last_stage
                    );
                    return Err(LoginError::LoginStageErrorBackwards);
                } else if stage == last_stage {
                    log::error!("Stage {:?} didn't change. This is most likely a bug with the auth flow!", stage);
                    return Err(LoginError::LoginStageErrorDidntChange);
                }
            }
            last_auth_stage = Some(stage);

            match credentials.stage() {
                auth::credentials::AuthStageWithData::Initial => {
                    log::debug!("Auth Flow: Initial");

                    let pending = authenticator.create_authorization();
                    modal_action.set_visit_url(ModalActionVisitUrl {
                        message: "Login with Microsoft".into(),
                        url: pending.url.as_str().into(),
                        prevent_auto_finish: false,
                    });
                    self.send.send(MessageToFrontend::Refresh);

                    log::debug!("Starting serve_redirect server");
                    let finished = tokio::select! {
                        finished = serve_redirect::start_server(pending) => finished?,
                        _ = modal_action.request_cancel.cancelled() => {
                            return Err(LoginError::CancelledByUser);
                        }
                    };

                    log::debug!("serve_redirect handled successfully");

                    modal_action.unset_visit_url();
                    self.send.send(MessageToFrontend::Refresh);

                    log::debug!("Finishing authorization, getting msa tokens");
                    let msa_tokens = authenticator.finish_authorization(finished).await?;

                    credentials.msa_access = Some(msa_tokens.access);
                    credentials.msa_refresh = msa_tokens.refresh;
                    credentials.msa_refresh_force_client_id = None;
                },
                auth::credentials::AuthStageWithData::MsaRefresh(refresh) => {
                    log::debug!("Auth Flow: MsaRefresh");

                    match authenticator.refresh_msa(&refresh, &credentials.msa_refresh_force_client_id).await {
                        Ok(Some(msa_tokens)) => {
                            credentials.msa_access = Some(msa_tokens.access);
                            credentials.msa_refresh = msa_tokens.refresh;
                        },
                        Ok(None) => {
                            if !allow_backwards {
                                return Err(MsaAuthorizationError::InvalidGrant.into());
                            }
                            credentials.msa_refresh = None;
                            credentials.msa_refresh_force_client_id = None;
                        },
                        Err(error) => {
                            if !allow_backwards || error.is_connection_error() {
                                return Err(error.into());
                            }
                            if !matches!(error, MsaAuthorizationError::InvalidGrant) {
                                log::warn!("Error using msa refresh to get msa access: {:?}", error);
                            }
                            credentials.msa_refresh = None;
                            credentials.msa_refresh_force_client_id = None;
                        },
                    }
                },
                auth::credentials::AuthStageWithData::MsaAccess(access) => {
                    log::debug!("Auth Flow: MsaAccess");

                    match authenticator.authenticate_xbox(&access).await {
                        Ok(xbl) => {
                            credentials.xbl = Some(xbl);
                        },
                        Err(error) => {
                            if !allow_backwards || error.is_connection_error() {
                                return Err(error.into());
                            }
                            if !matches!(error, XboxAuthenticateError::NonOkHttpStatus(StatusCode::UNAUTHORIZED)) {
                                log::warn!("Error using msa access to get xbl token: {:?}", error);
                            }
                            credentials.msa_access = None;
                        },
                    }
                },
                auth::credentials::AuthStageWithData::XboxLive(xbl) => {
                    log::debug!("Auth Flow: XboxLive");

                    match authenticator.obtain_xsts(&xbl).await {
                        Ok(xsts) => {
                            credentials.xsts = Some(xsts);
                        },
                        Err(error) => {
                            if !allow_backwards || error.is_connection_error() {
                                return Err(error.into());
                            }
                            if !matches!(error, XboxAuthenticateError::NonOkHttpStatus(StatusCode::UNAUTHORIZED)) {
                                log::warn!("Error using xbl to get xsts: {:?}", error);
                            }
                            credentials.xbl = None;
                        },
                    }
                },
                auth::credentials::AuthStageWithData::XboxSecure { xsts, userhash } => {
                    log::debug!("Auth Flow: XboxSecure");

                    match authenticator.authenticate_minecraft(&xsts, &userhash).await {
                        Ok(token) => {
                            credentials.access_token = Some(token);
                        },
                        Err(error) => {
                            if !allow_backwards || error.is_connection_error() {
                                return Err(error.into());
                            }
                            if !matches!(error, XboxAuthenticateError::NonOkHttpStatus(StatusCode::UNAUTHORIZED)) {
                                log::warn!("Error using xsts to get minecraft access token: {:?}", error);
                            }
                            credentials.xsts = None;
                        },
                    }
                },
                auth::credentials::AuthStageWithData::AccessToken(access_token) => {
                    log::debug!("Auth Flow: AccessToken");

                    match authenticator.get_minecraft_profile(&access_token).await {
                        Ok(profile) => {
                            login_tracker.set_count(AUTH_STAGE_COUNT as usize + 1);
                            login_tracker.notify();

                            return Ok((profile, access_token));
                        },
                        Err(error) => {
                            if !allow_backwards || error.is_connection_error() {
                                return Err(error.into());
                            }
                            if !matches!(error, XboxAuthenticateError::NonOkHttpStatus(StatusCode::UNAUTHORIZED)) {
                                log::warn!("Error using access token to get profile: {:?}", error);
                            }
                            credentials.access_token = None;
                        },
                    }
                },
            }
        }
    }

    pub fn update_profile_head(&self, profile: &MinecraftProfileResponse) {
        log::info!("Updating profile head for {}", profile.id);

        let Some(skin) = profile.skins.iter().find(|skin| skin.state == SkinState::Active).cloned() else {
            return;
        };

        let mut head_cache = self.head_cache.write();
        if let Some(existing) = head_cache.get_mut(&skin.url) {
            match existing {
                HeadCacheEntry::Pending { accounts } => {
                    accounts.push(profile.id);
                },
                HeadCacheEntry::Success { head } => {
                    let head = head.clone();
                    drop(head_cache);
                    self.account_info.write().modify(move |account_info| {
                        if let Some(account) = account_info.accounts.get_mut(&profile.id) {
                            account.head = Some(head);
                        }
                    });
                },
                HeadCacheEntry::Failed => {}
            }
            return;
        }

        head_cache.insert(skin.url.clone(), HeadCacheEntry::Pending { accounts: vec![profile.id] });

        let head_cache = self.head_cache.clone();
        let account_info = self.account_info.clone();
        let skin_url = skin.url;

        let http_client = self.http_client.clone();

        tokio::task::spawn(async move {
            log::info!("Downloading skin from {}", skin_url);
            let Ok(response) = http_client.get(&*skin_url).send().await else {
                log::warn!("Http error while requesting skin from {}", skin_url);
                head_cache.write().insert(skin_url.clone(), HeadCacheEntry::Failed);
                return;
            };
            let Ok(bytes) = response.bytes().await else {
                log::warn!("Http error while downloading skin bytes from {}", skin_url);
                head_cache.write().insert(skin_url.clone(), HeadCacheEntry::Failed);
                return;
            };
            let Ok(mut image) = image::load_from_memory(&bytes) else {
                log::warn!("Image load error for skin from {}", skin_url);
                head_cache.write().insert(skin_url.clone(), HeadCacheEntry::Failed);
                return;
            };

            let mut head = image.crop(8, 8, 8, 8);
            let head_overlay = image.crop(40, 8, 8, 8);

            image::imageops::overlay(&mut head, &head_overlay, 0, 0);

            let mut head_bytes = Vec::new();
            let mut cursor = Cursor::new(&mut head_bytes);
            if head.write_to(&mut cursor, image::ImageFormat::Png).is_err() {
                head_cache.write().insert(skin_url.clone(), HeadCacheEntry::Failed);
                return;
            }

            let head_png: Arc<[u8]> = Arc::from(head_bytes);

            let accounts = {
                let mut head_cache = head_cache.write();
                let previous = head_cache.insert(skin_url.clone(), HeadCacheEntry::Success { head: head_png.clone() });

                if let Some(HeadCacheEntry::Pending { accounts }) = previous {
                    accounts
                } else {
                    Vec::new()
                }
            };

            log::info!("Successfully downloaded skin from {}", skin_url);

            if accounts.is_empty() {
                return;
            }

            let mut account_info = account_info.write();
            account_info.modify(move |info| {
                for uuid in accounts {
                    if let Some(account) = info.accounts.get_mut(&uuid) {
                        account.head = Some(head_png.clone());
                    }
                }
            });
        });
    }

    pub async fn prelaunch(&self, id: InstanceID, modal_action: &ModalAction) -> Vec<PathBuf> {
        self.apply_syncing_to_instance(id);
        self.prelaunch_apply_modpacks(id, modal_action).await
    }

    pub fn apply_syncing_to_instance(&self, id: InstanceID) {
        let (disable, path) = if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
            (instance.configuration.get().disable_file_syncing, instance.dot_minecraft_path.clone())
        } else {
            return;
        };

        if disable {
            crate::syncing::apply_to_instance(&SyncTargets::default(), &self.directories, path);
        } else {
            crate::syncing::apply_to_instance(&self.config.write().get().sync_targets, &self.directories, path);
        }
    }

    pub async fn prelaunch_apply_modpacks(&self, id: InstanceID, modal_action: &ModalAction) -> Vec<PathBuf> {
        let (loader, minecraft_version, mod_dir) = if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
            let configuration = instance.configuration.get();
            (configuration.loader, configuration.minecraft_version, instance.content_state[ContentFolder::Mods].path.clone())
        } else {
            return Vec::new();
        };

        if loader == Loader::Vanilla {
            return Vec::new();
        }

        let Some(mods) = self.clone().load_instance_content(id, ContentFolder::Mods).await else {
            return Vec::new();
        };

        struct HashedDownload {
            sha1: Arc<str>,
            path: Arc<str>,
        }

        struct ModpackInstall {
            hashed_downloads: Vec<HashedDownload>,
            aux_path: Option<PathBuf>,
            overrides: Arc<[(SafePath, Arc<[u8]>)]>,
        }

        let loader_supports_add_mods = loader == Loader::Fabric;

        // Remove .pandora.filename mods
        if let Ok(read_dir) = std::fs::read_dir(&mod_dir) {
            for entry in read_dir {
                let Ok(entry) = entry else {
                    continue;
                };
                let file_name = entry.file_name();
                if file_name.to_string_lossy().starts_with(".pandora.") {
                    log::trace!("Removing temporary mod file {:?}", &file_name);
                    _ = std::fs::remove_file(entry.path());
                }
            }
        }

        let mut modpack_installs = Vec::new();

        for summary in &*mods {
            if !summary.enabled {
                continue;
            }

            if let ContentType::ModrinthModpack { downloads, overrides, .. } = &summary.content_summary.extra {
                let downloads = downloads.clone();

                let filtered_downloads = downloads.iter().filter(|dl| {
                    if let Some(env) = dl.env {
                        if env.client == ModrinthSideRequirement::Unsupported {
                            return false;
                        }
                    }

                    if let Some(metadata) = self.mod_metadata_manager.get_cached_by_sha1(&*dl.hashes.sha1) {
                        if let Some(id) = &metadata.id && summary.disabled_children.disabled_ids.contains(id) {
                            return false;
                        }
                        if let Some(name) = &metadata.name && summary.disabled_children.disabled_names.contains(name) {
                            return false;
                        }
                    }

                    !summary.disabled_children.disabled_filenames.contains(&dl.path)
                });

                let content_install = ContentInstall {
                    target: bridge::install::InstallTarget::Library,
                    loader_hint: loader,
                    version_hint: Some(minecraft_version.into()),
                    files: filtered_downloads.clone().filter_map(|file| {
                        let path = SafePath::new(&file.path)?;
                        Some(ContentInstallFile {
                            replace_old: None,
                            path: ContentInstallPath::Safe(path),
                            download: ContentDownload::Url {
                                url: file.downloads[0].clone(),
                                sha1: file.hashes.sha1.clone(),
                                size: file.file_size,
                            },
                            content_source: schema::content::ContentSource::ModrinthUnknown,
                        })
                    }).collect(),
                };

                self.install_content(content_install, modal_action.clone()).await;

                modpack_installs.push(ModpackInstall {
                    hashed_downloads: filtered_downloads.map(|download| {
                        HashedDownload {
                            sha1: download.hashes.sha1.clone(),
                            path: download.path.clone(),
                        }
                    }).collect(),
                    aux_path: crate::pandora_aux_path_for_content(&summary),
                    overrides: overrides.clone(),
                });
            }
        }

        let dot_minecraft_path = if let Some(instance) = self.instance_state.read().instances.get(id) {
            instance.dot_minecraft_path.clone()
        } else {
            return Vec::new();
        };

        let mut add_mods = Vec::new();

        for modpack_install in modpack_installs {
            let overrides = modpack_install.overrides;
            let content_library_dir = &self.directories.content_library_dir.clone();
            let mut aux: Option<AuxiliaryContentMeta> = if let Some(aux_path) = &modpack_install.aux_path {
                crate::read_json(&aux_path).unwrap_or_default()
            } else {
                None
            };
            let mut aux_changed = false;

            fn should_override_file(path: &str, dest: &Path, new_sha1: [u8; 20], aux: &Option<AuxiliaryContentMeta>) -> bool {
                let Some(aux) = aux else {
                    return true;
                };
                let Some(old_sha1) = aux.applied_overrides.filename_to_hash.get(path) else {
                    return true;
                };

                // Always try to override config/yosbr/ files
                if path.starts_with("config/yosbr/") {
                    return !crate::check_sha1_hash(dest, new_sha1).unwrap_or(false);
                }

                let mut old_hash = [0u8; 20];
                let Ok(_) = hex::decode_to_slice(&**old_sha1, &mut old_hash) else {
                    return true;
                };

                if let Ok(matches) = crate::check_sha1_hash(dest, old_hash) {
                    // Override the file if the hash on disk matches the old hash, and the override has changed
                    // This makes it so that if the file wasn't modified, it'll override with the new version
                    // But if the file was modified by the user, it'll avoid overriding
                    matches && old_hash != new_sha1
                } else {
                    // File doesn't exist, override it
                    true
                }
            }

            for file in modpack_install.hashed_downloads {
                let mut expected_hash = [0u8; 20];
                let Ok(_) = hex::decode_to_slice(&*file.sha1, &mut expected_hash) else {
                    continue;
                };
                let Some(dest_path) = SafePath::new(&file.path) else {
                    continue;
                };

                let path = crate::create_content_library_path(content_library_dir, expected_hash, dest_path.extension());

                if file.path.starts_with("mods/") && file.path.ends_with(".jar") {
                    if loader_supports_add_mods {
                        add_mods.push(path);
                    } else if let Some(filename) = dest_path.file_name() {
                        let filename = format!(".pandora.{filename}");
                        let hidden_dest_path = mod_dir.join(filename);
                        let _ = std::fs::hard_link(path, hidden_dest_path);
                    }
                } else {
                    let dest_path = dest_path.to_path(&dot_minecraft_path);

                    if should_override_file(&file.path, &dest_path, expected_hash, &aux) {
                        if let Some(aux) = &mut aux {
                            aux.applied_overrides.filename_to_hash.insert(file.path.clone(), file.sha1.clone());
                            aux_changed = true;
                        }

                        let _ = std::fs::create_dir_all(dest_path.parent().unwrap());
                        let _ = std::fs::copy(path, dest_path);
                    }
                }
            }

            if !overrides.is_empty() {
                let tracker = ProgressTracker::new("Copying overrides".into(), self.send.clone());
                modal_action.trackers.push(tracker.clone());

                tracker.set_total(overrides.len());
                tracker.notify();

                for (rel_path, file) in overrides.iter() {
                    let mut hasher = Sha1::new();
                    hasher.update(&file);
                    let expected_hash = hasher.finalize().into();

                    let path = crate::create_content_library_path(content_library_dir, expected_hash, rel_path.extension());

                    if !path.exists() {
                        let _ = std::fs::create_dir_all(path.parent().unwrap());
                        let _ = std::fs::write(&path, file);
                    }

                    if rel_path.starts_with("mods") && let Some(extension) = rel_path.extension() && extension == "jar" {
                        if loader_supports_add_mods {
                            add_mods.push(path);
                        } else if let Some(filename) = rel_path.file_name() {
                            let filename = format!(".pandora.{filename}");
                            let hidden_dest_path = mod_dir.join(filename);
                            let _ = std::fs::hard_link(path, hidden_dest_path);
                        }
                    } else {
                        let dest_path = rel_path.to_path(&dot_minecraft_path);

                        if should_override_file(&rel_path.as_str(), &dest_path, expected_hash, &aux) {
                            if let Some(aux) = &mut aux {
                                let sha1 = hex::encode(expected_hash);
                                aux.applied_overrides.filename_to_hash.insert(rel_path.as_str().into(), sha1.into());
                                aux_changed = true;
                            }

                            let _ = std::fs::create_dir_all(dest_path.parent().unwrap());
                            let _ = std::fs::copy(path, dest_path);
                        }
                    }
                    tracker.add_count(1);
                    tracker.notify();
                }

                if let Some(aux_path) = &modpack_install.aux_path && aux_changed {
                    if let Ok(bytes) = serde_json::to_vec(aux.as_ref().unwrap()) {
                        _ = crate::write_safe(&aux_path, &bytes);
                    }
                }

                tracker.set_finished(ProgressTrackerFinishType::Fast);
            }
        }

        add_mods.sort();
        add_mods.dedup();
        add_mods
    }

    pub async fn load_instance_servers(self, id: InstanceID) -> Option<Arc<[InstanceServerSummary]>> {
        if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
            let mut file_watching = self.file_watching.write();
            if !instance.watching_dot_minecraft {
                instance.watching_dot_minecraft = true;
                file_watching.watch_filesystem(instance.dot_minecraft_path.clone(), WatchTarget::InstanceDotMinecraftDir {
                    id: instance.id,
                });
            }
            if !instance.watching_server_dat {
                instance.watching_server_dat = true;
                file_watching.watch_filesystem(instance.server_dat_path.clone(), WatchTarget::ServersDat {
                    id: instance.id,
                });
            }
        }

        let result = Instance::load_servers(self.instance_state.clone(), id).await;

        if let Some((servers, newly_loaded)) = result.clone() && newly_loaded {
            self.send.send(MessageToFrontend::InstanceServersUpdated {
                id,
                servers: Arc::clone(&servers)
            });
        }

        result.map(|(servers, _)| servers)

    }

    pub async fn load_instance_content(self, id: InstanceID, folder: ContentFolder) -> Option<Arc<[InstanceContentSummary]>> {
        if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
            let mut file_watching = self.file_watching.write();
            if !instance.watching_dot_minecraft {
                instance.watching_dot_minecraft = true;
                file_watching.watch_filesystem(instance.dot_minecraft_path.clone(), WatchTarget::InstanceDotMinecraftDir {
                    id: instance.id,
                });
            }
            let content_state = &mut instance.content_state[folder];
            if !content_state.watching_path {
                content_state.watching_path = true;
                file_watching.watch_filesystem(content_state.path.clone(), WatchTarget::InstanceContentDir {
                    id: instance.id,
                    folder
                });
            }
        }

        let result = Instance::load_content(self.instance_state.clone(), id, &self.mod_metadata_manager, folder).await;

        if let Some((content, newly_loaded)) = result.clone() && newly_loaded {
            match folder {
                ContentFolder::Mods => {
                    self.send.send(MessageToFrontend::InstanceModsUpdated {
                        id,
                        mods: Arc::clone(&content)
                    });
                },
                ContentFolder::ResourcePacks => {
                    self.send.send(MessageToFrontend::InstanceResourcePacksUpdated {
                        id,
                        resource_packs: Arc::clone(&content)
                    });
                },
            }
        }

        result.map(|(content, _)| content)
    }

    pub async fn load_instance_worlds(self, id: InstanceID) -> Option<Arc<[InstanceWorldSummary]>> {
        if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
            let mut file_watching = self.file_watching.write();
            if !instance.watching_dot_minecraft {
                instance.watching_dot_minecraft = true;
                file_watching.watch_filesystem(instance.dot_minecraft_path.clone(), WatchTarget::InstanceDotMinecraftDir {
                    id: instance.id,
                });
            }
            if !instance.watching_saves_dir {
                instance.watching_saves_dir = true;
                file_watching.watch_filesystem(instance.saves_path.clone(), WatchTarget::InstanceSavesDir {
                    id: instance.id,
                });
            }
        }

        let result = Instance::load_worlds(self.instance_state.clone(), id).await;

        if let Some((worlds, newly_loaded)) = result.clone() && newly_loaded {
            self.send.send(MessageToFrontend::InstanceWorldsUpdated {
                id,
                worlds: Arc::clone(&worlds)
            });

            let mut file_watching = self.file_watching.write();
            for summary in worlds.iter() {
                file_watching.watch_filesystem(summary.level_path.clone(), WatchTarget::InstanceWorldDir {
                    id,
                });
            }
        }

        result.map(|(worlds, _)| worlds)
    }

    pub async fn create_instance_sanitized(&self, name: &str, version: &str, loader: Loader, icon: Option<EmbeddedOrRaw>) -> Option<PathBuf> {
        let mut name = sanitize_filename::sanitize_with_options(name, sanitize_filename::Options { windows: true, ..Default::default() });

        if self.instance_state.read().instances.iter().any(|i| i.name == name) {
            let original_name = name.clone();
            for i in 1..32 {
                let new_name = format!("{original_name} ({i})");
                if !self.instance_state.read().instances.iter().any(|i| i.name == new_name) {
                    name = new_name;
                    break;
                }
            }
        }

        return self.create_instance(&name, version, loader, icon).await;
    }

    pub async fn create_instance(&self, name: &str, version: &str, loader: Loader, icon: Option<EmbeddedOrRaw>) -> Option<PathBuf> {
        log::info!("Creating instance {name}");
        if loader == Loader::Unknown {
            self.send.send_warning(format!("Unable to create instance, unknown loader"));
            return None;
        }
        if !crate::is_single_component_path_str(&name) {
            self.send.send_warning(format!("Unable to create instance, name must not be a path: {}", name));
            return None;
        }
        if !sanitize_filename::is_sanitized_with_options(&*name, sanitize_filename::OptionsForCheck { windows: true, ..Default::default() }) {
            self.send.send_warning(format!("Unable to create instance, name is invalid: {}", name));
            return None;
        }
        if self.instance_state.read().instances.iter().any(|i| i.name == name) {
            self.send.send_warning("Unable to create instance, name is already used".to_string());
            return None;
        }

        self.file_watching.write().watch_filesystem(self.directories.instances_dir.clone(), WatchTarget::InstancesDir);

        let instance_dir = self.directories.instances_dir.join(name);

        _ = std::fs::create_dir_all(&instance_dir);

        let mut instance_info = InstanceConfiguration::new(version.into(), loader);

        match icon {
            Some(EmbeddedOrRaw::Embedded(e)) => {
                instance_info.instance_fallback_icon = Some(e.into());
            },
            Some(EmbeddedOrRaw::Raw(image_bytes)) => {
                if let Ok(format) = image::guess_format(&*image_bytes) {
                    if format == ImageFormat::Png {
                        let icon_path = instance_dir.join("icon.png");
                        crate::write_safe(&icon_path, &*image_bytes).unwrap();
                    } else {
                        self.send.send_error("Unable to apply icon: only pngs are supported");
                    }
                } else {
                    self.send.send_error("Unable to apply icon: unknown format");
                }
            },
            None => {},
        }

        let info_path = instance_dir.join("info_v1.json");
        crate::write_safe(&info_path, serde_json::to_string(&instance_info).unwrap().as_bytes()).unwrap();

        Some(instance_dir.clone())
    }

    pub async fn rename_instance(&self, id: InstanceID, name: &str) {
        if !crate::is_single_component_path_str(&name) {
            self.send.send_warning(format!("Unable to rename instance, name must not be a path: {}", name));
            return;
        }
        if !sanitize_filename::is_sanitized_with_options(&*name, sanitize_filename::OptionsForCheck { windows: true, ..Default::default() }) {
            self.send.send_warning(format!("Unable to rename instance, name is invalid: {}", name));
            return;
        }
        if self.instance_state.read().instances.iter().any(|i| i.name == name) {
            self.send.send_warning("Unable to rename instance, name is already used".to_string());
            return;
        }

        let new_instance_dir = self.directories.instances_dir.join(name);

        if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
            let result = std::fs::rename(&instance.root_path, new_instance_dir);
            if let Err(err) = result {
                self.send.send_error(format!("Unable to rename instance folder: {}", err));
            }
        }
    }

    pub async fn get_login_info(&self, modal_action: &ModalAction) -> Option<MinecraftLoginInfo> {
        let selected_account = {
            let mut account_info = self.account_info.write();
            let account_info = account_info.get();

            let mut selected_account = account_info.selected_account;

            if let Some(uuid) = selected_account {
                if let Some(account) = account_info.accounts.get(&uuid) {
                    if account.offline {
                        return Some(MinecraftLoginInfo {
                            uuid,
                            username: account.username.clone(),
                            access_token: None
                        })
                    }
                } else {
                    selected_account = None;
                }
            }

            selected_account
        };

        let Some((profile, access_token)) = self.login_flow(modal_action, selected_account).await else {
            return None;
        };

        Some(MinecraftLoginInfo {
            uuid: profile.id,
            username: profile.name.clone(),
            access_token: Some(access_token),
        })
    }
}

impl BackendStateFileWatching {
    pub fn watch_filesystem(&mut self, path: Arc<Path>, target: WatchTarget) {
        let Ok(canonical) = path.canonicalize() else {
            log::error!("Unable to watch {:?} because it could not be canonicalized", path);
            return;
        };
        let canonical: Arc<Path> = if canonical == &*path {
            log::debug!("Watching {:?} as {:?}", path, target);
            path.clone()
        } else {
            log::debug!("Watching {:?} (real path {:?}) as {:?}", path, canonical, target);
            canonical.into()
        };

        if let Err(err) = self.watcher.watch(&path, notify::RecursiveMode::NonRecursive) {
            log::error!("Unable to watch filesystem: {:?}", err);
            return;
        }
        self.watching.insert(path.clone(), target);

        if canonical != path {
            self.symlink_src_to_links.entry(canonical.clone()).or_default().insert(path.clone());
            self.symlink_link_to_src.insert(path, canonical);
        }
    }

    pub fn get_target(&self, path: &Path) -> Option<&WatchTarget> {
        self.watching.get(path)
    }

    pub fn remove(&mut self, path: &Path) -> Option<WatchTarget> {
        if let Some(src) = self.symlink_link_to_src.remove(path) {
            if let Some(links) = self.symlink_src_to_links.get_mut(&src) {
                links.shift_remove(path);
                if links.is_empty() {
                    self.symlink_src_to_links.remove(&src);
                }
            }
        }
        self.watching.remove(path)
    }

    pub fn all_paths(&self, path: Arc<Path>) -> Vec<Arc<Path>> {
        let mut paths = Vec::new();

        if self.watching.contains_key(&path) {
            paths.push(path.clone());
        } else if let Some(parent) = path.parent() && self.watching.contains_key(parent) {
            paths.push(path.clone());
        }

        if let Some(links) = self.symlink_src_to_links.get(&path) {
            for link in links {
                if self.watching.contains_key(link) {
                    paths.push(link.clone());
                } else if let Some(link_parent) = link.parent() && self.watching.contains_key(link_parent) {
                    paths.push(link.clone());
                }
            }
        }

        if let Some(parent) = path.parent() && let Some(filename) = path.file_name() {
            if let Some(links) = self.symlink_src_to_links.get(parent) {
                for link_parent in links {
                    let child_link: Arc<Path> = link_parent.join(filename).into();
                    if self.watching.contains_key(&child_link) {
                        paths.push(child_link.clone());
                    } else if self.watching.contains_key(link_parent) {
                        paths.push(child_link.clone());
                    }
                }
            }
        }

        paths
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LoginError {
    #[error("Login stage error: Backwards")]
    LoginStageErrorBackwards,
    #[error("Login stage error: Didn't change")]
    LoginStageErrorDidntChange,
    #[error("Process authorization error: {0}")]
    ProcessAuthorizationError(#[from] ProcessAuthorizationError),
    #[error("Microsoft authorization error: {0}")]
    MsaAuthorizationError(#[from] MsaAuthorizationError),
    #[error("XboxLive authentication error: {0}")]
    XboxAuthenticateError(#[from] XboxAuthenticateError),
    #[error("Cancelled by user")]
    CancelledByUser,
}
