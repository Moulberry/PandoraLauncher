use std::{
    collections::HashMap, ffi::OsStr, path::{Path, PathBuf}, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant, SystemTime}
};

use auth::{
    authenticator::{Authenticator, MsaAuthorizationError, XboxAuthenticateError},
    credentials::{AccountCredentials, AUTH_STAGE_COUNT},
    models::MinecraftAccessToken,
    secret::{PlatformSecretStorage, SecretStorageError},
    serve_redirect::{self, ProcessAuthorizationError},
};
use bridge::{
    handle::{BackendHandle, BackendReceiver, FrontendHandle}, install::{ContentDownload, ContentInstall, ContentInstallFile, ContentInstallPath}, instance::{ContentFolder, ContentType, InstanceContentSummary, InstanceID, ModpackFile, ModpackFilePath, ModpackFileSource}, message::{EmbeddedOrRaw, MessageToFrontend}, modal_action::{ModalAction, ModalActionVisitUrl, ProgressTracker, ProgressTrackerFinishType}, quit::QuitCoordinator, safe_path::SafePath
};
use image::ImageFormat;
use indexmap::IndexSet;
use parking_lot::RwLock;
use reqwest::{StatusCode, redirect::Policy};
use rustc_hash::FxHashMap;
use schema::{auxiliary::AuxiliaryContentMeta, backend_config::{BackendConfig, ProxyConfig, SyncTargets}, content::{ContentInstallReason, ContentSource}, curseforge::{CachedCurseforgeFileInfo, CurseforgeGetFilesRequest}, instance::InstanceConfiguration, loader::Loader, minecraft_profile::MinecraftProfileResponse};
use strum::IntoEnumIterator;
use tokio::sync::{OnceCell, Semaphore, mpsc::Receiver};
use ustr::Ustr;
use uuid::Uuid;

use crate::{
    account::{BackendAccountInfo, MinecraftLoginInfo}, directories::LauncherDirectories, id_slab::IdSlab, instance::Instance, launch::Launcher, metadata::{items::{CurseforgeGetFilesMetadataItem, MinecraftVersionManifestMetadataItem}, manager::MetadataManager}, mod_metadata::ModMetadataManager, persistent::Persistent, server_list_pinger::ServerListPinger, skin_manager::SkinManager
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

pub fn start(runtime: tokio::runtime::Runtime, launcher_dir: PathBuf, send: FrontendHandle, self_handle: BackendHandle, recv: BackendReceiver, quit_handler: QuitCoordinator) {
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
    };

    let mut state_file_watching = BackendStateFileWatching {
        watcher,
        watching: HashMap::new(),
        watch_target_to_path: HashMap::new(),
        symlink_src_to_links: Default::default(),
        symlink_link_to_src: Default::default(),
    };

    // Create initial directories
    let _ = std::fs::create_dir_all(&directories.instances_dir);
    state_file_watching.watch_filesystem(directories.root_launcher_dir.clone(), WatchTarget::RootDir);

    // Load accounts
    let account_info = Persistent::load(directories.accounts_json.clone());

    let state = BackendState {
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
        login_semaphore: Arc::new(Semaphore::new(1)),
        cached_minecraft_profiles: Default::default(),
        skin_manager: Default::default(),
        server_list_pinger: Arc::new(ServerListPinger::new()),
        quit_coordinator: quit_handler,
        should_quit: AtomicBool::new(false),
        content_install_semaphore: Semaphore::new(8),
    };

    log::debug!("Doing initial backend load");

    runtime.block_on(async {
        state.send.send(state.account_info.write().get().create_update_message());
        state.load_all_instances().await;
    });

    runtime.spawn(state.start(recv, watcher_rx));

    std::mem::forget(runtime);
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum WatchTarget {
    RootDir,
    InstancesDir,
    InvalidInstanceDir,
    InstanceDir { id: InstanceID },
    InstanceDotMinecraftDir { id: InstanceID },
    InstanceWorldDir { id: InstanceID },
    InstanceSavesDir { id: InstanceID },
    InstanceContentDir { id: InstanceID, folder: ContentFolder },
    SkinLibraryDir,
}

pub struct BackendStateInstances {
    pub instances: IdSlab<Instance>,
    pub instances_generation: usize,
}

pub struct BackendStateFileWatching {
    watcher: notify_debouncer_full::Debouncer<notify::RecommendedWatcher, notify_debouncer_full::RecommendedCache>,
    watching: HashMap<Arc<Path>, WatchTarget>,
    watch_target_to_path: HashMap<WatchTarget, Arc<Path>>,
    symlink_src_to_links: HashMap<Arc<Path>, IndexSet<Arc<Path>>>,
    symlink_link_to_src: HashMap<Arc<Path>, Arc<Path>>,
}

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
    pub login_semaphore: Arc<Semaphore>,
    pub cached_minecraft_profiles: Arc<RwLock<FxHashMap<Uuid, CachedMinecraftProfile>>>,
    pub skin_manager: Arc<RwLock<SkinManager>>,
    pub server_list_pinger: Arc<ServerListPinger>,
    pub quit_coordinator: QuitCoordinator,
    pub should_quit: AtomicBool,
    pub content_install_semaphore: Semaphore,
}

pub struct CachedMinecraftProfile {
    pub profile: MinecraftProfileResponse,
    pub not_before: Instant,
    pub not_after: Instant,
}

impl CachedMinecraftProfile {
    pub fn new(profile: MinecraftProfileResponse) -> Self {
        let now = Instant::now();
        Self {
            profile,
            not_before: now,
            not_after: now + Duration::from_mins(5)
        }
    }

    pub fn is_valid(&self, now: Instant) -> bool {
        now >= self.not_before && now < self.not_after
    }
}

impl BackendState {
    async fn start(self, recv: BackendReceiver, watcher_rx: Receiver<notify_debouncer_full::DebounceEventResult>) {
        log::info!("Starting backend");

        tokio::task::spawn(crate::update::check_for_updates(self.redirecting_http_client.clone(), self.send.clone()));

        // Pre-fetch version manifest
        self.meta.load(&MinecraftVersionManifestMetadataItem).await;

        Arc::new(self).handle(recv, watcher_rx).await;
    }

    pub async fn load_all_instances(&self) {
        log::info!("Loading all instances");

        let mut paths_with_time = Vec::new();

        self.file_watching.write().watch_filesystem(self.directories.instances_dir.clone(), WatchTarget::InstancesDir);
        for entry in std::fs::read_dir(&self.directories.instances_dir).unwrap() {
            let Ok(entry) = entry else {
                log::warn!("Error reading directory in instances folder: {:?}", entry.unwrap_err());
                continue;
            };

            let path = entry.path();

            let Some(file_name) = path.file_name() else {
                continue;
            };
            if file_name.as_encoded_bytes()[0] == b'.' {
                continue;
            }

            let mut time = SystemTime::UNIX_EPOCH;
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    continue;
                }
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

    pub fn remove_instance(&self, id: InstanceID) {
        log::info!("Removing instance {id:?}");

        let mut instance_state = self.instance_state.write();

        if let Some(instance) = instance_state.instances.remove(id) {
            self.send.send(MessageToFrontend::InstanceRemoved { id });
            self.send.send_info(format!("Instance '{}' removed", instance.name));
        }
    }

    pub fn load_instance_from_path(&self, path: &Path, mut show_errors: bool, show_success: bool) -> bool {
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
                existing.rewatch_directories(&mut self.file_watching.write());

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

            self.restore_mods_folder_if_stopped(instance);

            if show_success {
                self.send.send_success(format!("Instance '{}' created", instance.name));
            }
            let message = MessageToFrontend::InstanceAdded {
                id: instance.id,
                name: instance.name,
                icon: instance.icon.clone(),
                root_path: instance.resolve_real_root_path(),
                dot_minecraft_folder: instance.dot_minecraft_path.clone(),
                configuration: instance.configuration.get().clone(),
                playtime: instance.playtime(),
                worlds_state: instance.worlds_state.clone(),
                servers_state: instance.servers_state.clone(),
                content_states: enum_map::EnumMap::from_fn(|folder| {
                    instance.content_state[folder].load_state.clone()
                }),
            };
            self.send.send(message);

            instance.id
        };

        self.file_watching.write().watch_filesystem(path.into(), WatchTarget::InstanceDir { id: instance_id });
        true
    }

    async fn handle(self: Arc<Self>, mut backend_recv: BackendReceiver, mut watcher_rx: Receiver<notify_debouncer_full::DebounceEventResult>) {
        let mut interval = tokio::time::interval(Duration::from_millis(1000));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
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
                event = watcher_rx.recv() => {
                    if let Some(event) = event {
                        self.handle_filesystem(event).await;
                    } else {
                        log::info!("Backend filesystem has shut down");
                        break;
                    }
                },
                _ = interval.tick() => {
                    self.handle_tick().await;
                }
            }

            if self.should_quit.load(Ordering::Relaxed) {
                while let Some(message) = backend_recv.try_recv() {
                    self.handle_message(message).await;
                }
                self.handle_tick().await;
                break;
            }
        }

        self.send.send(MessageToFrontend::Quit);
    }

    async fn handle_tick(&self) { // todo: make this non-async
        self.meta.expire().await;
        self.mod_metadata_manager.write_changes();

        let mut any_process_alive = false;

        let mut instance_state = self.instance_state.write();
        for instance in instance_state.instances.iter_mut() {
            let mut killed = false;

            instance.processes.retain_mut(|process| {
                match process.try_wait() {
                    Ok(None) => true,
                    Ok(Some(status)) => {
                        log::info!("Child process {} is no longer alive: {}", process.id(), status);
                        killed = true;
                        false
                    }
                    Err(err) => {
                        log::error!("An error occured while waiting for process {}: {:?}", process.id(), err);
                        killed = true;
                        false
                    },
                }
            });
            instance.closing_processes.retain_mut(|(process, _)| {
                match process.try_wait() {
                    Ok(None) => true,
                    Ok(Some(status)) => {
                        log::info!("Child process {} closed: {}", process.id(), status);
                        killed = true;
                        false
                    }
                    Err(err) => {
                        log::error!("An error occured while waiting for closing process {}: {:?}", process.id(), err);
                        killed = true;
                        false
                    },
                }
            });

            let now = Instant::now();
            let to_kill = instance.closing_processes.extract_if(.., |(_, deadline)| {
                now > *deadline
            });
            for (process, _) in to_kill {
                log::info!("Force killed process {}", process.id());
                let result = process.kill();
                killed = true;
                if let Err(err) = result {
                    self.send.send_error("Failed to kill instance");
                    log::error!("Failed to kill instance: {err:?}");
                }
            }

            if killed {
                instance.update_session();
                self.send.send(instance.create_modify_message());
            } else if let Some(launch_keepalive) = &instance.launch_keepalive && !launch_keepalive.is_alive() {
                self.send.send(instance.create_modify_message());
            } else if instance.has_active_session() {
                self.send.send(MessageToFrontend::InstancePlaytimeUpdated {
                    id: instance.id,
                    playtime: instance.playtime(),
                });
            }

            self.restore_mods_folder_if_stopped(instance);
            any_process_alive |= !instance.processes.is_empty() || !instance.closing_processes.is_empty();
        }

        self.quit_coordinator.set_can_quit(!any_process_alive);
    }

    pub async fn login(
        &self,
        credentials: &mut AccountCredentials,
        login_tracker: Option<&ProgressTracker>,
        modal_action: Option<&ModalAction>,
    ) -> Result<(MinecraftProfileResponse, MinecraftAccessToken), LoginError> {
        log::info!("Starting login");

        let mut authenticator = Authenticator::new(self.http_client.clone());

        if let Some(login_tracker) = login_tracker {
            login_tracker.set_total(AUTH_STAGE_COUNT as usize + 1);
            login_tracker.notify();
        }

        let mut last_auth_stage = None;
        let mut allow_backwards = true;
        loop {
            if let Some(modal_action) = modal_action && modal_action.has_requested_cancel() {
                return Err(LoginError::CancelledByUser);
            }

            let stage_with_data = credentials.stage();
            let stage = stage_with_data.stage();

            if let Some(login_tracker) = login_tracker {
                login_tracker.set_count(stage as usize + 1);
                login_tracker.notify();
            }

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

                    let Some(modal_action) = modal_action else {
                        return Err(LoginError::NeedsUserInteraction);
                    };

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
                            if let Some(login_tracker) = login_tracker {
                                login_tracker.set_count(AUTH_STAGE_COUNT as usize + 1);
                                login_tracker.notify();
                            }

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

    pub async fn prelaunch(self: &Arc<Self>, id: InstanceID, modal_action: &ModalAction) -> std::io::Result<()> {
        self.apply_syncing_to_instance(id);
        self.prelaunch_setup_mods(id, modal_action).await
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

    pub fn restore_mods_folder_if_stopped(&self, instance: &mut Instance) {
        if !instance.processes.is_empty() || !instance.closing_processes.is_empty() {
            return;
        }
        if let Some(keepalive) = &instance.launch_keepalive && keepalive.is_alive() {
            return;
        }

        let original_mods_dir = instance.root_path.join("original_mods");
        if !original_mods_dir.exists() {
            instance.set_frozen_mods_folder(false);
            return;
        }

        let mod_dir = instance.content_state[ContentFolder::Mods].path.clone();

        // Copy sinytra connector cache
        if !instance.configuration.get().sandbox {
            let connector = mod_dir.join(".connector");
            if connector.exists() {
                let original_connector = original_mods_dir.join(".connector");
                _ = std::fs::create_dir_all(&original_connector);
                _ = crate::copy_content_recursive(&connector, &original_connector, false, &|_, _| {});
            }
        }

        _ = std::fs::remove_dir_all(&mod_dir);
        if let Err(err) = std::fs::rename(&original_mods_dir, &mod_dir) {
            self.send.send_error(format!("Unable to restore mods directory: {}", err));
            log::error!("Unable to restore mods directory: {err:?}");
        }

        instance.set_frozen_mods_folder(false);
    }

    pub async fn prelaunch_setup_mods(self: &Arc<Self>, id: InstanceID, modal_action: &ModalAction) -> std::io::Result<()> {
        let (loader, minecraft_version, root_dir, dot_minecraft_dir, mods_dir) = if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
            if !instance.processes.is_empty() {
                return Ok(());
            }

            let configuration = instance.configuration.get();
            (configuration.loader, configuration.minecraft_version, instance.root_path.clone(), instance.dot_minecraft_path.clone(), instance.content_state[ContentFolder::Mods].path.clone())
        } else {
            return Ok(());
        };

        if !mods_dir.is_dir() {
            return Ok(());
        }

        let Some(mods) = Instance::load_content(self.clone(), id, ContentFolder::Mods).await else {
            return Ok(());
        };

        let mut mod_copies = Vec::new();

        // Remove .pandora.filename mods (todo: get rid of this)
        if let Ok(read_dir) = std::fs::read_dir(&mods_dir) {
            for entry in read_dir {
                let Ok(entry) = entry else {
                    continue;
                };
                let path = entry.path();
                let Some(file_name) = path.file_name() else {
                    continue;
                };

                let file_name = file_name.as_encoded_bytes();
                if file_name.starts_with(b".pandora.") {
                    log::trace!("Removing temporary mod file {:?}", &file_name);
                    _ = std::fs::remove_file(entry.path());
                }
            }
        }

        self.prelaunch_collect_mods_and_apply_modpack(loader, minecraft_version, &mods, &dot_minecraft_dir, &mods_dir, &mut mod_copies, modal_action).await;

        let sandbox = if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
            instance.set_frozen_mods_folder(true);
            instance.configuration.get().sandbox
        } else {
            true
        };

        let original_mods_dir = root_dir.join("original_mods");
        std::fs::rename(&mods_dir, &original_mods_dir)?;
        if let Err(err) = self.prelaunch_create_mods_dir(mod_copies, &mods_dir, modal_action) {
            _ = std::fs::remove_dir_all(&mods_dir);
            _ = std::fs::rename(&original_mods_dir, &mods_dir);

            if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                instance.set_frozen_mods_folder(true);
            }

            return Err(err);
        }

        // Copy sinytra connector cache
        if !sandbox {
            let original_connector = original_mods_dir.join(".connector");
            if original_connector.exists() {
                let connector = mods_dir.join(".connector");
                _ = std::fs::create_dir_all(&connector);
                _ = crate::copy_content_recursive(&original_connector, &connector, false, &|_, _| {});
            }
        }

        Ok(())
    }

    async fn prelaunch_collect_mods_and_apply_modpack(self: &Arc<Self>, loader: Loader, minecraft_version: Ustr, mods: &[InstanceContentSummary], dot_minecraft_dir: &Path, mod_dir: &Path, mod_copies: &mut Vec<PrelaunchModCopy>, modal_action: &ModalAction) {
        struct ModpackInstall {
            aux_path: Option<PathBuf>,
            files: Vec<ModpackFile>
        }

        let mut modpack_installs = Vec::new();
        let content_library_dir = self.directories.content_library_dir.clone();

        for summary in &*mods {
            if !summary.enabled {
                continue;
            }

            let content_summary = if self.download_modpack_children(summary, loader, minecraft_version, modal_action).await {
                self.mod_metadata_manager.get_path(&summary.path)
            } else {
                summary.content_summary.clone()
            };

            let modpack = match &content_summary.extra {
                ContentType::ModrinthModpack { files, .. } => {
                    Some(files)
                },
                ContentType::CurseforgeModpack { unknown_files, files, .. } => {
                    if !unknown_files.is_empty() {
                        log::warn!("Unable to resolve some curseforge files, mods might be missing from modpack");
                    }
                    Some(files)
                },
                _ => {
                    let path = &summary.path;
                    let Ok(rel_path) = path.strip_prefix(&mod_dir) else {
                        continue;
                    };

                    let extension = path.extension().and_then(OsStr::to_str);
                    let content_library_path = crate::create_content_library_path(&content_library_dir, summary.content_summary.hash, extension);

                    if content_library_path.exists() {
                        mod_copies.push(PrelaunchModCopy {
                            path: rel_path.to_path_buf(),
                            source: PrelaunchModCopySource::FromContentLibrary { hash: summary.content_summary.hash },
                        });
                    } else if let Ok(file) = std::fs::read(&path) {
                        mod_copies.push(PrelaunchModCopy {
                            path: rel_path.to_path_buf(),
                            source: PrelaunchModCopySource::FromBytes { bytes: file.into() },
                        });
                    }

                    None
                }
            };

            if let Some(files) = modpack {
                let filtered_files = files.iter().filter(|dl| {
                    let mut id = None;
                    let mut name = None;

                    if let Some(content_summary) = &dl.summary {
                        id = content_summary.id.as_ref().map(|s| &**s);
                        name = content_summary.name.as_ref().map(|s| &**s);
                    }

                    summary.disabled_children.is_enabled(dl.default_disabled, id, name, dl.path.as_str())
                }).cloned().collect::<Vec<_>>();

                modpack_installs.push(ModpackInstall {
                    aux_path: crate::pandora_aux_path_for_content(&summary),
                    files: filtered_files
                });
            }
        }

        for modpack_install in modpack_installs {
            let content_library_dir = &self.directories.content_library_dir.clone();
            let mut aux: Option<AuxiliaryContentMeta> = if let Some(aux_path) = &modpack_install.aux_path {
                Some(crate::read_json(&aux_path).unwrap_or_default())
            } else {
                None
            };

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

            if !modpack_install.files.is_empty() {
                let tracker = ProgressTracker::new("Copying modpack files".into(), self.send.clone());
                modal_action.trackers.push(tracker.clone());
                tracker.set_total(modpack_install.files.len());
                tracker.notify();

                let mut aux_changed = false;

                for file in modpack_install.files {
                    let Some(rel_path) = file.path() else {
                        continue;
                    };

                    if let ModpackFileSource::Builtin { bytes } = file.source {
                        if let Some(filename) = rel_path.strip_prefix("mods") {
                            mod_copies.push(PrelaunchModCopy {
                                path: filename.to_path(Path::new("")),
                                source: PrelaunchModCopySource::FromBytes {
                                    bytes: bytes.clone()
                                }
                            });
                        } else {
                            let dest_path = rel_path.to_path(&dot_minecraft_dir);

                            if should_override_file(file.path.as_str(), &dest_path, file.hash, &aux) {
                                if let Some(aux) = &mut aux {
                                    let sha1 = hex::encode(file.hash);
                                    aux.applied_overrides.filename_to_hash.insert(file.path.as_str().into(), sha1.into());
                                    aux_changed = true;
                                }

                                _ = crate::write_safe(&dest_path, &bytes);
                            }
                        }
                    } else {
                        if let Some(filename) = rel_path.strip_prefix("mods") {
                            mod_copies.push(PrelaunchModCopy {
                                path: filename.to_path(Path::new("")),
                                source: PrelaunchModCopySource::FromContentLibrary {
                                    hash: file.hash
                                }
                            });
                        } else {
                            let dest_path = rel_path.to_path(&dot_minecraft_dir);

                            let content_path = crate::create_content_library_path(content_library_dir, file.hash, rel_path.extension());

                            if should_override_file(file.path.as_str(), &dest_path, file.hash, &aux) {
                                if let Some(aux) = &mut aux {
                                    let sha1 = hex::encode(file.hash);
                                    aux.applied_overrides.filename_to_hash.insert(file.path.as_str().into(), sha1.into());
                                    aux_changed = true;
                                }

                                let _ = std::fs::create_dir_all(dest_path.parent().unwrap());
                                let _ = std::fs::copy(content_path, dest_path);
                            }
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

                tracker.set_finished(ProgressTrackerFinishType::Normal);
                tracker.notify();
            }
        }
    }

    fn prelaunch_create_mods_dir(&self, mod_copies: Vec<PrelaunchModCopy>, mods_dir: &Path, modal_action: &ModalAction) -> std::io::Result<()> {
        let tracker = ProgressTracker::new("Copying immutable mods directory".into(), self.send.clone());
        modal_action.trackers.push(tracker.clone());

        tracker.set_total(mod_copies.len());
        tracker.notify();

        let content_library_dir = &self.directories.content_library_dir.clone();

        for mod_copy in mod_copies {
            let target_path = mods_dir.join(&mod_copy.path);
            if let Some(parent) = target_path.parent() {
                _ = std::fs::create_dir_all(parent);
            }
            match mod_copy.source {
                PrelaunchModCopySource::FromContentLibrary { hash } => {
                    let extension = mod_copy.path.extension().and_then(OsStr::to_str);
                    let path = crate::create_content_library_path(&content_library_dir, hash, extension);
                    std::fs::copy(path, target_path)?;
                },
                PrelaunchModCopySource::FromBytes { bytes } => {
                    std::fs::write(target_path, &bytes)?;
                },
            }
            tracker.add_count(1);
            tracker.notify();
        }

        tracker.set_finished(ProgressTrackerFinishType::Normal);
        Ok(())
    }

    pub async fn download_modpack_children(self: &Arc<Self>, summary: &InstanceContentSummary, loader: Loader, minecraft_version: Ustr, modal_action: &ModalAction) -> bool {
        let mut curseforge_file_ids = Vec::new();

        let (files, fallback_source) = if let ContentType::ModrinthModpack { files, .. } = &summary.content_summary.extra {
            (files.to_vec(), ContentSource::ModrinthUnknown)
        } else if let ContentType::CurseforgeModpack { unknown_files, files, .. } = &summary.content_summary.extra {
            for unknown_file in unknown_files.iter() {
                curseforge_file_ids.push(unknown_file.file_id);
            }

            (files.to_vec(), ContentSource::Manual)
        } else {
            return false;
        };

        let mut content_install_files = Vec::new();

        for file in files.iter() {
            if let Some(summary) = &file.summary && summary.hash == file.hash {
                continue;
            }

            match &file.source {
                ModpackFileSource::DownloadUrl { url, size } => {
                    content_install_files.push(ContentInstallFile {
                        replace_old: None,
                        path: ContentInstallPath::ModpackFilePath(file.path.clone()),
                        download: ContentDownload::Url {
                            url: url.clone(),
                            sha1: file.hash,
                            size: *size,
                        },
                        content_source: fallback_source.clone(),
                        reason: ContentInstallReason::Modpack,
                    });
                },
                ModpackFileSource::DownloadCurseforge { file_id } => {
                    if file.disabled_third_party_downloads {
                        continue;
                    }

                    curseforge_file_ids.push(*file_id);
                },
                ModpackFileSource::Builtin { .. } => {},
            }
        }

        if !curseforge_file_ids.is_empty() {
            let tracker = ProgressTracker::new("Requesting download URLs from CurseForge".into(), self.send.clone());
            modal_action.trackers.push(tracker.clone());
            tracker.set_total(1);
            tracker.notify();

            let files_result = self.meta.fetch(&CurseforgeGetFilesMetadataItem(&CurseforgeGetFilesRequest {
                file_ids: curseforge_file_ids,
            })).await;

            tracker.set_count(1);
            tracker.set_finished(ProgressTrackerFinishType::from_err(files_result.is_err()));
            tracker.notify();

            if let Ok(files) = files_result {
                for file in files.data.iter() {
                    let sha1 = file.hashes.iter()
                        .find(|hash| hash.algo == 1).map(|hash| &hash.value);
                    let Some(sha1) = sha1 else {
                        continue;
                    };

                    let mut hash = [0u8; 20];
                    let Ok(_) = hex::decode_to_slice(&**sha1, &mut hash) else {
                        log::warn!("File {} has invalid sha1: {}", file.file_name, sha1);
                        continue;
                    };

                    self.mod_metadata_manager.set_cached_curseforge_info(file.id, CachedCurseforgeFileInfo {
                        hash,
                        filename: file.file_name.clone(),
                        disabled_third_party_downloads: file.download_url.is_none()
                    });

                    let Some(filename) = SafePath::new(&file.file_name) else {
                        log::warn!("Skipping file because of invalid filename: {}", file.file_name);
                        continue;
                    };

                    if let Some(download_url) = &file.download_url {
                        content_install_files.push(ContentInstallFile {
                            replace_old: None,
                            path: ContentInstallPath::ModpackFilePath(ModpackFilePath::Filename(filename)),
                            download: ContentDownload::Url {
                                url: download_url.clone(),
                                sha1: hash,
                                size: file.file_length as usize,
                            },
                            content_source: ContentSource::CurseforgeProject { project_id: file.mod_id },
                            reason: ContentInstallReason::Modpack,
                        });
                    }
                }
            }
        }

        if !content_install_files.is_empty() {
            let content_install = ContentInstall {
                target: bridge::install::InstallTarget::Library,
                loader,
                minecraft_version,
                files: content_install_files.into(),
            };

            self.install_content(content_install, modal_action.clone()).await;
            true
        } else {
            false
        }
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

    pub async fn rename_instance(self: &Arc<Self>, id: InstanceID, name: &str) {
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

        if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
            if cfg!(windows) {
                self.file_watching.write().unwatch_subdirectories_of_instance(id);
                instance.mark_all_dirty(self, false);
            }

            let new_instance_dir = self.directories.instances_dir.join(name);
            if let Err(err) = std::fs::rename(&instance.root_path, new_instance_dir) {
                self.send.send_error(format!("Unable to rename instance folder: {}", err));
            }
        }

    }

    pub async fn get_login_info(&self, modal_action: &ModalAction, instance_account: Option<Uuid>) -> Option<MinecraftLoginInfo> {
        let selected_account_with_name = {
            let mut account_info = self.account_info.write();
            let account_info = account_info.get();

            if let Some(uuid) = instance_account.or(account_info.selected_account) {
                if let Some(account) = account_info.accounts.get(&uuid) {
                    if account.offline {
                        return Some(MinecraftLoginInfo {
                            uuid,
                            username: account.username.clone(),
                            access_token: None
                        })
                    }
                    Some((uuid, account.username.clone()))
                } else {
                    None
                }
            } else {
                None
            }
        };

        let selected_account = selected_account_with_name.as_ref().map(|v| v.0);
        let Some((profile, access_token)) = self.login_flow(modal_action, selected_account).await else {
            if let Some((uuid, username)) = selected_account_with_name {
                self.send.send_error("Unable to log into Minecraft account, using offline mode...");
                return Some(MinecraftLoginInfo {
                    uuid,
                    username,
                    access_token: None,
                })
            }

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
            return;
        };

        let canonical: Arc<Path> = if canonical == &*path {
            path.clone()
        } else {
            let is_just_long_path_prefixed = if cfg!(windows) {
                let canonical_bytes = canonical.as_os_str().as_encoded_bytes();
                let path_bytes = path.as_os_str().as_encoded_bytes();
                canonical_bytes.len() == path_bytes.len()+4
                    && &canonical_bytes[..4] == b"\\\\?\\"
                    && &canonical_bytes[4..] == path_bytes
            } else {
                false
            };
            if is_just_long_path_prefixed {
                path.clone()
            } else {
                canonical.into()
            }
        };

        if let Some(old_path) = self.watch_target_to_path.get(&target) && old_path == &path {
            let old_canonical = self.symlink_link_to_src.get(old_path).cloned().unwrap_or(old_path.clone());
            if old_canonical == canonical {
                return;
            }
        }

        if path == canonical {
            log::debug!("Watching {:?} as {:?}", path, target);
        } else {
            log::debug!("Watching {:?} (real path {:?}) as {:?}", path, canonical, target);
        }

        if let Err(err) = self.watcher.watch(&path, notify::RecursiveMode::NonRecursive) {
            log::error!("Unable to watch filesystem: {:?}", err);
            return;
        }

        if let Some(old_path) = self.watch_target_to_path.get(&target) {
            self.remove(&old_path.clone());
        }

        self.watching.insert(path.clone(), target);
        self.watch_target_to_path.insert(target, path.clone());

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
        if let Some(target) = self.watching.remove(path) {
            self.watch_target_to_path.remove(&target);
            Some(target)
        } else {
            None
        }
    }

    pub fn unwatch_subdirectories_of_instance(&mut self, id: InstanceID) {
        let targets = [
            WatchTarget::InstanceDotMinecraftDir { id },
            WatchTarget::InstanceWorldDir { id },
            WatchTarget::InstanceSavesDir { id },
        ];
        let content_folder_targets = ContentFolder::iter().map(|folder| {
            WatchTarget::InstanceContentDir { id, folder }
        });
        for target in targets.into_iter().chain(content_folder_targets) {
            if let Some(path) = self.watch_target_to_path.remove(&target) {
                self.remove(&path);
                _ = self.watcher.unwatch(&path);
            };
        }
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
    #[error("Needs user interaction")]
    NeedsUserInteraction,
    #[error("Cancelled by user")]
    CancelledByUser,
}

struct PrelaunchModCopy {
    path: PathBuf,
    source: PrelaunchModCopySource
}

enum PrelaunchModCopySource {
    FromContentLibrary {
        hash: [u8; 20]
    },
    FromBytes {
        bytes: Arc<[u8]>,
    }
}
