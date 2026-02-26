use std::{
    collections::{HashMap, VecDeque}, fmt::Display, path::Path, sync::Arc, time::{Duration, Instant}
};

use bridge::keep_alive::{KeepAlive, KeepAliveHandle};
use reqwest::StatusCode;
use schema::{
    assets_index::AssetsIndex, fabric_launch::FabricLaunch, fabric_loader_manifest::FabricLoaderManifest, forge::{ForgeMavenManifest, NeoforgeMavenManifest}, java_runtime_component::JavaRuntimeComponentManifest, java_runtimes::JavaRuntimes, maven::MavenMetadataXml, modrinth::{ModrinthProjectRequest, ModrinthProjectResult, ModrinthProjectVersion, ModrinthProjectVersionsRequest, ModrinthProjectVersionsResult, ModrinthSearchRequest, ModrinthSearchResult, ModrinthVersionFileUpdateResult}, version::MinecraftVersion, version_manifest::MinecraftVersionManifest
};
use serde::Deserialize;
use sha1::{Digest, Sha1};
use tokio::task::JoinHandle;
use ustr::Ustr;

use crate::metadata::items::MetadataItem;

const DATA_TTL: Duration = Duration::from_secs(5 * 60);

pub(super) type MetaLoadStateWrapper<T> = Arc<tokio::sync::Mutex<(Option<KeepAliveHandle>, MetaLoadState<T>)>>;

#[derive(Default)]
pub struct MetadataManagerStates {
    pub(super) minecraft_version_manifest: MetaLoadStateWrapper<MinecraftVersionManifest>,
    pub(super) mojang_java_runtimes: MetaLoadStateWrapper<JavaRuntimes>,
    pub(super) fabric_loader_manifest: MetaLoadStateWrapper<FabricLoaderManifest>,
    pub(super) neoforge_installer_maven_manifest: MetaLoadStateWrapper<NeoforgeMavenManifest>,
    pub(super) forge_installer_maven_manifest: MetaLoadStateWrapper<ForgeMavenManifest>,
    pub(super) fabric_launch: HashMap<(Ustr, Ustr), MetaLoadStateWrapper<FabricLaunch>>,
    pub(super) version_info: HashMap<Ustr, MetaLoadStateWrapper<MinecraftVersion>>,
    pub(super) assets_index: HashMap<Ustr, MetaLoadStateWrapper<AssetsIndex>>,
    pub(super) java_runtime_manifests: HashMap<Ustr, MetaLoadStateWrapper<JavaRuntimeComponentManifest>>,
    pub(super) modrinth_search: HashMap<ModrinthSearchRequest, MetaLoadStateWrapper<ModrinthSearchResult>>,
    pub(super) modrinth_project_versions: HashMap<ModrinthProjectVersionsRequest, MetaLoadStateWrapper<ModrinthProjectVersionsResult>>,
    pub(super) modrinth_project: HashMap<ModrinthProjectRequest, MetaLoadStateWrapper<ModrinthProjectResult>>,
    pub(super) modrinth_versions: HashMap<Arc<str>, MetaLoadStateWrapper<ModrinthProjectVersion>>,
    pub(super) modrinth_version_updates: HashMap<Arc<str>, MetaLoadStateWrapper<ModrinthVersionFileUpdateResult>>,
}

pub struct MetadataManager {
    states: tokio::sync::Mutex<MetadataManagerStates>,

    pub(super) metadata_cache: Arc<Path>,
    pub(super) version_manifest_cache: Arc<Path>,
    pub(super) mojang_java_runtimes_cache: Arc<Path>,
    pub(super) fabric_loader_manifest_cache: Arc<Path>,
    pub(super) neoforge_installer_maven_cache: Arc<Path>,
    pub(super) forge_installer_maven_cache: Arc<Path>,

    expiring: tokio::sync::Mutex<VecDeque<(Instant, KeepAlive)>>,

    http_client: reqwest::Client,
}

#[derive(thiserror::Error, Clone, Debug)]
pub enum MetaLoadError {
    InvalidHash,
    Reqwest(Arc<reqwest::Error>),
    SerdeJson(Arc<serde_json::Error>),
    SerdeXml(Arc<serde_xml_rs::Error>),
    TokioJoin(Arc<tokio::task::JoinError>),
    Error(Arc<str>),
    ErrorWithDescription(Arc<str>, Arc<str>),
    NonOK(u16),
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
    Loaded(Arc<T>),
    Error(MetaLoadError),
}

impl MetadataManager {
    pub fn new(http_client: reqwest::Client, directory: Arc<Path>) -> Self {
        Self {
            states: tokio::sync::Mutex::new(MetadataManagerStates::default()),

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

    pub async fn expire(&self) {
        let now = Instant::now();

        let mut expiring = self.expiring.lock().await;
        while let Some((expires_at, _)) = expiring.front() {
            if now > *expires_at {
                expiring.pop_front();
                continue;
            }
            return;
        }
    }

    pub async fn load<I: MetadataItem>(&self, item: &I) {
        let wrapper = item.state(&mut *self.states.lock().await);
        let mut wrapper = wrapper.lock().await;

        let is_valid = wrapper.0.as_ref().map(|h| h.is_alive()).unwrap_or(true);
        if !is_valid || matches!(wrapper.1, MetaLoadState::Unloaded) {
            if item.expires() {
                let keep_alive = KeepAlive::new();
                let handle = keep_alive.create_handle();
                wrapper.0 = Some(handle);
                self.expiring.lock().await.push_back((Instant::now() + DATA_TTL, keep_alive));
            }

            let cache_file = item.cache_file(self);
            Self::inner_start_loading(
                &mut wrapper.1,
                item,
                cache_file,
                &self.http_client,
            );
        }
    }

    pub async fn fetch<I: MetadataItem>(&self, item: &I) -> Result<Arc<<I as MetadataItem>::T>, MetaLoadError> {
        self.fetch_with_keepalive(item, false).await.0
    }

    pub async fn fetch_with_keepalive<I: MetadataItem>(&self, item: &I, force_reload: bool) -> (Result<Arc<<I as MetadataItem>::T>, MetaLoadError>, Option<KeepAliveHandle>) {
        let wrapper = item.state(&mut *self.states.lock().await);
        let mut wrapper = wrapper.lock().await;

        let is_valid = wrapper.0.as_ref().map(|h| h.is_alive()).unwrap_or(true);
        if force_reload || !is_valid || matches!(wrapper.1, MetaLoadState::Unloaded) {
            if item.expires() {
                let keep_alive = KeepAlive::new();
                let handle = keep_alive.create_handle();
                wrapper.0 = Some(handle);
                self.expiring.lock().await.push_back((Instant::now() + DATA_TTL, keep_alive));
            }

            let cache_file = item.cache_file(self);
            Self::inner_start_loading(
                &mut wrapper.1,
                item,
                cache_file,
                &self.http_client,
            );
        }

        let valid = wrapper.0.clone();

        match &mut wrapper.1 {
            MetaLoadState::Unloaded => unreachable!(),
            MetaLoadState::Pending(join_handle) => {
                let result = join_handle.await.map_err(MetaLoadError::from).flatten();
                match result {
                    Ok(value) => {
                        wrapper.1 = MetaLoadState::Loaded(Arc::clone(&value));
                        (Ok(value), valid)
                    },
                    Err(error) => {
                        wrapper.1 = MetaLoadState::Error(error.clone());
                        (Err(error), valid)
                    },
                }
            },
            MetaLoadState::Loaded(value) => {
                (Ok(Arc::clone(value)), valid)
            },
            MetaLoadState::Error(meta_load_error) => {
                (Err(meta_load_error.clone()), valid)
            },
        }
    }

    fn inner_start_loading<I: MetadataItem>(
        state: &mut MetaLoadState<I::T>,
        item: &I,
        cache_file: Option<impl AsRef<Path> + Send + Sync + 'static>,
        http_client: &reqwest::Client,
    ) {
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

        *state = MetaLoadState::Pending(join_handle);
    }
}
