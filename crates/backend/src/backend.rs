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
use bridge::{
    handle::{BackendHandle, BackendReceiver, FrontendHandle}, install::{ContentDownload, ContentInstall, ContentInstallFile, ContentInstallPath}, instance::{InstanceID, InstanceModSummary, InstanceServerSummary, InstanceWorldSummary, LoaderSpecificModSummary}, message::MessageToFrontend, modal_action::{ModalAction, ModalActionVisitUrl, ProgressTracker, ProgressTrackerFinishType}, safe_path::SafePath
};
use parking_lot::RwLock;
use reqwest::{StatusCode, redirect::Policy};
use rustc_hash::{FxHashMap, FxHashSet};
use schema::{backend_config::BackendConfig, instance::InstanceConfiguration, loader::Loader, modrinth::ModrinthSideRequirement};
use sha1::{Digest, Sha1};
use tokio::sync::{mpsc::Receiver, OnceCell};
use ustr::Ustr;
use uuid::Uuid;

use crate::{
    account::BackendAccountInfo, directories::LauncherDirectories, id_slab::IdSlab, instance::Instance, launch::Launcher, metadata::{items::MinecraftVersionManifestMetadataItem, manager::MetadataManager}, mod_metadata::ModMetadataManager, persistent::Persistent
};

pub fn start(launcher_dir: PathBuf, send: FrontendHandle, self_handle: BackendHandle, recv: BackendReceiver) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime");

    let http_client = reqwest::ClientBuilder::new()
        // .connect_timeout(Duration::from_secs(5))
        .redirect(Policy::none())
        .use_rustls_tls()
        .user_agent("PandoraLauncher/0.1.0 (https://github.com/Moulberry/PandoraLauncher)")
        .build()
        .unwrap();

    let redirecting_http_client = reqwest::ClientBuilder::new()
        .use_rustls_tls()
        .user_agent("PandoraLauncher/0.1.0 (https://github.com/Moulberry/PandoraLauncher)")
        .build()
        .unwrap();

    let directories = Arc::new(LauncherDirectories::new(launcher_dir));

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
        instance_by_path: HashMap::new(),
        instances_generation: 0,
        reload_mods_immediately: FxHashSet::default(),
    };

    let mut state_file_watching = BackendStateFileWatching {
        watcher,
        watching: HashMap::new(),
    };

    // Create initial directories
    let _ = std::fs::create_dir_all(&directories.instances_dir);
    state_file_watching.try_watch_filesystem(&directories.root_launcher_dir, WatchTarget::RootDir);

    // Load accounts
    let account_info = Persistent::load(directories.accounts_json.clone());

    // Load config
    let config = Persistent::load(directories.config_json.clone());

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
    InstanceModsDir { id: InstanceID },
}

pub struct BackendStateInstances {
    pub instances: IdSlab<Instance>,
    pub instance_by_path: HashMap<PathBuf, InstanceID>,
    pub instances_generation: usize,
    pub reload_mods_immediately: FxHashSet<InstanceID>,
}

pub struct BackendStateFileWatching {
    pub watcher: notify_debouncer_full::Debouncer<notify::RecommendedWatcher, notify_debouncer_full::RecommendedCache>,
    pub watching: HashMap<Arc<Path>, WatchTarget>,
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
        // Pre-fetch version manifest
        self.meta.load(&MinecraftVersionManifestMetadataItem).await;

        self.handle(recv, watcher_rx).await;
    }

    pub async fn load_all_instances(&mut self) {
        let mut paths_with_time = Vec::new();

        self.file_watching.write().try_watch_filesystem(&self.directories.instances_dir, WatchTarget::InstancesDir);
        for entry in std::fs::read_dir(&self.directories.instances_dir).unwrap() {
            let Ok(entry) = entry else {
                eprintln!("Error reading directory in instances folder: {:?}", entry.unwrap_err());
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
                self.watch_filesystem(&path, WatchTarget::InvalidInstanceDir);
            }
        }
    }

    pub fn watch_filesystem(&self, path: &Path, target: WatchTarget) {
        self.file_watching.write().watch_filesystem(path, target, &self.send);
    }

    pub fn remove_instance(&mut self, id: InstanceID) {
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
                if let Some(existing) = instance_state.instance_by_path.get(path)
                    && let Some(existing_instance) = instance_state.instances.remove(*existing)
                {
                    self.send.send(MessageToFrontend::InstanceRemoved { id: existing_instance.id});
                    show_errors = true;
                }

                if show_errors {
                    let error = instance.unwrap_err();
                    self.send.send_error(format!("Unable to load instance from {:?}:\n{}", &path, &error));
                    eprintln!("Error loading instance: {:?}", &error);
                }

                return false;
            };

            if let Some(existing) = instance_state.instance_by_path.get(path)
                && let Some(existing_instance) = instance_state.instances.get_mut(*existing)
            {
                existing_instance.copy_basic_attributes_from(instance);

                let _ = self.send.send(existing_instance.create_modify_message());

                if show_success {
                    self.send.send_info(format!("Instance '{}' updated", existing_instance.name));
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
                dot_minecraft_folder: instance.dot_minecraft_path.clone(),
                configuration: instance.configuration.get().clone(),
                worlds_state: Arc::clone(&instance.worlds_state),
                servers_state: Arc::clone(&instance.servers_state),
                mods_state: Arc::clone(&instance.mods_state),
            };
            self.send.send(message);

            instance_state.instance_by_path.insert(path.to_owned(), instance.id);

            instance.id
        };

        self.watch_filesystem(path, WatchTarget::InstanceDir { id: instance_id });
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
                        eprintln!("Backend receiver has shut down");
                        break;
                    }
                },
                instance_change = watcher_rx.recv() => {
                    if let Some(instance_change) = instance_change {
                        self.handle_filesystem(instance_change).await;
                    } else {
                        eprintln!("Backend filesystem has shut down");
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
                    eprintln!(
                        "Stage {:?} went backwards from {:?} when going backwards isn't allowed. This is most likely a bug with the auth flow!",
                        stage, last_stage
                    );
                    return Err(LoginError::LoginStageErrorBackwards);
                } else if stage == last_stage {
                    eprintln!("Stage {:?} didn't change. This is most likely a bug with the auth flow!", stage);
                    return Err(LoginError::LoginStageErrorDidntChange);
                }
            }
            last_auth_stage = Some(stage);

            match credentials.stage() {
                auth::credentials::AuthStageWithData::Initial => {
                    let pending = authenticator.create_authorization();
                    modal_action.set_visit_url(ModalActionVisitUrl {
                        message: "Login with Microsoft".into(),
                        url: pending.url.as_str().into(),
                        prevent_auto_finish: false,
                    });
                    self.send.send(MessageToFrontend::Refresh);

                    let finished = tokio::select! {
                        finished = serve_redirect::start_server(pending) => finished?,
                        _ = modal_action.request_cancel.cancelled() => {
                            return Err(LoginError::CancelledByUser);
                        }
                    };

                    modal_action.unset_visit_url();
                    self.send.send(MessageToFrontend::Refresh);

                    let msa_tokens = authenticator.finish_authorization(finished).await?;

                    credentials.msa_access = Some(msa_tokens.access);
                    credentials.msa_refresh = msa_tokens.refresh;
                },
                auth::credentials::AuthStageWithData::MsaRefresh(refresh) => {
                    match authenticator.refresh_msa(&refresh).await {
                        Ok(Some(msa_tokens)) => {
                            credentials.msa_access = Some(msa_tokens.access);
                            credentials.msa_refresh = msa_tokens.refresh;
                        },
                        Ok(None) => {
                            if !allow_backwards {
                                return Err(MsaAuthorizationError::InvalidGrant.into());
                            }
                            credentials.msa_refresh = None;
                        },
                        Err(error) => {
                            if !allow_backwards || error.is_connection_error() {
                                return Err(error.into());
                            }
                            if !matches!(error, MsaAuthorizationError::InvalidGrant) {
                                eprintln!("Error using msa refresh to get msa access: {:?}", error);
                            }
                            credentials.msa_refresh = None;
                        },
                    }
                },
                auth::credentials::AuthStageWithData::MsaAccess(access) => {
                    match authenticator.authenticate_xbox(&access).await {
                        Ok(xbl) => {
                            credentials.xbl = Some(xbl);
                        },
                        Err(error) => {
                            if !allow_backwards || error.is_connection_error() {
                                return Err(error.into());
                            }
                            if !matches!(error, XboxAuthenticateError::NonOkHttpStatus(StatusCode::UNAUTHORIZED)) {
                                eprintln!("Error using msa access to get xbl token: {:?}", error);
                            }
                            credentials.msa_access = None;
                        },
                    }
                },
                auth::credentials::AuthStageWithData::XboxLive(xbl) => {
                    match authenticator.obtain_xsts(&xbl).await {
                        Ok(xsts) => {
                            credentials.xsts = Some(xsts);
                        },
                        Err(error) => {
                            if !allow_backwards || error.is_connection_error() {
                                return Err(error.into());
                            }
                            if !matches!(error, XboxAuthenticateError::NonOkHttpStatus(StatusCode::UNAUTHORIZED)) {
                                eprintln!("Error using xbl to get xsts: {:?}", error);
                            }
                            credentials.xbl = None;
                        },
                    }
                },
                auth::credentials::AuthStageWithData::XboxSecure { xsts, userhash } => {
                    match authenticator.authenticate_minecraft(&xsts, &userhash).await {
                        Ok(token) => {
                            credentials.access_token = Some(token);
                        },
                        Err(error) => {
                            if !allow_backwards || error.is_connection_error() {
                                return Err(error.into());
                            }
                            if !matches!(error, XboxAuthenticateError::NonOkHttpStatus(StatusCode::UNAUTHORIZED)) {
                                eprintln!("Error using xsts to get minecraft access token: {:?}", error);
                            }
                            credentials.xsts = None;
                        },
                    }
                },
                auth::credentials::AuthStageWithData::AccessToken(access_token) => {
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
                                eprintln!("Error using access token to get profile: {:?}", error);
                            }
                            credentials.access_token = None;
                        },
                    }
                },
            }
        }
    }

    pub fn update_profile_head(&self, profile: &MinecraftProfileResponse) {
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
            let Ok(response) = http_client.get(&*skin_url).send().await else {
                head_cache.write().insert(skin_url.clone(), HeadCacheEntry::Failed);
                return;
            };
            let Ok(bytes) = response.bytes().await else {
                head_cache.write().insert(skin_url.clone(), HeadCacheEntry::Failed);
                return;
            };
            let Ok(mut image) = image::load_from_memory(&bytes) else {
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
        self.prelaunch_apply_syncing(id);
        self.prelaunch_apply_modpacks(id, modal_action).await
    }

    pub fn prelaunch_apply_syncing(&self, id: InstanceID) {
        let path = if let Some(instance) = self.instance_state.read().instances.get(id) {
            instance.dot_minecraft_path.clone()
        } else {
            return;
        };

        crate::syncing::apply_to_instance(self.config.write().get().sync_targets, &self.directories, path);
    }

    pub async fn prelaunch_apply_modpacks(&self, id: InstanceID, modal_action: &ModalAction) -> Vec<PathBuf> {
        let (loader, minecraft_version, mod_dir) = if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
            let configuration = instance.configuration.get();
            (configuration.loader, configuration.minecraft_version, instance.mods_path.clone())
        } else {
            return Vec::new();
        };

        if loader == Loader::Vanilla {
            return Vec::new();
        }

        let Some(mods) = self.clone().load_instance_mods(id).await else {
            return Vec::new();
        };

        struct HashedDownload {
            sha1: Arc<str>,
            path: Arc<str>,
        }

        struct ModpackInstall {
            hashed_downloads: Vec<HashedDownload>,
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
                    _ = std::fs::remove_file(entry.path());
                }
            }
        }

        let mut modpack_installs = Vec::new();

        for summary in &*mods {
            if !summary.enabled {
                continue;
            }

            if let LoaderSpecificModSummary::ModrinthModpack { downloads, overrides, .. } = &summary.mod_summary.extra {
                let downloads = downloads.clone();

                let filtered_downloads = downloads.iter().filter(|dl| {
                    if let Some(env) = dl.env {
                        if env.client == ModrinthSideRequirement::Unsupported {
                            return false;
                        }
                    }

                    !summary.disabled_children.contains(&*dl.path)
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
                            content_source: schema::content::ContentSource::Modrinth,
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

                    let _ = std::fs::create_dir_all(dest_path.parent().unwrap());
                    let _ = std::fs::copy(path, dest_path);
                }
            }

            if !overrides.is_empty() {
                let tracker = ProgressTracker::new("Copying overrides".into(), self.send.clone());
                modal_action.trackers.push(tracker.clone());

                tracker.set_total(overrides.len());
                tracker.notify();

                let tracker = &tracker;
                let dot_minecraft_path = &dot_minecraft_path;
                let mod_dir = &mod_dir;
                let futures = overrides.iter().map(|(dest_path, file)| async move {
                    let file2 = file.clone();
                    let expected_hash = tokio::task::spawn_blocking(move || {
                        let mut hasher = Sha1::new();
                        hasher.update(&file2);
                        hasher.finalize().into()
                    }).await.unwrap();

                    let path = crate::create_content_library_path(content_library_dir, expected_hash, dest_path.extension());

                    if !path.exists() {
                        let _ = std::fs::create_dir_all(path.parent().unwrap());
                        let _ = tokio::fs::write(&path, file).await;
                    }

                    if dest_path.starts_with("mods") && let Some(extension) = dest_path.extension() && extension == "jar" {
                        if loader_supports_add_mods {
                            return Some(path);
                        } else if let Some(filename) = dest_path.file_name() {
                            let filename = format!(".pandora.{filename}");
                            let hidden_dest_path = mod_dir.join(filename);
                            let _ = std::fs::hard_link(path, hidden_dest_path);
                        }
                    } else {
                        let dest_path = dest_path.to_path(&dot_minecraft_path);

                        let _ = std::fs::create_dir_all(dest_path.parent().unwrap());
                        let _ = tokio::fs::copy(path, dest_path).await;
                    }
                    tracker.add_count(1);
                    tracker.notify();
                    None
                });

                add_mods.extend(futures::future::join_all(futures).await.into_iter().flatten());

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
                if file_watching.watcher.watch(&instance.dot_minecraft_path, notify::RecursiveMode::NonRecursive).is_ok() {
                    file_watching.watching.insert(instance.dot_minecraft_path.clone(), WatchTarget::InstanceDotMinecraftDir {
                        id: instance.id,
                    });
                }
            }
            if !instance.watching_server_dat {
                instance.watching_server_dat = true;
                let server_dat = instance.server_dat_path.clone();
                if file_watching.watcher.watch(&server_dat, notify::RecursiveMode::NonRecursive).is_ok() {
                    file_watching.watching.insert(server_dat.clone(), WatchTarget::ServersDat { id: instance.id });
                }
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

    pub async fn load_instance_mods(self, id: InstanceID) -> Option<Arc<[InstanceModSummary]>> {
        if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
            let mut file_watching = self.file_watching.write();
            if !instance.watching_dot_minecraft {
                instance.watching_dot_minecraft = true;
                if file_watching.watcher.watch(&instance.dot_minecraft_path, notify::RecursiveMode::NonRecursive).is_ok() {
                    file_watching.watching.insert(instance.dot_minecraft_path.clone(), WatchTarget::InstanceDotMinecraftDir {
                        id: instance.id,
                    });
                }
            }
            if !instance.watching_mods_dir {
                instance.watching_mods_dir = true;
                let mods_path = instance.mods_path.clone();
                if file_watching.watcher.watch(&mods_path, notify::RecursiveMode::NonRecursive).is_ok() {
                    file_watching.watching.insert(mods_path.clone(), WatchTarget::InstanceModsDir { id: instance.id });
                }
            }
        }

        let result = Instance::load_mods(self.instance_state.clone(), id, &self.mod_metadata_manager).await;

        if let Some((mods, newly_loaded)) = result.clone() && newly_loaded {
            self.send.send(MessageToFrontend::InstanceModsUpdated {
                id,
                mods: Arc::clone(&mods)
            });
        }

        result.map(|(mods, _)| mods)
    }

    pub async fn load_instance_worlds(self, id: InstanceID) -> Option<Arc<[InstanceWorldSummary]>> {
        if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
            let mut file_watching = self.file_watching.write();
            if !instance.watching_dot_minecraft {
                instance.watching_dot_minecraft = true;
                if file_watching.watcher.watch(&instance.dot_minecraft_path, notify::RecursiveMode::NonRecursive).is_ok() {
                    file_watching.watching.insert(instance.dot_minecraft_path.clone(), WatchTarget::InstanceDotMinecraftDir {
                        id: instance.id,
                    });
                }
            }
            if !instance.watching_saves_dir {
                instance.watching_saves_dir = true;
                let saves = instance.saves_path.clone();
                if file_watching.watcher.watch(&saves, notify::RecursiveMode::NonRecursive).is_ok() {
                    file_watching.watching.insert(saves.clone(), WatchTarget::InstanceSavesDir { id: instance.id });
                }
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
                if file_watching.watcher.watch(&summary.level_path, notify::RecursiveMode::NonRecursive).is_ok() {
                    file_watching.watching.insert(summary.level_path.clone(), WatchTarget::InstanceWorldDir {
                        id,
                    });
                }
            }
        }

        result.map(|(worlds, _)| worlds)
    }

    pub async fn create_instance_sanitized(&self, name: &str, version: &str, loader: Loader) -> Option<PathBuf> {
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

        return self.create_instance(&name, version, loader).await;
    }

    pub async fn create_instance(&self, name: &str, version: &str, loader: Loader) -> Option<PathBuf> {
        if loader == Loader::Unknown {
            self.send.send_warning(format!("Unable to create instance, unknown loader"));
            return None;
        }
        if !crate::is_single_component_path(&name) {
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

        self.watch_filesystem(&self.directories.instances_dir.clone(), WatchTarget::InstancesDir);

        let instance_dir = self.directories.instances_dir.join(name);

        let _ = tokio::fs::create_dir_all(&instance_dir).await;

        let instance_info = InstanceConfiguration {
            minecraft_version: Ustr::from(version),
            loader,
            memory: None,
            jvm_flags: None,
            jvm_binary: None,
        };

        let info_path = instance_dir.join("info_v1.json");
        crate::write_safe(&info_path, serde_json::to_string(&instance_info).unwrap().as_bytes()).unwrap();

        Some(instance_dir.clone())
    }

    pub async fn rename_instance(&self, id: InstanceID, name: &str) {
        if !crate::is_single_component_path(&name) {
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
}

impl BackendStateFileWatching {
    pub fn try_watch_filesystem(&mut self, path: &Path, target: WatchTarget) -> bool {
        if self.watcher.watch(path, notify::RecursiveMode::NonRecursive).is_err() {
            return false;
        }
        self.watching.insert(path.into(), target);
        true
    }

    pub fn watch_filesystem(&mut self, path: &Path, target: WatchTarget, send: &FrontendHandle) {
        if self.watcher.watch(path, notify::RecursiveMode::NonRecursive).is_err() {
            if path.exists() {
                send.send_error(format!("Unable to watch directory {:?}, launcher may be out of sync with files!", path));
            }
            return;
        }
        self.watching.insert(path.into(), target);
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
