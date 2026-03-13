use std::{
    collections::HashSet, hash::{DefaultHasher, Hash, Hasher}, io::Read, path::Path, process::Child, sync::Arc
};

use anyhow::Context;
use base64::Engine;
use bridge::{
    instance::{
        ContentSummary, ContentUpdateContext, ContentUpdateStatus, InstanceContentID, InstanceContentSummary, InstanceID, InstanceServerSummary, InstanceStatus, InstanceWorldSummary
    }, keep_alive::KeepAliveHandle, message::{BridgeDataLoadState, MessageToFrontend}, notify_signal::{KeepAliveNotifySignal, KeepAliveNotifySignalHandle}
};
use futures::FutureExt;
use relative_path::RelativePath;
use rustc_hash::FxHashSet;
use schema::{auxiliary::{AuxDisabledChildren, AuxiliaryContentMeta}, instance::InstanceConfiguration, loader::Loader};
use strum::IntoEnumIterator;
use thiserror::Error;

use ustr::Ustr;

use crate::{BackendState, BackendStateFileWatching, FolderChanges, IoOrSerializationError, WatchTarget, id_slab::{GetId, Id}, launcher_import, mod_metadata::{ContentUpdateAction, ContentUpdateKey, ModMetadataManager}, persistent::Persistent};

#[derive(Debug)]
pub struct Instance {
    pub id: InstanceID,
    pub root_path: Arc<Path>,
    pub dot_minecraft_path: Arc<Path>,
    pub server_dat_path: Arc<Path>,
    pub saves_path: Arc<Path>,
    pub name: Ustr,
    pub icon: Option<Arc<[u8]>>,
    pub configuration: Persistent<InstanceConfiguration>,

    pub launch_keepalive: Option<KeepAliveHandle>,
    pub processes: Vec<Child>,

    pub worlds_state: BridgeDataLoadState,
    dirty_worlds: FolderChanges,
    pending_worlds_load: Option<KeepAliveNotifySignalHandle>,
    worlds: Option<Arc<[InstanceWorldSummary]>>,

    pub servers_state: BridgeDataLoadState,
    dirty_servers: bool,
    pending_servers_load: Option<KeepAliveNotifySignalHandle>,
    servers: Option<Arc<[InstanceServerSummary]>>,

    content_generation: usize,

    pub content_state: enum_map::EnumMap<ContentFolder, ContentFolderState>,
}

#[derive(Debug)]
pub struct ContentFolderState {
    pub path: Arc<Path>,
    pub load_state: BridgeDataLoadState,
    dirty_paths: FolderChanges,
    generation: usize,
    pending_load: Option<KeepAliveNotifySignalHandle>,
    summaries: Option<Arc<[InstanceContentSummary]>>,
}

#[derive(enum_map::Enum, Debug, strum::EnumIter, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContentFolder {
    Mods,
    ResourcePacks,
}

impl ContentFolder {
    pub fn path(self) -> &'static RelativePath {
        match self {
            ContentFolder::Mods => RelativePath::new("mods"),
            ContentFolder::ResourcePacks => RelativePath::new("resourcepacks"),
        }
    }
}

impl ContentFolderState {
    pub fn new(path: Arc<Path>) -> Self {
        Self {
            path,
            load_state: BridgeDataLoadState::default(),
            dirty_paths: FolderChanges::all_dirty(),
            generation: 0,
            pending_load: None,
            summaries: None,
        }
    }
}

impl Id for InstanceID {
    fn get_index(&self) -> usize {
        self.index
    }
}

impl GetId for Instance {
    type Id = InstanceID;

    fn get_id(&self) -> Self::Id {
        self.id
    }
}

#[derive(Error, Debug)]
pub enum InstanceLoadError {
    #[error("Not a directory")]
    NotADirectory,
    #[error("An I/O error occured while trying to read the instance")]
    IoError(#[from] std::io::Error),
    #[error("A serialization error occured while trying to read the instance")]
    SerdeError(#[from] serde_json::Error),
}

impl From<IoOrSerializationError> for InstanceLoadError {
    fn from(value: IoOrSerializationError) -> Self {
        match value {
            IoOrSerializationError::Io(error) => Self::IoError(error),
            IoOrSerializationError::Serialization(error) => Self::SerdeError(error),
        }
    }
}

impl Instance {
    pub fn on_root_renamed(&mut self, backend: &Arc<BackendState>, path: &Path) {
        log::info!("Instance {:?} has been moved to {:?}", self.root_path, path);

        self.name = path.file_name().unwrap().to_string_lossy().into_owned().into();
        self.root_path = path.into();
        self.configuration = Persistent::load_or(path.join("info_v1.json").into(), self.configuration.get().clone());

        let mut dot_minecraft_path = path.to_owned();
        dot_minecraft_path.push(".minecraft");

        for content_folder in ContentFolder::iter() {
            self.content_state[content_folder].path = content_folder.path().to_path(&dot_minecraft_path).into();
            self.mark_content_dirty(backend, content_folder, FolderChanges::all_dirty(), true);
        }

        self.server_dat_path = dot_minecraft_path.join("servers.dat").into();
        self.mark_servers_dirty(backend, true);

        self.saves_path = dot_minecraft_path.join("saves").into();
        self.mark_world_dirty(backend, FolderChanges::all_dirty(), true);

        self.dot_minecraft_path = dot_minecraft_path.into();
    }

    pub fn rewatch_directories(&mut self, file_watching: &mut BackendStateFileWatching) {
        let mut watch_dot_minecraft = false;

        if self.servers_state.is_not_unloaded() {
            watch_dot_minecraft = true;
        }

        if self.worlds_state.is_not_unloaded() {
            file_watching.watch_filesystem(self.saves_path.clone(), WatchTarget::InstanceSavesDir { id: self.id });
            watch_dot_minecraft = true;
        }

        for folder in ContentFolder::iter() {
            if self.content_state[folder].load_state.is_not_unloaded() {
                file_watching.watch_filesystem(self.content_state[folder].path.clone(), WatchTarget::InstanceContentDir { id: self.id, folder });
                watch_dot_minecraft = true;
            }
        }

        if watch_dot_minecraft {
            file_watching.watch_filesystem(self.dot_minecraft_path.clone(), WatchTarget::InstanceDotMinecraftDir { id: self.id });
        }
    }

    pub fn mark_all_dirty(&mut self, backend: &Arc<BackendState>, reload: bool) {
        for content_folder in ContentFolder::iter() {
            self.mark_content_dirty(backend, content_folder, FolderChanges::all_dirty(), reload);
        }
        self.mark_servers_dirty(backend, reload);
        self.mark_world_dirty(backend, FolderChanges::all_dirty(), reload);
    }

    pub fn try_get_content(&self, id: InstanceContentID) -> Option<(&InstanceContentSummary, ContentFolder)> {
        for (folder, state) in &self.content_state {
            if state.generation == id.generation {
                let summaries = state.summaries.as_ref()?;
                let content = summaries.get(id.index)?;
                return Some((content, folder));
            }
        }
        None
    }

    pub async fn load_worlds(
        backend: Arc<BackendState>,
        id: InstanceID,
    ) -> Option<Arc<[InstanceWorldSummary]>> {
        Self::load_worlds_inner(backend, id).await
    }

    fn load_worlds_inner(
        backend: Arc<BackendState>,
        id: InstanceID,
    ) -> futures::future::BoxFuture<'static, Option<Arc<[InstanceWorldSummary]>>> {
        async move {
            let mut await_pending: Option<KeepAliveNotifySignalHandle> = None;

            let (future, keep_alive) = loop {
                if let Some(pending) = await_pending {
                    pending.await_notification().await;
                }

                let mut guard = backend.instance_state.write();
                let this = guard.instances.get_mut(id)?;

                if let Some(pending) = &this.pending_worlds_load && !pending.is_notified() {
                    await_pending = Some(pending.clone());
                    continue;
                }

                let mut file_watching = backend.file_watching.write();
                file_watching.watch_filesystem(this.dot_minecraft_path.clone(), WatchTarget::InstanceDotMinecraftDir {
                    id: this.id,
                });
                file_watching.watch_filesystem(this.saves_path.clone(), WatchTarget::InstanceSavesDir {
                    id: this.id,
                });

                let (all_dirty, dirty_paths) = this.dirty_worlds.take();
                let future = if let Some(last) = &this.worlds && !all_dirty {
                    if !dirty_paths.is_empty() {
                        let last = last.clone();
                        tokio::task::spawn_blocking(move || {
                            Self::load_worlds_dirty(dirty_paths, last)
                        })
                    } else {
                        return Some(last.clone());
                    }
                } else {
                    let saves_path = this.saves_path.clone();
                    tokio::task::spawn_blocking(move || {
                        Self::load_worlds_all(&saves_path)
                    })
                };

                let keep_alive = KeepAliveNotifySignal::new();
                this.pending_worlds_load = Some(keep_alive.create_handle());
                this.worlds_state.load_started();

                break (future, keep_alive);
            };

            let worlds = future.await.unwrap();

            let mut guard = backend.instance_state.write();
            let this = guard.instances.get_mut(id)?;

            this.worlds = Some(worlds.clone());
            this.worlds_state.load_finished();
            let should_load = this.worlds_state.should_load();
            drop(guard);

            backend.send.send(MessageToFrontend::InstanceWorldsUpdated {
                id,
                worlds: Arc::clone(&worlds)
            });

            let mut file_watching = backend.file_watching.write();
            for summary in worlds.iter() {
                file_watching.watch_filesystem(summary.level_path.clone(), WatchTarget::InstanceWorldDir {
                    id,
                });
            }
            drop(file_watching);

            keep_alive.notify();
            if should_load {
                tokio::task::spawn(Self::load_worlds_inner(backend.clone(), id));
            }
            Some(worlds)
        }.boxed()
    }

    fn load_worlds_all(saves_path: &Path) -> Arc<[InstanceWorldSummary]> {
        log::info!("Loading all worlds in {:?}", saves_path);

        let Ok(directory) = std::fs::read_dir(&saves_path) else {
            return [].into();
        };

        let mut count = 0;
        let mut summaries = Vec::with_capacity(64);

        for entry in directory {
            if count >= 64 {
                break;
            }

            let Ok(entry) = entry else {
                log::error!("Error reading directory in saves folder: {:?}", entry.unwrap_err());
                continue;
            };
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            count += 1;

            match load_world_summary(&path) {
                Ok(summary) => {
                    summaries.push(summary);
                },
                Err(err) => {
                    log::error!("Error loading world summary: {:?}", err);
                },
            }
        }

        summaries.sort_by_key(|s| -s.last_played);

        summaries.into()
    }

    fn load_worlds_dirty(dirty: FxHashSet<Arc<Path>>, last: Arc<[InstanceWorldSummary]>) -> Arc<[InstanceWorldSummary]> {
        log::debug!("Loading changed worlds");
        log::trace!("Changed worlds: {:?}", dirty);

        let mut summaries = Vec::with_capacity(64);

        let mut count = 0;

        for path in dirty.iter() {
            if count >= 64 {
                break;
            }

            if !path.is_dir() {
                continue;
            }

            count += 1;

            match load_world_summary(path) {
                Ok(summary) => {
                    summaries.push(summary);
                },
                Err(err) => {
                    log::error!("Error loading world summary: {:?}", err);
                },
            }
        }

        for old_summary in &*last {
            if !dirty.contains(&old_summary.level_path) && old_summary.level_path.exists() {
                summaries.push(old_summary.clone());
            }
        }

        summaries.sort_by_key(|s| -s.last_played);

        if summaries.len() > 64 {
            summaries.truncate(64);
        }

        summaries.into()
    }

    pub async fn load_servers(
        backend: Arc<BackendState>,
        id: InstanceID,
    ) -> Option<Arc<[InstanceServerSummary]>> {
        Self::load_servers_inner(backend, id).await
    }

    fn load_servers_inner(
        backend: Arc<BackendState>,
        id: InstanceID,
    ) -> futures::future::BoxFuture<'static, Option<Arc<[InstanceServerSummary]>>> {
        async move {
            let mut await_pending: Option<KeepAliveNotifySignalHandle> = None;

            let (future, keep_alive) = loop {
                if let Some(pending) = await_pending {
                    pending.await_notification().await;
                }

                let mut guard = backend.instance_state.write();
                let this = guard.instances.get_mut(id)?;

                if let Some(pending) = &this.pending_servers_load && !pending.is_notified() {
                    await_pending = Some(pending.clone());
                    continue;
                }

                let mut file_watching = backend.file_watching.write();
                file_watching.watch_filesystem(this.dot_minecraft_path.clone(), WatchTarget::InstanceDotMinecraftDir {
                    id: this.id,
                });

                let future = if let Some(last) = &this.servers && !this.dirty_servers {
                    return Some(last.clone());
                } else {
                    let server_dat_path = this.server_dat_path.clone();
                    tokio::task::spawn_blocking(move || {
                        Self::load_servers_all(&server_dat_path)
                    })
                };

                let keep_alive = KeepAliveNotifySignal::new();
                this.pending_servers_load = Some(keep_alive.create_handle());
                this.servers_state.load_started();

                this.dirty_servers = false;

                break (future, keep_alive);
            };

            let servers = future.await.unwrap();

            let mut guard = backend.instance_state.write();
            let this = guard.instances.get_mut(id)?;

            this.servers = Some(servers.clone());
            this.servers_state.load_finished();
            let should_load = this.servers_state.should_load();
            drop(guard);

            backend.send.send(MessageToFrontend::InstanceServersUpdated {
                id,
                servers: Arc::clone(&servers)
            });

            keep_alive.notify();
            if should_load {
                tokio::task::spawn(Self::load_servers_inner(backend, id));
            }
            Some(servers)
        }.boxed()
    }

    fn load_servers_all(server_dat_path: &Path) -> Arc<[InstanceServerSummary]> {
        log::info!("Loading servers from {:?}", server_dat_path);

        if !server_dat_path.is_file() {
            return Arc::from([]);
        }

        let result = match load_servers_summary(&server_dat_path) {
            Ok(summaries) => summaries.into(),
            Err(err) => {
                log::error!("Error loading servers: {:?}", err);
                Arc::from([])
            },
        };

        result
    }

    pub async fn load_content(
        backend: Arc<BackendState>,
        id: InstanceID,
        content_folder: ContentFolder,
    ) -> Option<Arc<[InstanceContentSummary]>> {
        Self::load_content_inner(backend, id, content_folder).await
    }

    fn load_content_inner(
        backend: Arc<BackendState>,
        id: InstanceID,
        content_folder: ContentFolder,
    ) -> futures::future::BoxFuture<'static, Option<Arc<[InstanceContentSummary]>>> {
        async move {
            let mut await_pending: Option<KeepAliveNotifySignalHandle> = None;

            let (future, keep_alive) = loop {
                if let Some(pending) = await_pending {
                    pending.await_notification().await;
                }

                let mut guard = backend.instance_state.write();
                let this = guard.instances.get_mut(id)?;
                let state = &mut this.content_state[content_folder];

                if let Some(pending) = &state.pending_load && !pending.is_notified() {
                    await_pending = Some(pending.clone());
                    continue;
                }

                let mut file_watching = backend.file_watching.write();
                file_watching.watch_filesystem(this.dot_minecraft_path.clone(), WatchTarget::InstanceDotMinecraftDir {
                    id: this.id,
                });
                file_watching.watch_filesystem(state.path.clone(), WatchTarget::InstanceContentDir {
                    id: this.id,
                    folder: content_folder
                });

                let (all_dirty, dirty_paths) = state.dirty_paths.take();
                let future = if let Some(last) = &state.summaries && !all_dirty {
                    if !dirty_paths.is_empty() {
                        let mod_metadata_manager = backend.mod_metadata_manager.clone();
                        let last = last.clone();
                        let config = this.configuration.get();
                        let for_loader = config.loader;
                        let for_version = config.minecraft_version;
                        tokio::task::spawn_blocking(move || {
                            Self::load_content_dirty(dirty_paths, mod_metadata_manager, last, for_loader, for_version)
                        })
                    } else {
                        return Some(last.clone());
                    }
                } else {
                    let path = state.path.clone();
                    let mod_metadata_manager = backend.mod_metadata_manager.clone();
                    let config = this.configuration.get();
                    let for_loader = config.loader;
                    let for_version = config.minecraft_version;
                    tokio::task::spawn_blocking(move || {
                        Self::load_content_all(&path, mod_metadata_manager, for_loader, for_version)
                    })
                };

                let keep_alive = KeepAliveNotifySignal::new();
                state.pending_load = Some(keep_alive.create_handle());
                state.load_state.load_started();

                break (future, keep_alive);
            };

            let mut result = future.await.unwrap();

            let mut guard = backend.instance_state.write();
            let this = guard.instances.get_mut(id)?;
            let state = &mut this.content_state[content_folder];

            this.content_generation = this.content_generation.wrapping_add(1);
            state.generation = this.content_generation;
            for (index, summary) in result.iter_mut().enumerate() {
                summary.id = InstanceContentID {
                    index,
                    generation: state.generation,
                };
            }

            let result: Arc<[InstanceContentSummary]> = result.into();
            state.summaries = Some(result.clone());
            state.pending_load = None;
            state.load_state.load_finished();
            let should_load = state.load_state.should_load();
            drop(guard);

            match content_folder {
                ContentFolder::Mods => {
                    backend.send.send(MessageToFrontend::InstanceModsUpdated {
                        id,
                        mods: Arc::clone(&result)
                    });
                },
                ContentFolder::ResourcePacks => {
                    backend.send.send(MessageToFrontend::InstanceResourcePacksUpdated {
                        id,
                        resource_packs: Arc::clone(&result)
                    });
                },
            }

            keep_alive.notify();
            if should_load {
                tokio::task::spawn(Self::load_content_inner(backend, id, content_folder));
            }
            Some(result)
        }.boxed()
    }

    fn load_content_all(
        path: &Path,
        mod_metadata_manager: Arc<ModMetadataManager>,
        for_loader: Loader,
        for_version: Ustr
    ) -> Vec<InstanceContentSummary> {
        log::info!("Loading all content from {:?}", path);

        let Ok(directory) = std::fs::read_dir(&path) else {
            return Vec::new();
        };

        let mut summaries = Vec::with_capacity(32);

        // todo: multithread?

        for entry in directory {
            let Ok(entry) = entry else {
                log::error!("Error reading file in content folder: {:?}", entry.unwrap_err());
                continue;
            };

            if let Some(summary) = create_instance_content_summary(&entry.path(), &mod_metadata_manager, for_loader, for_version) {
                summaries.push(summary);
            }
        }

        summaries.sort_by(|a, b| {
            a.content_summary.id.cmp(&b.content_summary.id)
                .then_with(|| lexical_sort::natural_lexical_cmp(&a.filename, &b.filename).reverse())
        });

        summaries
    }

    fn load_content_dirty(
        dirty: FxHashSet<Arc<Path>>,
        mod_metadata_manager: Arc<ModMetadataManager>,
        last: Arc<[InstanceContentSummary]>,
        for_loader: Loader,
        for_version: Ustr,
    ) -> Vec<InstanceContentSummary> {
        log::debug!("Loading changed content");
        log::trace!("Changed content: {:?}", dirty);

        let mut summaries = Vec::with_capacity(last.len() + 8);

        let mut alternative_dirty = HashSet::new();

        for path in dirty.iter() {
            let mut alternate_path = path.to_path_buf();
            if let Some(extension) = path.extension() && extension == "disabled" {
                alternate_path.set_extension("");
            } else {
                alternate_path.add_extension("disabled");
            };

            let check_alternative = !dirty.contains(&*alternate_path);

            if let Some(summary) = create_instance_content_summary(&path, &mod_metadata_manager, for_loader, for_version) {
                summaries.push(summary);
            } else if check_alternative {
                if let Some(summary) = create_instance_content_summary(&alternate_path, &mod_metadata_manager, for_loader, for_version) {
                    summaries.push(summary);
                }
            }
            if check_alternative {
                alternative_dirty.insert(alternate_path);
            }
        }

        for old_summary in &*last {
            if !dirty.contains(&old_summary.path) && !alternative_dirty.contains(&*old_summary.path) {
                if old_summary.path.exists() {
                    summaries.push(old_summary.clone());
                } else {
                    // Check if the file has been renamed to .disabled and we haven't been informed yet
                    // This isn't necessary because we *will* be informed of the rename
                    // But checking this here will prevent flickering in the UI

                    let mut alternate_path = old_summary.path.to_path_buf();
                    if old_summary.enabled {
                        alternate_path.add_extension("disabled");
                    } else {
                        alternate_path.set_extension("");
                    };

                    if alternate_path.exists() {
                        let enabled = !old_summary.enabled;

                        let Some(filename) = alternate_path.file_name().and_then(|s| s.to_str()) else {
                            continue;
                        };

                        let filename_without_disabled = if !enabled {
                            &filename[..filename.len()-".disabled".len()]
                        } else {
                            filename
                        };

                        let mut hasher = DefaultHasher::new();
                        filename_without_disabled.hash(&mut hasher);
                        let filename_hash = hasher.finish();

                        summaries.push(InstanceContentSummary {
                            content_summary: old_summary.content_summary.clone(),
                            id: InstanceContentID::dangling(),
                            lowercase_search_keys: old_summary.lowercase_search_keys.clone(),
                            filename: filename.into(),
                            filename_hash,
                            path: alternate_path.into(),
                            enabled,
                            content_source: old_summary.content_source.clone(),
                            update: old_summary.update.clone(),
                            disabled_children: old_summary.disabled_children.clone(),
                        });
                    }

                }
            }
        }

        summaries.sort_by(|a, b| {
            a.content_summary.id.cmp(&b.content_summary.id)
                .then_with(|| a.filename.cmp(&b.filename).reverse())
        });

        summaries
    }

    pub fn load_from_folder(path: impl AsRef<Path>) -> Result<Self, InstanceLoadError> {
        let path = path.as_ref();
        log::info!("Loading instance from {:?}", path);

        if !path.is_dir() {
            return Err(InstanceLoadError::NotADirectory);
        }

        let info_path: Arc<Path> = path.join("info_v1.json").into();

        let instance_info = if !info_path.exists() && let Some(fallback) = launcher_import::try_load_from_other_launcher_formats(&path) {
            Persistent::load_or(info_path.clone(), fallback)
        } else {
            Persistent::try_load(info_path.clone())?
        };

        let mut dot_minecraft_path = path.to_owned();
        dot_minecraft_path.push(".minecraft");

        let saves_path = dot_minecraft_path.join("saves");
        let server_dat_path = dot_minecraft_path.join("servers.dat");

        let content_state = enum_map::EnumMap::from_fn(|content_type: ContentFolder| {
            ContentFolderState::new(content_type.path().to_path(&dot_minecraft_path).into())
        });

        let icon_path = path.join("icon.png");
        let icon = std::fs::read(icon_path).ok().map(|v| v.into());

        Ok(Self {
            id: InstanceID::dangling(),
            root_path: path.into(),
            dot_minecraft_path: dot_minecraft_path.into(),
            server_dat_path: server_dat_path.into(),
            saves_path: saves_path.into(),
            name: path.file_name().unwrap().to_string_lossy().into_owned().into(),
            icon,
            configuration: instance_info,

            launch_keepalive: None,
            processes: Vec::new(),

            worlds_state: BridgeDataLoadState::default(),
            dirty_worlds: FolderChanges::all_dirty(),
            pending_worlds_load: None,
            worlds: None,

            servers_state: BridgeDataLoadState::default(),
            dirty_servers: true,
            pending_servers_load: None,
            servers: None,

            content_generation: 0,

            content_state,
        })
    }

    pub fn mark_world_dirty(&mut self, backend: &Arc<BackendState>, changes: FolderChanges, reload: bool) {
        if changes.is_empty() {
            return;
        }

        changes.apply_to(&mut self.dirty_worlds);

        self.worlds_state.set_dirty();
        if reload && self.worlds_state.should_load() {
            tokio::task::spawn(Self::load_worlds_inner(backend.clone(), self.id));
        }
    }

    pub fn mark_servers_dirty(&mut self, backend: &Arc<BackendState>, reload: bool) {
        if self.dirty_servers {
            return;
        }
        self.dirty_servers = true;

        self.servers_state.set_dirty();
        if reload && self.servers_state.should_load() {
            tokio::task::spawn(Self::load_servers_inner(backend.clone(), self.id));
        }
    }

    pub fn mark_content_dirty(&mut self, backend: &Arc<BackendState>, content_folder: ContentFolder, mut changes: FolderChanges, reload: bool) {
        if changes.is_empty() {
            return;
        }
        let state = &mut self.content_state[content_folder];

        let (all_dirty, paths) = changes.take();

        if all_dirty {
            state.dirty_paths.dirty_all();
        } else {
            let mut total_aux_paths = 0;
            for path in &paths {
                if let Some(filename) = path.file_name() {
                    if filename.as_encoded_bytes().ends_with(b".aux.json") {
                        total_aux_paths += 1;
                        continue;
                    }
                }

                state.dirty_paths.dirty_path(path.clone());
            }

            if total_aux_paths > 0 {
                let mut used_aux_paths = FxHashSet::default();
                if let Some(summaries) = &state.summaries {
                    for summary in summaries.iter() {
                        let Some(aux_path) = crate::pandora_aux_path_for_content(&summary) else {
                            continue;
                        };
                        if paths.contains(aux_path.as_path()) {
                            used_aux_paths.insert(aux_path);
                            state.dirty_paths.dirty_path(summary.path.clone());
                        }
                    }
                }
                if used_aux_paths.len() < total_aux_paths {
                    state.dirty_paths.dirty_all();
                }
            }
        }

        state.load_state.set_dirty();
        if reload && state.load_state.should_load() {
            tokio::task::spawn(Self::load_content_inner(backend.clone(), self.id, content_folder));
        }
    }

    pub fn copy_basic_attributes_from(&mut self, new: Self) {
        assert_eq!(new.id, InstanceID::dangling());

        self.root_path = new.root_path;
        self.name = new.name;
        self.configuration = new.configuration;
    }

    pub fn status(&self) -> InstanceStatus {
        if !self.processes.is_empty() {
            InstanceStatus::Running
        } else if let Some(keepalive) = &self.launch_keepalive && keepalive.is_alive() {
            InstanceStatus::Launching
        } else {
            InstanceStatus::NotRunning
        }
    }

    pub fn create_modify_message(&mut self) -> MessageToFrontend {
        MessageToFrontend::InstanceModified {
            id: self.id,
            name: self.name,
            icon: self.icon.clone(),
            root_path: self.resolve_real_root_path(),
            dot_minecraft_folder: self.dot_minecraft_path.clone(),
            configuration: self.configuration.get().clone(),
            status: self.status(),
        }
    }

    pub fn resolve_real_root_path(&self) -> Arc<Path> {
        #[cfg(windows)]
        if let Ok(target) = junction::get_target(&self.root_path) {
            return target.into();
        };

        if let Ok(target) = std::fs::read_link(&self.root_path) {
            target.into()
        } else {
            self.root_path.clone()
        }
    }
}

fn create_instance_content_summary(path: &Path, mod_metadata_manager: &Arc<ModMetadataManager>, for_loader: Loader, for_version: Ustr) -> Option<InstanceContentSummary> {
    if !path.is_file() {
        return None;
    }
    let Some(filename) = path.file_name().and_then(|s| s.to_str()) else {
        return None;
    };
    if filename.starts_with(".pandora.") {
        return None;
    }
    let enabled = if filename.ends_with(".jar.disabled") || filename.ends_with(".mrpack.disabled") || filename.ends_with(".zip.disabled") {
        false
    } else if filename.ends_with(".jar") || filename.ends_with(".mrpack") || filename.ends_with(".zip") {
        true
    } else {
        log::trace!("Skipping content file {}, unknown extension", filename);
        return None;
    };
    let Ok(mut file) = std::fs::File::open(&path) else {
        return None;
    };

    let Some(summary) = mod_metadata_manager.get_file(&mut file) else {
        return None;
    };

    let filename_without_disabled = if !enabled {
        &filename[..filename.len()-".disabled".len()]
    } else {
        filename
    };

    let mut hasher = DefaultHasher::new();
    filename_without_disabled.hash(&mut hasher);
    let filename_hash = hasher.finish();

    let filename: Arc<str> = filename.into();
    let lowercase_filename = filename.to_lowercase();
    let lowercase_filename = if lowercase_filename == &*filename {
        filename.clone()
    } else {
        lowercase_filename.into()
    };

    let content_source = mod_metadata_manager.read_content_sources().get(&summary.hash).unwrap_or_default();

    let lowercase_search_keys = summary.id.clone().into_iter()
        .chain(summary.name.clone().into_iter())
        .chain(std::iter::once(lowercase_filename))
        .collect();

    let disabled_children = read_disabled_children_for(&summary, path).unwrap_or_default();

    let update_status = mod_metadata_manager.updates.read().get(&ContentUpdateKey {
        hash: summary.hash,
        loader: for_loader,
        version: for_version,
    }).map(ContentUpdateAction::to_status).unwrap_or(ContentUpdateStatus::Unknown);

    Some(InstanceContentSummary {
        content_summary: summary,
        id: InstanceContentID::dangling(),
        lowercase_search_keys,
        filename,
        filename_hash,
        path: path.into(),
        enabled,
        content_source,
        update: ContentUpdateContext::new(update_status, for_loader, for_version),
        disabled_children: Arc::new(disabled_children),
    })
}

fn read_disabled_children_for(
    summary: &ContentSummary,
    path: &Path,
) -> Option<AuxDisabledChildren> {
    let aux_path = crate::pandora_aux_path(&summary.id, &summary.name, path)?;
    let aux: AuxiliaryContentMeta = crate::read_json(&aux_path).ok()?;
    Some(aux.disabled_children)
}

fn load_world_summary(path: &Path) -> anyhow::Result<InstanceWorldSummary> {
    let level_dat_path = path.join("level.dat");
    if !level_dat_path.is_file() {
        anyhow::bail!("level.dat doesn't exist");
    }

    let compressed = std::fs::read(&level_dat_path)?;

    let mut decoder = flate2::bufread::GzDecoder::new(compressed.as_slice());

    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;

    let mut nbt_data = decompressed.as_slice();
    let result = nbt::decode::read_named(&mut nbt_data)?;

    let root = result.as_compound().context("Unable to get root compound")?;
    let data = root.find_compound("Data").context("Unable to get Data")?;
    let last_played: i64 = data.find_numeric("LastPlayed").context("Unable to get LastPlayed")?;
    let level_name = data.find_string("LevelName").cloned().unwrap_or_default();

    let folder = path.file_name().context("Unable to get filename")?.to_string_lossy();

    let subtitle = if let Some(date_time) = chrono::DateTime::from_timestamp_millis(last_played) && last_played > 0 {
        let date_time = date_time.with_timezone(&chrono::Local);
        format!("{} ({})", folder, date_time.format("%d/%m/%Y %H:%M")).into()
    } else {
        format!("{}", folder).into()
    };

    let title = if level_name.is_empty() {
        folder.into_owned().into()
    } else {
        level_name.into()
    };

    let icon_path = path.join("icon.png");
    let icon = if icon_path.is_file() {
        std::fs::read(icon_path).map(Arc::from).ok()
    } else {
        None
    };

    Ok(InstanceWorldSummary {
        title,
        subtitle,
        level_path: path.into(),
        last_played,
        png_icon: icon,
    })
}

fn load_servers_summary(server_dat_path: &Path) -> anyhow::Result<Vec<InstanceServerSummary>> {
    let raw = std::fs::read(server_dat_path)?;

    let mut nbt_data = raw.as_slice();
    let result = nbt::decode::read_named(&mut nbt_data)?;

    let root = result.as_compound().context("Unable to get root compound")?;
    let servers = root.find_list("servers", nbt::TAG_COMPOUND_ID).context("Unable to get servers")?;

    let mut summaries = Vec::with_capacity(servers.len());

    for server in servers.iter() {
        let server = server.as_compound().unwrap();

        if let Some(hidden) = server.find_byte("hidden")
            && *hidden != 0
        {
            continue;
        }

        let Some(ip) = server.find_string("ip") else {
            continue;
        };

        let name: Arc<str> = server
            .find_string("name")
            .map(|v| Arc::from(v.as_str()))
            .unwrap_or_else(|| Arc::from("<unnamed>"));

        let icon = server
            .find_string("icon")
            .and_then(|v| base64::engine::general_purpose::STANDARD.decode(v).map(Arc::from).ok());

        summaries.push(InstanceServerSummary {
            name,
            ip: Arc::from(ip.as_str()),
            png_icon: icon,
        });
    }

    Ok(summaries)
}
