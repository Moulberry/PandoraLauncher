use std::{collections::HashMap, sync::{Arc, atomic::AtomicBool}};

use bridge::{handle::BackendHandle, message::MessageToBackend, meta::{MetadataRequest, MetadataResult}, notify_signal::KeepAliveNotifySignalHandle};
use gpui::{prelude::*, *};
use schema::{curseforge::{CurseforgeChangelogResult, CurseforgeGetModFilesResult, CurseforgeSearchResult}, fabric_loader_manifest::FabricLoaderManifest, forge::{ForgeMavenManifest, NeoforgeMavenManifest}, modrinth::{ModrinthChangelogResult, ModrinthProjectResult, ModrinthProjectVersionsResult, ModrinthSearchResult}, version_manifest::MinecraftVersionManifest};

#[derive(Debug)]
pub enum FrontendMetadataState {
    Loading,
    Loaded {
        result: Result<MetadataResult, Arc<str>>,
        alive: Option<KeepAliveNotifySignalHandle>,
        can_send_reload: AtomicBool
    },
}

pub enum FrontendMetadataResult<'a, T> {
    Loading,
    Loaded(&'a T),
    Error(SharedString, Option<KeepAliveNotifySignalHandle>),
}

impl <'a, T> FrontendMetadataResult<'a, T> {
    pub fn as_typeless(self) -> TypelessFrontendMetadataResult {
        match self {
            FrontendMetadataResult::Loading => TypelessFrontendMetadataResult::Loading,
            FrontendMetadataResult::Loaded(_) => TypelessFrontendMetadataResult::Loaded,
            FrontendMetadataResult::Error(error, alive) => TypelessFrontendMetadataResult::Error(error, alive),
        }
    }
}

pub enum TypelessFrontendMetadataResult {
    Loading,
    Loaded,
    Error(SharedString, Option<KeepAliveNotifySignalHandle>),
}

pub struct FrontendMetadata {
    pub data: HashMap<MetadataRequest, Entity<FrontendMetadataState>>,
    pub backend_handle: BackendHandle,
}

impl FrontendMetadata {
    pub fn new(backend_handle: BackendHandle) -> Self {
        Self {
            data: HashMap::new(),
            backend_handle,
        }
    }

    pub fn force_reload(entity: &Entity<Self>, request: MetadataRequest, cx: &mut App) -> Entity<FrontendMetadataState> {
        entity.update(cx, |this, cx| {
            if let Some(existing) = this.data.get(&request) {
                this.backend_handle.send(MessageToBackend::RequestMetadata {
                    request: request.clone(),
                    force_reload: true,
                });
                return existing.clone();
            }

            let loading = cx.new(|_| FrontendMetadataState::Loading);
            this.backend_handle.send(MessageToBackend::RequestMetadata {
                request: request.clone(),
                force_reload: true,
            });
            this.data.insert(request, loading.clone());
            loading
        })
    }

    pub fn request(entity: &Entity<Self>, request: MetadataRequest, cx: &mut App) -> Entity<FrontendMetadataState> {
        entity.update(cx, |this, cx| {
            if let Some(existing) = this.data.get(&request) {
                let mut is_reloading_error = false;
                if let FrontendMetadataState::Loaded { result, alive, can_send_reload } = existing.read(cx) {
                    if alive.as_ref().map(|k| !k.is_alive()).unwrap_or(false)
                        && can_send_reload.swap(false, std::sync::atomic::Ordering::Relaxed)
                    {
                        this.backend_handle.send(MessageToBackend::RequestMetadata {
                            request: request.clone(),
                            force_reload: false,
                        });
                        is_reloading_error = result.is_err();
                    }
                }
                if is_reloading_error {
                    existing.update(cx, |value, cx| {
                        *value = FrontendMetadataState::Loading;
                        cx.notify();
                    });
                }
                return existing.clone();
            }

            let loading = cx.new(|_| FrontendMetadataState::Loading);
            this.backend_handle.send(MessageToBackend::RequestMetadata {
                request: request.clone(),
                force_reload: false,
            });
            this.data.insert(request, loading.clone());
            loading
        })
    }

    pub fn set(
        entity: &Entity<Self>,
        request: MetadataRequest,
        result: Result<MetadataResult, Arc<str>>,
        alive: Option<KeepAliveNotifySignalHandle>,
        cx: &mut App,
    ) {
        entity.update(cx, |this, cx| {
            let loaded = FrontendMetadataState::Loaded { result, alive, can_send_reload: AtomicBool::new(true) };
            if let Some(existing) = this.data.get(&request) {
                existing.update(cx, |value, cx| {
                    *value = loaded;
                    cx.notify();
                });
            } else {
                this.data.insert(request, cx.new(|_| loaded));
            }
        });
    }
}

pub trait AsMetadataResult<T> {
    fn result(&self) -> FrontendMetadataResult<'_, T>;
}

macro_rules! define_as_metadata_result {
    ($t:ident) => {
        impl AsMetadataResult<$t> for FrontendMetadataState {
            fn result(&self) -> FrontendMetadataResult<'_, $t> {
                match self {
                    FrontendMetadataState::Loading => FrontendMetadataResult::Loading,
                    FrontendMetadataState::Loaded { result, alive, .. } => {
                        match result {
                            Ok(MetadataResult::$t(result)) => FrontendMetadataResult::Loaded(&*result),
                            Ok(_) => FrontendMetadataResult::Error(t::system::metadata_error().into(), alive.clone()),
                            Err(error) => FrontendMetadataResult::Error(SharedString::new(error.clone()), alive.clone()),
                        }
                    },
                }
            }
        }
    };
}

define_as_metadata_result!(MinecraftVersionManifest);
define_as_metadata_result!(ModrinthSearchResult);
define_as_metadata_result!(ModrinthProjectVersionsResult);
define_as_metadata_result!(FabricLoaderManifest);
define_as_metadata_result!(ForgeMavenManifest);
define_as_metadata_result!(NeoforgeMavenManifest);
define_as_metadata_result!(ModrinthProjectResult);
define_as_metadata_result!(ModrinthChangelogResult);
define_as_metadata_result!(CurseforgeSearchResult);
define_as_metadata_result!(CurseforgeGetModFilesResult);
define_as_metadata_result!(CurseforgeChangelogResult);
