use std::{collections::HashMap, sync::Arc};

use crate::ts;
use bridge::{handle::BackendHandle, keep_alive::KeepAliveHandle, message::MessageToBackend, meta::{MetadataRequest, MetadataResult}};
use gpui::{prelude::*, *};
use schema::{fabric_loader_manifest::FabricLoaderManifest, forge::{ForgeMavenManifest, NeoforgeMavenManifest}, maven::MavenMetadataXml, modrinth::{ModrinthProjectResult, ModrinthProjectVersionsResult, ModrinthSearchResult}, version_manifest::MinecraftVersionManifest};

#[derive(Debug)]
pub enum FrontendMetadataState {
    Loading,
    Loaded {
        result: Result<MetadataResult, Arc<str>>,
        keep_alive: Option<KeepAliveHandle>,
    },
}

pub enum FrontendMetadataResult<'a, T> {
    Loading,
    Loaded(&'a T),
    Error(SharedString),
}

impl <'a, T> FrontendMetadataResult<'a, T> {
    pub fn as_typeless(self) -> TypelessFrontendMetadataResult {
        match self {
            FrontendMetadataResult::Loading => TypelessFrontendMetadataResult::Loading,
            FrontendMetadataResult::Loaded(_) => TypelessFrontendMetadataResult::Loaded,
            FrontendMetadataResult::Error(error) => TypelessFrontendMetadataResult::Error(error),
        }
    }
}

pub enum TypelessFrontendMetadataResult {
    Loading,
    Loaded,
    Error(SharedString),
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
                if let FrontendMetadataState::Loaded { keep_alive, .. } = existing.read(cx) {
                    if !keep_alive.as_ref().map(|k| k.is_alive()).unwrap_or(true) {
                        this.backend_handle.send(MessageToBackend::RequestMetadata {
                            request: request.clone(),
                            force_reload: false,
                        });
                    }
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
        keep_alive: Option<KeepAliveHandle>,
        cx: &mut App,
    ) {
        entity.update(cx, |this, cx| {
            this.data.get(&request).unwrap().update(cx, |value, cx| {
                *value = FrontendMetadataState::Loaded { result, keep_alive };
                cx.notify();
            });
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
                    FrontendMetadataState::Loaded { result, .. } => {
                        match result {
                            Ok(MetadataResult::$t(result)) => FrontendMetadataResult::Loaded(&*result),
                            Ok(_) => FrontendMetadataResult::Error(ts!("system.metadata_error")),
                            Err(error) => FrontendMetadataResult::Error(SharedString::new(error.clone())),
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