use std::{
    collections::HashSet, hash::{DefaultHasher, Hash, Hasher}, io::Read, path::Path, sync::Arc, time::{Instant, SystemTime, UNIX_EPOCH}
};

use anyhow::Context;
use base64::Engine;
use bridge::{
    instance::{
        ContentFolder, ContentSummary, ContentUpdateContext, ContentUpdateStatus, InstanceContentID, InstanceContentSummary, InstanceID, InstancePlaytime, InstanceServerSummary, InstanceStatus, InstanceWorldSummary
    }, keep_alive::KeepAliveHandle, message::{BridgeDataLoadState, MessageToFrontend}, notify_signal::{KeepAliveNotifySignal, KeepAliveNotifySignalHandle},
};
use command::PandoraProcess;
use futures::FutureExt;
use rustc_hash::FxHashSet;
use schema::{auxiliary::{AuxDisabledChildren, AuxiliaryContentMeta}, instance::InstanceConfiguration, loader::Loader, unique_bytes::UniqueBytes};
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use thiserror::Error;

use ustr::Ustr;

use crate::{BackendState, BackendStateFileWatching, FolderChanges, IoOrSerializationError, WatchTarget, id_slab::{GetId, Id}, launcher_import, mod_metadata::{ContentUpdateAction, ContentUpdateKey, ModMetadataManager}, persistent::Persistent, server_list_pinger::{PingResult, ServerListPinger}};

#[derive(Debug)]
pub struct Instance {
    pub id: InstanceID,
    pub root_path: Arc<Path>,
    pub dot_minecraft_path: Arc<Path>,
    pub server_dat_path: Arc<Path>,
    pub saves_path: Arc<Path>,
    pub name: Ustr,
    pub icon: Option<UniqueBytes>,
    pub configuration: Persistent<InstanceConfiguration>,
    pub stats: Persistent<InstanceStats>,

    pub launch_keepalive: Option<KeepAliveHandle>,
    pub processes: Vec<PandoraProcess>,
    pub closing_processes: Vec<(PandoraProcess, Instant)>,
    session_started_at: Option<Instant>,

    pub worlds_state: BridgeDataLoadState,
    dirty_worlds: FolderChanges,
    pending_worlds_load: Option<KeepAliveNotifySignalHandle>,
    worlds: Option<Arc<[InstanceWorldSummary]>>,

    pub servers_state: BridgeDataLoadState,
    dirty_servers: bool,
    pending_servers_load: Option<KeepAliveNotifySignalHandle>,
    servers: Option<Arc<[InstanceServerSummary]>>,

    content_generation: usize,

    frozen_mods_folder: bool,
    pub content_state: enum_map::EnumMap<ContentFolder, ContentFolderState>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstanceStats {
    pub total_playtime_secs: u64,
    pub session_count: u64,
    #[serde(default)]
    pub last_played_unix_ms: Option<i64>,
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
        self.stats = Persistent::load_or(path.join("stats_v1.json").into(), self.stats.get().clone());

        let mut dot_minecraft_path = path.to_owned();
        dot_minecraft_path.push(".minecraft");

        for content_folder in ContentFolder::iter() {
            self.content_state[content_folder].path = dot_minecraft_path.join(content_folder.folder_name()).into();
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

    pub async fn reorder_servers(
        backend: Arc<BackendState>,
        id: InstanceID,
        from_index: usize,
        to_index: usize,
    ) {
        let server_dat_path = {
            let guard = backend.instance_state.read();
            let Some(instance) = guard.instances.get(id) else {
                return;
            };
            instance.server_dat_path.clone()
        };

        if !server_dat_path.is_file() {
            backend.send.send_error("server.dat is not a file");
            return;
        }

        let raw = match std::fs::read(&server_dat_path) {
            Ok(raw) => raw,
            Err(err) => {
                log::error!("Error while reading server.dat: {err:?}");
                backend.send.send_error("Error while reading server.dat: {err}");
                return;
            },
        };
        let mut nbt_data = raw.as_slice();
        let mut result = match nbt::decode::read_named(&mut nbt_data) {
            Ok(result) => result,
            Err(err) => {
                log::error!("Error while decoding server.dat: {err:?}");
                backend.send.send_error("Error while decoding server.dat: {err}");
                return;
            },
        };

        let Some(mut root) = result.as_compound_mut() else {
            backend.send.send_error("Unable to get root compound");
            return;
        };
        let Some(mut servers) = root.find_list_mut("servers", nbt::TAG_COMPOUND_ID) else {
            backend.send.send_error("Unable to get servers list");
            return;
        };

        if servers.move_index(from_index, to_index) {
            let bytes = nbt::encode::write_named(&result);
            if let Err(err) = crate::write_safe(&server_dat_path, &bytes) {
                log::error!("Error while writing server.dat: {err:?}");
                backend.send.send_error("Error while writing server.dat: {err}");
                return;
            }
        }
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
                    let backend = backend.clone();
                    let instance_id = this.id;
                    let version = this.configuration.get().minecraft_version;
                    tokio::task::spawn_blocking(move || {
                        Self::load_servers_all(&server_dat_path, &backend, version, instance_id)
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

    fn load_servers_all(server_dat_path: &Path, backend: &Arc<BackendState>, version: Ustr, instance: InstanceID) -> Arc<[InstanceServerSummary]> {
        log::info!("Loading servers from {:?}", server_dat_path);

        if !server_dat_path.is_file() {
            return Arc::from([]);
        }

        let result = match load_servers_summary(&server_dat_path, backend, version, instance) {
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

                if this.frozen_mods_folder && content_folder == ContentFolder::Mods && let Some(last) = &state.summaries {
                    return Some(last.clone());
                }

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

            if content_folder == ContentFolder::Shaders && !result.is_empty() && !this.configuration.get().show_shader_tab {
                this.configuration.modify(|config| {
                    config.show_shader_tab = true;
                });
            } else if content_folder == ContentFolder::Mods && !this.configuration.get().show_shader_tab {
                let mut has_shader_mod = false;
                'out: for summary in &result {
                    if let Some(id) = summary.content_summary.id.as_deref() {
                        if crate::KNOWN_SHADER_MODS.contains(&id) {
                            has_shader_mod = true;
                            break 'out;
                        }
                    }
                    if let Some(modpack_files) = summary.content_summary.extra.modpack_files() {
                        for modpack_file in modpack_files.iter() {
                            if let Some(summary) = &modpack_file.summary {
                                if let Some(id) = summary.id.as_deref() {
                                    if crate::KNOWN_SHADER_MODS.contains(&id) {
                                        has_shader_mod = true;
                                        break 'out;
                                    }
                                }
                            }
                        }
                    }
                }
                if has_shader_mod {
                    this.configuration.modify(|config| {
                        config.show_shader_tab = true;
                    });
                }
            }

            let result: Arc<[InstanceContentSummary]> = result.into();
            state.summaries = Some(result.clone());
            state.pending_load = None;
            state.load_state.load_finished();
            let should_load = state.load_state.should_load();
            drop(guard);

            backend.send.send(MessageToFrontend::InstanceContentUpdated {
                id,
                content_folder,
                content: Arc::clone(&result)
            });

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
                .then_with(|| lexical_sort::natural_lexical_cmp(&a.filename, &b.filename))
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

            alternative_dirty.insert(alternate_path);
        }

        for old_summary in &*last {
            if !dirty.contains(&old_summary.path) && !alternative_dirty.contains(&*old_summary.path) {
                if old_summary.path.exists() {
                    summaries.push(old_summary.clone());
                }
            }
        }

        summaries.sort_by(|a, b| {
            a.content_summary.id.cmp(&b.content_summary.id)
                .then_with(|| lexical_sort::natural_lexical_cmp(&a.filename, &b.filename))
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
        let stats = Persistent::load(path.join("stats_v1.json").into());

        let mut dot_minecraft_path = path.to_owned();
        dot_minecraft_path.push(".minecraft");

        let saves_path = dot_minecraft_path.join("saves");
        let server_dat_path = dot_minecraft_path.join("servers.dat");

        let content_state = enum_map::EnumMap::from_fn(|content_type: ContentFolder| {
            ContentFolderState::new(dot_minecraft_path.join(content_type.folder_name()).into())
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
            stats,

            launch_keepalive: None,
            processes: Vec::new(),
            closing_processes: Vec::new(),
            session_started_at: None,

            worlds_state: BridgeDataLoadState::default(),
            dirty_worlds: FolderChanges::all_dirty(),
            pending_worlds_load: None,
            worlds: None,

            servers_state: BridgeDataLoadState::default(),
            dirty_servers: true,
            pending_servers_load: None,
            servers: None,

            content_generation: 0,

            frozen_mods_folder: false,
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

    pub fn update_session(&mut self) {
        let running = !self.processes.is_empty();
        if running {
            if self.session_started_at.is_some() {
                return;
            }

            self.session_started_at = Some(Instant::now());
            let now = unix_time_ms_now();
            self.stats.modify(|stats| {
                stats.session_count = stats.session_count.saturating_add(1);
                stats.last_played_unix_ms = now;
            });
        } else {
            let Some(started_at) = self.session_started_at.take() else {
                return;
            };

            let elapsed = started_at.elapsed().as_secs();
            self.stats.modify(|stats| {
                stats.total_playtime_secs = stats.total_playtime_secs.saturating_add(elapsed);
            });
        }
    }

    pub fn playtime(&mut self) -> InstancePlaytime {
        let stats = self.stats.get().clone();
        let current_session_secs = self.session_started_at
            .map(|started_at| started_at.elapsed().as_secs())
            .unwrap_or(0);

        InstancePlaytime {
            total_secs: stats.total_playtime_secs.saturating_add(current_session_secs),
            current_session_secs,
            last_played_unix_ms: stats.last_played_unix_ms,
        }
    }

    pub fn has_active_session(&self) -> bool {
        self.session_started_at.is_some()
    }

    pub fn status(&self) -> InstanceStatus {
        if !self.processes.is_empty() {
            InstanceStatus::Running
        } else if !self.closing_processes.is_empty() {
            InstanceStatus::Stopping
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
            playtime: self.playtime(),
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

    pub fn set_frozen_mods_folder(&mut self, frozen_mods_folder: bool) {
        self.frozen_mods_folder = frozen_mods_folder;
    }
}

fn unix_time_ms_now() -> Option<i64> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_millis()).ok()
}

fn create_instance_content_summary(path: &Path, mod_metadata_manager: &Arc<ModMetadataManager>, for_loader: Loader, for_version: Ustr) -> Option<InstanceContentSummary> {
    if !path.is_file() {
        // Special case for loading a resourcepack folder
        if let Ok(pack_mcmeta_bytes) = std::fs::read(path.join("pack.mcmeta")) {
            let pack_png_bytes = std::fs::read(path.join("pack.png")).ok();
            return try_load_resourcepack_folder(&pack_mcmeta_bytes, pack_png_bytes.as_deref(), path);
        }

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

    let metadata = file.metadata().ok();
    let modified_unix_ms = if let Some(metadata) = &metadata {
        let mut time = SystemTime::UNIX_EPOCH;
        if let Ok(created) = metadata.created() {
            time = time.max(created);
        }
        if let Ok(modified) = metadata.modified() {
            time = time.max(modified);
        }
        time.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_millis().min(u64::MAX as u128) as u64
    } else {
        0
    };

    let summary = mod_metadata_manager.get_file(&mut file, metadata, path.extension());

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
        modified_unix_ms,
        path: path.into(),
        can_toggle: true,
        enabled,
        content_source,
        update: ContentUpdateContext::new(update_status, for_loader, for_version.as_str()),
        disabled_children: Arc::new(disabled_children),
    })
}

fn try_load_resourcepack_folder(pack_mcmeta_bytes: &[u8], pack_png_bytes: Option<&[u8]>, path: &Path) -> Option<InstanceContentSummary> {
    let Some(filename) = path.file_name().and_then(|s| s.to_str()) else {
        return None;
    };

    let summary = ModMetadataManager::create_resource_pack(pack_mcmeta_bytes, pack_png_bytes)?;

    let mut hasher = DefaultHasher::new();
    filename.hash(&mut hasher);
    let filename_hash = hasher.finish();

    let filename: Arc<str> = filename.into();
    let lowercase_filename = filename.to_lowercase();
    let lowercase_filename = if lowercase_filename == &*filename {
        filename.clone()
    } else {
        lowercase_filename.into()
    };

    let lowercase_search_keys = summary.id.clone().into_iter()
        .chain(summary.name.clone().into_iter())
        .chain(std::iter::once(lowercase_filename))
        .collect();

    return Some(InstanceContentSummary {
        content_summary: summary,
        id: InstanceContentID::dangling(),
        lowercase_search_keys,
        filename,
        filename_hash,
        modified_unix_ms: 0,
        path: path.into(),
        can_toggle: false,
        enabled: true,
        content_source: schema::content::ContentSource::Manual,
        update: ContentUpdateContext::new(ContentUpdateStatus::ManualInstall, Loader::Vanilla, ""),
        disabled_children: Default::default(),
    });
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
        std::fs::read(icon_path).map(UniqueBytes::from).ok()
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

fn load_servers_summary(server_dat_path: &Path, backend: &Arc<BackendState>, version: Ustr, instance: InstanceID) -> anyhow::Result<Vec<InstanceServerSummary>> {
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

        let ip: Arc<str> = ip.as_str().into();
        let result = ServerListPinger::load_status(backend, ip.clone(), version, instance);
        let (pinging, status, ping) = match result {
            PingResult::Pinging => (true, None, None),
            PingResult::Loaded { status, ping } => (false, Some(status), ping),
            PingResult::Error => (false, None, None),
        };

        let name: Arc<str> = server
            .find_string("name")
            .map(|v| Arc::from(v.as_str()))
            .unwrap_or_else(|| Arc::from("<unnamed>"));

        let mut icon: Option<UniqueBytes> = if let Some(status) = &status
            && let Some(icon) = &status.favicon
            && let Some(base64) = icon.strip_prefix("data:image/png;base64,")
        {
            base64::engine::general_purpose::STANDARD.decode(base64.replace('\n', "")).map(UniqueBytes::from).ok()
        } else {
            None
        };

        if icon.is_none() {
            icon = server
                .find_string("icon")
                .and_then(|v| base64::engine::general_purpose::STANDARD.decode(v).map(UniqueBytes::from).ok());
        }

        summaries.push(InstanceServerSummary {
            name,
            ip,
            png_icon: icon,
            pinging,
            status,
            ping,
        });
    }

    Ok(summaries)
}
