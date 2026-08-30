use std::{
    collections::{HashMap, VecDeque}, fmt::Display, path::Path, sync::Arc, time::{Duration, Instant}
};

use bridge::notify_signal::{KeepAliveNotifySignal, KeepAliveNotifySignalHandle};
use parking_lot::Mutex;
use reqwest::StatusCode;
use schema::{
    assets_index::AssetsIndex,
    curseforge::{
        CurseforgeChangelogRequest, CurseforgeChangelogResult, CurseforgeFingerprintRequest, CurseforgeFingerprintResponse, CurseforgeGetFilesRequest,
        CurseforgeGetModFilesRequest, CurseforgeGetModFilesResult, CurseforgeSearchRequest,
        CurseforgeSearchResult
    },
    fabric_launch::FabricLaunch, fabric_loader_manifest::FabricLoaderManifest,
    forge::{ForgeMavenManifest, NeoforgeMavenManifest}, java_runtime_component::JavaRuntimeComponentManifest,
    java_runtimes::JavaRuntimes,
    modrinth::{
        ModrinthChangelogRequest, ModrinthChangelogResult, ModrinthProjectRequest,
        ModrinthProjectResult, ModrinthProjectVersion, ModrinthProjectVersionsRequest,
        ModrinthProjectVersionsResult, ModrinthProjectsRequest, ModrinthProjectsResponse,
        ModrinthSearchRequest, ModrinthSearchResult, ModrinthVersionFileUpdateResult,
        ModrinthVersionsFromHashesRequest, ModrinthVersionsFromHashesResponse
    },
    version::MinecraftVersion, version_manifest::MinecraftVersionManifest
};
use serde::Deserialize;
use sha1::{Digest, Sha1};
use tokio::task::JoinHandle;
use ustr::Ustr;

use crate::metadata::items::{MetadataItem, ModrinthV3VersionUpdateMetadataItem, ModrinthVersionUpdateMetadataItem};

pub struct MetaState<T> {
    keep_alive: Option<KeepAliveNotifySignalHandle>,
    load_state: MetaLoadState<T>,
    failure_count: usize,
}

impl <T> Default for MetaState<T> {
    fn default() -> Self {
        Self {
            keep_alive: None,
            load_state: MetaLoadState::Unloaded,
            failure_count: 0
        }
    }
}

impl <T> MetaState<T> {
    pub fn should_reload(&self, force: bool) -> bool {
        match self.load_state {
            MetaLoadState::Unloaded => true,
            MetaLoadState::Pending(_) => false,
            MetaLoadState::PendingOther(_) => false,
            MetaLoadState::Loaded(_) | MetaLoadState::Error(_) => {
                force || self.keep_alive.as_ref().map(|h| !h.is_alive()).unwrap_or(false)
            },
        }
    }
}

pub type MetaStateWrapper<T> = Arc<Mutex<MetaState<T>>>;

#[derive(Default)]
pub struct MetadataManagerStates {
    pub(super) minecraft_version_manifest: MetaStateWrapper<MinecraftVersionManifest>,
    pub(super) mojang_java_runtimes: MetaStateWrapper<JavaRuntimes>,
    pub(super) fabric_loader_manifest: MetaStateWrapper<FabricLoaderManifest>,
    pub(super) neoforge_installer_maven_manifest: MetaStateWrapper<NeoforgeMavenManifest>,
    pub(super) forge_installer_maven_manifest: MetaStateWrapper<ForgeMavenManifest>,
    pub(super) fabric_launch: HashMap<(Ustr, Ustr), MetaStateWrapper<FabricLaunch>>,
    pub(super) version_info: HashMap<Ustr, MetaStateWrapper<MinecraftVersion>>,
    pub(super) assets_index: HashMap<Ustr, MetaStateWrapper<AssetsIndex>>,
    pub(super) java_runtime_manifests: HashMap<Ustr, MetaStateWrapper<JavaRuntimeComponentManifest>>,
    pub(super) modrinth_search: HashMap<ModrinthSearchRequest, MetaStateWrapper<ModrinthSearchResult>>,
    pub(super) modrinth_project_versions: HashMap<ModrinthProjectVersionsRequest, MetaStateWrapper<ModrinthProjectVersionsResult>>,
    pub(super) modrinth_project: HashMap<ModrinthProjectRequest, MetaStateWrapper<ModrinthProjectResult>>,
    pub(super) modrinth_projects: HashMap<ModrinthProjectsRequest, MetaStateWrapper<ModrinthProjectsResponse>>,
    pub(super) modrinth_versions: HashMap<Arc<str>, MetaStateWrapper<ModrinthProjectVersion>>,
    pub(super) modrinth_changelogs: HashMap<ModrinthChangelogRequest, MetaStateWrapper<ModrinthChangelogResult>>,
    pub(super) modrinth_version_v2_updates: HashMap<ModrinthVersionUpdateMetadataItem, MetaStateWrapper<ModrinthVersionFileUpdateResult>>,
    pub(super) modrinth_version_v3_updates: HashMap<ModrinthV3VersionUpdateMetadataItem, MetaStateWrapper<ModrinthVersionFileUpdateResult>>,
    pub(super) modrinth_versions_from_hashes: HashMap<ModrinthVersionsFromHashesRequest, MetaStateWrapper<ModrinthVersionsFromHashesResponse>>,
    pub(super) curseforge_search: HashMap<CurseforgeSearchRequest, MetaStateWrapper<CurseforgeSearchResult>>,
    pub(super) curseforge_get_mod_files: HashMap<CurseforgeGetModFilesRequest, MetaStateWrapper<CurseforgeGetModFilesResult>>,
    pub(super) curseforge_get_files: HashMap<CurseforgeGetFilesRequest, MetaStateWrapper<CurseforgeGetModFilesResult>>,
    pub(super) curseforge_changelogs: HashMap<CurseforgeChangelogRequest, MetaStateWrapper<CurseforgeChangelogResult>>,
    pub(super) curseforge_fingerprints: HashMap<CurseforgeFingerprintRequest, MetaStateWrapper<CurseforgeFingerprintResponse>>,
}

#[derive(Clone, Copy, enum_map::Enum)]
enum ExpirationDuration {
    Success,
    RetryError1,
    RetryError2,
    RetryError3,
    RetryError4,
}

impl ExpirationDuration {
    pub fn duration(self) -> Duration {
        match self {
            ExpirationDuration::Success => Duration::from_secs(5 * 60),
            ExpirationDuration::RetryError1 => Duration::from_secs(1),
            ExpirationDuration::RetryError2 => Duration::from_secs(3),
            ExpirationDuration::RetryError3 => Duration::from_secs(9),
            ExpirationDuration::RetryError4 => Duration::from_secs(27),
        }
    }
}

pub struct MetadataManager {
    states: Mutex<MetadataManagerStates>,

    pub(super) metadata_cache: Arc<Path>,
    pub(super) version_manifest_cache: Arc<Path>,
    pub(super) mojang_java_runtimes_cache: Arc<Path>,
    pub(super) fabric_loader_manifest_cache: Arc<Path>,
    pub(super) neoforge_installer_maven_cache: Arc<Path>,
    pub(super) forge_installer_maven_cache: Arc<Path>,

    expiring: enum_map::EnumMap<ExpirationDuration, Mutex<VecDeque<(Instant, KeepAliveNotifySignal)>>>,

    http_client: reqwest::Client,
}

#[derive(thiserror::Error, Clone, Debug)]
pub enum MetaLoadError {
    InvalidHash,
    Reqwest(Arc<reqwest::Error>),
    TokioJoin(Arc<tokio::task::JoinError>),
    // Parsing
    SerdeJson(Arc<serde_json::Error>),
    SerdeXml(Arc<serde_xml_rs::Error>),
    // External
    Error(Arc<str>),
    ErrorWithDescription(Arc<str>, Arc<str>),
    NonOK(u16),
}

impl MetaLoadError {
    pub fn should_retry_error(&self) -> bool {
        match self {
            MetaLoadError::Reqwest(_) => true,
            MetaLoadError::NonOK(n) => *n >= 500 && *n <= 599, // retry on server errors
            _ => false,
        }
    }
}

impl Display for MetaLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHash => {
                f.write_str("Data did not match expected hash")
            },
            Self::Reqwest(error) => {
                if let Some(url) = error.url() {
                    if error.is_connect() {
                        return f.write_fmt(format_args!("Unable to connect to {}", url));
                    } else if error.is_timeout() {
                        return f.write_fmt(format_args!("Connection to {} timed out", url));
                    } else if error.is_decode() {
                        return f.write_fmt(format_args!("Unable to decode response from {}", url));
                    } else if error.is_builder() {
                        return f.write_fmt(format_args!("Unexpected error while constructing request to {}", url));
                    }
                } else if error.is_connect() {
                    return f.write_str("Unable to connect");
                } else if error.is_timeout() {
                    return f.write_str("Connection timed out");
                } else if error.is_decode() {
                    return f.write_str("Unable to decode response");
                } else if error.is_builder() {
                    return f.write_str("Unexpected error while constructing request");
                }

                f.debug_tuple("Reqwest").field(error).finish()
            },
            Self::SerdeJson(_) => {
                f.write_str("Json data was missing or malformed")
            }
            Self::SerdeXml(_) => {
                f.write_str("XML data was missing or malformed")
            }
            Self::Error(error) => {
                f.write_fmt(format_args!("{}", *error))
            }
            Self::ErrorWithDescription(error, description) => {
                f.write_fmt(format_args!("{}\nDescription: {}", *error, *description))
            }
            Self::NonOK(status_code) => {
                f.write_fmt(format_args!("Non-OK response: {}", *status_code))
            }
            Self::TokioJoin(error) => f.debug_tuple("TokioJoin").field(error).finish(),
        }
    }
}

impl From<reqwest::Error> for MetaLoadError {
    fn from(error: reqwest::Error) -> Self {
        Self::Reqwest(Arc::new(error))
    }
}

impl From<serde_json::Error> for MetaLoadError {
    fn from(error: serde_json::Error) -> Self {
        Self::SerdeJson(Arc::new(error))
    }
}

impl From<serde_xml_rs::Error> for MetaLoadError {
    fn from(error: serde_xml_rs::Error) -> Self {
        Self::SerdeXml(Arc::new(error))
    }
}

impl From<tokio::task::JoinError> for MetaLoadError {
    fn from(error: tokio::task::JoinError) -> Self {
        Self::TokioJoin(Arc::new(error))
    }
}

#[derive(Default)]
pub enum MetaLoadState<T> {
    #[default]
    Unloaded,
    Pending(JoinHandle<Result<Arc<T>, MetaLoadError>>),
    PendingOther(KeepAliveNotifySignalHandle),
    Loaded(Arc<T>),
    Error(MetaLoadError),
}

impl MetadataManager {
    pub fn new(http_client: reqwest::Client, directory: Arc<Path>) -> Self {
        Self {
            states: Mutex::new(MetadataManagerStates::default()),

            version_manifest_cache: directory.join("version_manifest.json").into(),
            mojang_java_runtimes_cache: directory.join("mojang_java_runtimes.json").into(),
            fabric_loader_manifest_cache: directory.join("fabric_loader_manifest.json").into(),
            neoforge_installer_maven_cache: directory.join("neoforge_installer_maven.xml").into(),
            forge_installer_maven_cache: directory.join("forge_installer_maven.xml").into(),
            metadata_cache: directory,

            expiring: Default::default(),

            http_client,
        }
    }

    pub fn expire(&self) {
        let now = Instant::now();

        for expiring in self.expiring.values() {
            let mut expiring = expiring.lock();
            while let Some((expires_at, _)) = expiring.front() {
                if now > *expires_at {
                    // todo: can we also delete the state entry to free up memory?
                    expiring.pop_front();
                    continue;
                }
                break;
            }
        }
    }

    fn create_expiry_keepalive(&self, duration: ExpirationDuration) -> KeepAliveNotifySignalHandle {
        let keep_alive = KeepAliveNotifySignal::new();
        let handle = keep_alive.create_handle();
        self.expiring[duration].lock().push_back((Instant::now() + duration.duration(), keep_alive));
        handle
    }

    pub fn preload<I: MetadataItem>(&self, item: &I) {
        let wrapper = item.state(&mut *self.states.lock());
        let mut wrapper = wrapper.lock();

        if wrapper.should_reload(false) {
            if item.expires() {
                wrapper.keep_alive = Some(self.create_expiry_keepalive(ExpirationDuration::Success));
            }

            let cache_file = item.cache_file(self);
            wrapper.load_state = Self::inner_start_loading(item, cache_file, &self.http_client);
        }
    }

    pub async fn fetch<I: MetadataItem>(&self, item: &I) -> Result<Arc<<I as MetadataItem>::T>, MetaLoadError> {
        self.fetch_with_keepalive(item, false).await.0
    }

    pub async fn fetch_with_keepalive<I: MetadataItem>(&self, item: &I, mut force_reload: bool) -> (Result<Arc<<I as MetadataItem>::T>, MetaLoadError>, Option<KeepAliveNotifySignalHandle>) {
        loop {
            if let Some(result) = self.fetch_with_keepalive_inner(item, force_reload).await {
                return result;
            } else {
                force_reload = true;
            }
        }
    }

    pub async fn fetch_with_keepalive_inner<I: MetadataItem>(&self, item: &I, mut force_reload: bool) -> Option<(Result<Arc<<I as MetadataItem>::T>, MetaLoadError>, Option<KeepAliveNotifySignalHandle>)> {
        let state = item.state(&mut *self.states.lock());
        enum LoopAction<T> {
            Resolve(KeepAliveNotifySignal, JoinHandle<Result<Arc<T>, MetaLoadError>>),
            Wait(KeepAliveNotifySignalHandle),
        }
        loop {
            // This weird loop_action thing avoids async issues by ensuring the non-send lock goes out of scope
            let loop_action = {
                let mut wrapper = state.lock();

                // Code for testing automatic metadata reloading
                // if matches!(wrapper.load_state, MetaLoadState::Unloaded) && wrapper.failure_count == 0 {
                //     wrapper.load_state = MetaLoadState::Error(MetaLoadError::Error("Initial metadata failure".into()));
                //     wrapper.failure_count += 1;
                //     wrapper.keep_alive = Some(self.create_expiry_keepalive(ExpirationDuration::RetryError1));
                //     return Some((Err(MetaLoadError::Error("Initial metadata failure".into())), wrapper.keep_alive.clone()));
                // }

                if wrapper.should_reload(force_reload) {
                    let cache_file = item.cache_file(self);
                    wrapper.load_state = Self::inner_start_loading(item, cache_file, &self.http_client);
                }
                force_reload = false;

                match &mut wrapper.load_state {
                    MetaLoadState::Unloaded => unreachable!(),
                    MetaLoadState::Pending(_) => {
                        let signal = KeepAliveNotifySignal::new();
                        let pending = std::mem::replace(&mut wrapper.load_state, MetaLoadState::PendingOther(signal.create_handle()));

                        let MetaLoadState::Pending(join_handle) = pending else {
                            unreachable!();
                        };
                        LoopAction::Resolve(signal, join_handle)
                    },
                    MetaLoadState::PendingOther(signal) => {
                        LoopAction::Wait(signal.clone())
                    },
                    MetaLoadState::Loaded(value) => {
                        return Some((Ok(Arc::clone(value)), wrapper.keep_alive.clone()));
                    },
                    MetaLoadState::Error(meta_load_error) => {
                        return Some((Err(meta_load_error.clone()), wrapper.keep_alive.clone()));
                    },
                }
            };

            match loop_action {
                LoopAction::Resolve(signal, join_handle) => {
                    let result = join_handle.await.map_err(MetaLoadError::from).flatten();
                    let mut wrapper = state.lock();

                    scopeguard::defer! {
                        drop(signal); // Signal other fetches waiting on us that they can try acquire the lock
                    }

                    match result {
                        Ok(value) => {
                            wrapper.load_state = MetaLoadState::Loaded(Arc::clone(&value));
                            wrapper.failure_count = 0;
                            if item.expires() {
                                wrapper.keep_alive = Some(self.create_expiry_keepalive(ExpirationDuration::Success));
                            }
                            return Some((Ok(value), wrapper.keep_alive.clone()));
                        },
                        Err(error) => {
                            wrapper.load_state = MetaLoadState::Error(error.clone());
                            if error.should_retry_error() {
                                wrapper.failure_count += 1;
                                if wrapper.failure_count == 1 {
                                    return None; // If first failure, immediately retry
                                } else if wrapper.failure_count == 2 {
                                    wrapper.keep_alive = Some(self.create_expiry_keepalive(ExpirationDuration::RetryError1));
                                } else if wrapper.failure_count == 3 {
                                    wrapper.keep_alive = Some(self.create_expiry_keepalive(ExpirationDuration::RetryError2));
                                } else if wrapper.failure_count == 4 {
                                    wrapper.keep_alive = Some(self.create_expiry_keepalive(ExpirationDuration::RetryError3));
                                } else {
                                    wrapper.keep_alive = Some(self.create_expiry_keepalive(ExpirationDuration::RetryError4));
                                }
                            } else {
                                wrapper.failure_count = 0;
                                wrapper.keep_alive = Some(self.create_expiry_keepalive(ExpirationDuration::Success));
                            };
                            return Some((Err(error), wrapper.keep_alive.clone()));
                        },
                    }
                },
                LoopAction::Wait(signal) => {
                    signal.await_notification().await;
                },
            }

        }
    }

    #[must_use]
    fn inner_start_loading<I: MetadataItem>(
        item: &I,
        cache_file: Option<impl AsRef<Path> + Send + Sync + 'static>,
        http_client: &reqwest::Client,
    ) -> MetaLoadState<I::T> {
        log::debug!("Loading metadata {:?}", item);

        let request = item.request(http_client);
        let expected_hash = item.data_hash().and_then(|sha1| {
            let mut expected_hash = [0u8; 20];
            hex::decode_to_slice(sha1.as_str(), &mut expected_hash).ok()?;
            Some(expected_hash)
        });
        let join_handle = tokio::task::spawn(async move {
            let mut file_fallback = None;

            if let Some(cache_file) = &cache_file {
                let cache_file = cache_file.as_ref().to_owned();
                let meta = tokio::task::spawn_blocking(move || {
                    let Ok(file) = std::fs::read(&cache_file) else {
                        return None;
                    };

                    let correct_hash = if let Some(expected_hash) = &expected_hash {
                        let mut hasher = Sha1::new();
                        hasher.update(&file);
                        let actual_hash = hasher.finalize();

                        expected_hash == &*actual_hash
                    } else {
                        true
                    };

                    if !correct_hash {
                        log::info!("Sha1 mismatch for {:?}, downloading file again...", cache_file);
                        return None;
                    }

                    let result = I::deserialize(&file);
                    match result {
                        Ok(meta) => {
                            Some(meta)
                        },
                        Err(error) => {
                            log::warn!("Error parsing cached metadata file for {:?}, downloading file again... {}", cache_file, error);
                            None
                        },
                    }
                }).await.unwrap();
                if let Some(meta) = meta {
                    if expected_hash.is_some() {
                        return Ok(Arc::new(meta));
                    } else {
                        file_fallback = Some(Arc::new(meta));
                    }
                }
            }

            let mut result: Result<Arc<I::T>, MetaLoadError> = async move {
                let response = request.send().await?;

                let status = response.status();
                if status != StatusCode::OK {
                    if status == StatusCode::BAD_REQUEST {
                        if let Ok(bytes) = response.bytes().await {
                            #[derive(Deserialize)]
                            struct ErrorMessages {
                                error: Arc<str>,
                                description: Option<Arc<str>>,
                            }
                            if let Ok(error_messages) = serde_json::from_slice::<ErrorMessages>(&bytes) {
                                if let Some(description) = error_messages.description {
                                    return Err(MetaLoadError::ErrorWithDescription(error_messages.error, description));
                                } else {
                                    return Err(MetaLoadError::Error(error_messages.error));
                                }
                            }
                        }
                    }

                    return Err(MetaLoadError::NonOK(status.as_u16()));
                }

                let bytes = response.bytes().await?;
                let bytes = I::post_process_download(&bytes)?;

                // We try to decode before checking the hash because it's a more
                // useful error message to know that the content is invalid
                let meta: I::T = I::deserialize(&bytes)?;

                let correct_hash = if let Some(expected_hash) = &expected_hash {
                    let mut hasher = Sha1::new();
                    hasher.update(&bytes);
                    let actual_hash = hasher.finalize();

                    expected_hash == &*actual_hash
                } else {
                    true
                };

                if !correct_hash {
                    return Err(MetaLoadError::InvalidHash);
                }

                if let Some(cache_file) = &cache_file {
                    if let Some(parent) = cache_file.as_ref().parent() {
                        let _ = tokio::fs::create_dir_all(parent).await;
                    }
                    let _ = tokio::fs::write(cache_file, bytes).await;
                }

                Ok(Arc::new(meta))
            }
            .await;

            if let Err(error) = &result {
                if let Some(file_fallback) = file_fallback {
                    log::warn!(
                        "Error while fetching metadata {:?}, using file fallback: {error:?}",
                        std::any::type_name::<I::T>()
                    );
                    result = Ok(file_fallback);
                } else {
                    log::error!("Error while fetching metadata {:?}: {error:?}", std::any::type_name::<I::T>());
                }
            }

            result
        });

        MetaLoadState::Pending(join_handle)
    }
}
