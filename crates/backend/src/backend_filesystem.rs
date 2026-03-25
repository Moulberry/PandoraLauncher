use std::{ffi::OsStr, path::Path, sync::Arc};

use bridge::{instance::InstanceID, message::MessageToFrontend};
use notify::{
    EventKind,
    event::{DataChange, ModifyKind, RenameMode},
};
use rustc_hash::{FxHashMap, FxHashSet};
use strum::IntoEnumIterator;

use crate::{BackendState, FolderChanges, WatchTarget, instance::ContentFolder, skin_manager::SkinManager};

#[derive(Debug)]
enum FilesystemEvent {
    Change(Arc<Path>),
    Remove(Arc<Path>),
    Rename(Arc<Path>, Arc<Path>),
}

impl FilesystemEvent {
    pub fn change_or_remove_path(&self) -> Option<&Arc<Path>> {
        match self {
            FilesystemEvent::Change(path) => Some(path),
            FilesystemEvent::Remove(path) => Some(path),
            FilesystemEvent::Rename(..) => None,
        }
    }
}

struct AfterDebounceEffects {
    skin_manager_changes: FolderChanges,
    content_changes: FxHashMap<(InstanceID, ContentFolder), FolderChanges>,
    world_changes: FxHashMap<InstanceID, FolderChanges>,
    server_dat_changes: FxHashSet<InstanceID>,
}

impl BackendState {
    pub async fn handle_filesystem(self: &Arc<Self>, result: notify_debouncer_full::DebounceEventResult) {
        match result {
            Ok(events) => {
                let mut after_debounce_effects = AfterDebounceEffects {
                    skin_manager_changes: FolderChanges::no_changes(),
                    content_changes: Default::default(),
                    world_changes: Default::default(),
                    server_dat_changes: Default::default(),
                };

                let mut last_event: Option<FilesystemEvent> = None;
                for event in events {
                    let Some(next_event) = get_simple_event(event.event) else {
                        continue;
                    };

                    log::trace!("Filesystem event: {:?}", next_event);

                    if let Some(last_event) = last_event.take() {
                        let last_path = last_event.change_or_remove_path();
                        let new_path = next_event.change_or_remove_path();
                        if last_path.is_none() || last_path != new_path {
                            self.handle_filesystem_event(last_event, &mut after_debounce_effects).await;
                        }
                    }

                    last_event = Some(next_event);
                }
                if let Some(last_event) = last_event.take() {
                    self.handle_filesystem_event(last_event, &mut after_debounce_effects).await;
                }
                SkinManager::skin_library_mark_dirty(self, after_debounce_effects.skin_manager_changes);
                let mut instances = self.instance_state.write();
                for ((instance, folder), changes) in after_debounce_effects.content_changes {
                    if let Some(instance) = instances.instances.get_mut(instance) {
                        instance.mark_content_dirty(self, folder, changes, true);
                    }
                }
                for (instance, changes) in after_debounce_effects.world_changes {
                    if let Some(instance) = instances.instances.get_mut(instance) {
                        instance.mark_world_dirty(self, changes, true);
                    }
                }
                for instance in after_debounce_effects.server_dat_changes {
                    if let Some(instance) = instances.instances.get_mut(instance) {
                        instance.mark_servers_dirty(self, true);
                    }
                }
            },
            Err(_) => {
                log::error!("An error occurred while watching the filesystem! The launcher might be out-of-sync with your files!");
                self.send.send_error("An error occurred while watching the filesystem! The launcher might be out-of-sync with your files!");
            },
        }
    }

    async fn handle_filesystem_change_event(
        self: &Arc<Self>,
        path: Arc<Path>,
        after_debounce_effects: &mut AfterDebounceEffects,
    ) {
        let target = self.file_watching.read().get_target(&path).copied();
        if let Some(target) = target && self.filesystem_handle_change(target, &path, after_debounce_effects).await {
            return;
        }
        let Some(parent_path) = path.parent() else {
            return;
        };
        let parent = self.file_watching.read().get_target(parent_path).copied();
        if let Some(parent) = parent {
            self.filesystem_handle_child_change(parent, parent_path, &path, after_debounce_effects).await;
        }
    }

    async fn handle_filesystem_remove_event(
        self: &Arc<Self>,
        path: Arc<Path>,
        target: Option<WatchTarget>,
        after_debounce_effects: &mut AfterDebounceEffects,
    ) {
        if let Some(target) = target
            && self.filesystem_handle_removed(target, &path, after_debounce_effects).await
        {
            return;
        }
        let Some(parent_path) = path.parent() else {
            return;
        };
        let parent = self.file_watching.write().get_target(parent_path).copied();
        if let Some(parent) = parent {
            self.filesystem_handle_child_removed(parent, parent_path, &path, after_debounce_effects).await;
        }
    }

    async fn handle_filesystem_event(
        self: &Arc<Self>,
        event: FilesystemEvent,
        after_debounce_effects: &mut AfterDebounceEffects,
    ) {
        match event {
            FilesystemEvent::Change(path) => {
                let paths = self.file_watching.write().all_paths(path.clone());
                for path in paths {
                    self.handle_filesystem_change_event(path, after_debounce_effects).await;
                }
            },
            FilesystemEvent::Remove(path) => {
                let paths = self.file_watching.write().all_paths(path.clone());
                for path in paths {
                    let target = self.file_watching.write().remove(&path);
                    self.handle_filesystem_remove_event(path, target, after_debounce_effects).await;
                }
            },
            FilesystemEvent::Rename(from, to) => {
                if let Some(from_parent) = from.parent() && to.parent() == Some(from_parent) && let Some(to_name) = to.file_name() {
                    let from_paths = self.file_watching.write().all_paths(from.clone());
                    for from in from_paths {
                        let to = from_parent.join(to_name).into();

                        let target = self.file_watching.write().remove(&from);
                        if let Some(target) = target
                            && self.filesystem_handle_renamed(target, &from, &to, after_debounce_effects).await
                        {
                            return;
                        }
                        self.handle_filesystem_remove_event(from, target, after_debounce_effects).await;
                        self.handle_filesystem_change_event(to, after_debounce_effects).await;
                    }
                } else {
                    let from_paths = self.file_watching.write().all_paths(from.clone());
                    for from in from_paths {
                        let target = self.file_watching.write().remove(&from);
                        self.handle_filesystem_remove_event(from, target, after_debounce_effects).await;
                    }

                    let to_paths = self.file_watching.write().all_paths(from.clone());
                    for to in to_paths {
                        self.handle_filesystem_change_event(to, after_debounce_effects).await;
                    }
                }

            },
        }
    }

    async fn filesystem_handle_change(
        self: &Arc<Self>,
        _target: WatchTarget,
        _path: &Arc<Path>,
        _after_debounce_effects: &mut AfterDebounceEffects,
    ) -> bool {
        false
    }

    async fn filesystem_handle_removed(
        self: &Arc<Self>,
        target: WatchTarget,
        path: &Arc<Path>,
        after_debounce_effects: &mut AfterDebounceEffects,
    ) -> bool {
        match target {
            WatchTarget::RootDir => {
                self.send.send_error("Launcher directory has been removed! This is very bad!");
                true
            },
            WatchTarget::InstancesDir => {
                self.send.send_error("Instances dir has been been removed! Uh oh!");

                let mut instance_state = self.instance_state.write();

                for instance in instance_state.instances.drain() {
                    self.send.send(MessageToFrontend::InstanceRemoved { id: instance.id });
                }

                true
            },
            WatchTarget::InstanceDir { id } => {
                self.remove_instance(id);
                true
            },
            WatchTarget::InvalidInstanceDir => {
                true
            },
            WatchTarget::InstanceWorldDir { id } => {
                after_debounce_effects.world_changes.entry(id)
                    .or_insert_with(FolderChanges::no_changes)
                    .dirty_path(path.clone());
                true
            },
            WatchTarget::InstanceSavesDir { id } => {
                after_debounce_effects.world_changes.entry(id)
                    .or_insert_with(FolderChanges::no_changes)
                    .dirty_all();
                true
            },
            WatchTarget::InstanceContentDir { id, folder } => {
                after_debounce_effects.content_changes.entry((id, folder))
                    .or_insert_with(FolderChanges::no_changes)
                    .dirty_all();
                true
            },
            WatchTarget::InstanceDotMinecraftDir { id } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.mark_all_dirty(self, false);
                }
                true
            },
            WatchTarget::SkinLibraryDir => {
                after_debounce_effects.skin_manager_changes.dirty_all();
                true
            }
        }
    }

    async fn filesystem_handle_renamed(
        self: &Arc<Self>,
        from_target: WatchTarget,
        from: &Arc<Path>,
        to: &Arc<Path>,
        _after_debounce_effects: &mut AfterDebounceEffects,
    ) -> bool {
        match from_target {
            WatchTarget::InstanceDir { id } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id)
                    && from.parent() == to.parent()
                {
                    let old_name = instance.name;
                    instance.on_root_renamed(self, to);

                    let mut file_watching = self.file_watching.write();
                    instance.rewatch_directories(&mut *file_watching);
                    file_watching.watch_filesystem(to.clone(), WatchTarget::InstanceDir { id });
                    drop(file_watching);

                    self.send.send_info(format!("Instance '{}' renamed to '{}'", old_name, instance.name));
                    self.send.send(instance.create_modify_message());

                    true
                } else {
                    false
                }
            },
            _ => false,
        }
    }

    async fn filesystem_handle_child_change(
        self: &Arc<Self>,
        parent: WatchTarget,
        parent_path: &Path,
        path: &Arc<Path>,
        after_debounce_effects: &mut AfterDebounceEffects,
    ) {
        match parent {
            WatchTarget::RootDir => {
                let Some(file_name) = path.file_name() else {
                    return;
                };
                if file_name == "instances" {
                    self.load_all_instances().await;
                } else if file_name == "config.json" {
                    self.config.write().mark_changed(&path);
                } else if file_name == "accounts.json" {
                    let mut account_info = self.account_info.write();
                    account_info.mark_changed(&path);
                    self.send.send(account_info.get().create_update_message());
                } else if file_name == "skins" {
                    let is_not_unloaded = self.skin_manager.read().skin_library_state.is_not_unloaded();
                    if is_not_unloaded {
                        self.file_watching.write().watch_filesystem(path.clone(), WatchTarget::SkinLibraryDir);
                        after_debounce_effects.skin_manager_changes.dirty_all();
                    }
                }
            }
            WatchTarget::InstancesDir => {
                if path.is_dir() {
                    let Some(file_name) = path.file_name() else {
                        return;
                    };
                    if file_name.as_encoded_bytes()[0] == b'.' {
                        return;
                    }

                    let success = self.load_instance_from_path(path, false, true);
                    if !success {
                        self.file_watching.write().watch_filesystem(path.clone(), WatchTarget::InvalidInstanceDir);
                    }
                }
            },
            WatchTarget::InstanceDir { id } => {
                let Some(file_name) = path.file_name() else {
                    return;
                };
                if file_name == "info_v1.json" {
                    if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                        instance.configuration.mark_changed(&path);
                        self.send.send(instance.create_modify_message());
                    } else {
                        self.load_instance_from_path(parent_path, true, true);
                    }
                } else if file_name == "stats_v1.json" {
                    if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                        instance.stats.mark_changed(&path);
                        self.send.send(instance.create_modify_message());
                    }
                } else if file_name == ".minecraft"
                    && let Some(instance) = self.instance_state.write().instances.get_mut(id)
                {
                    instance.mark_all_dirty(self, false);
                    instance.rewatch_directories(&mut *self.file_watching.write());
                }
            },
            WatchTarget::InstanceDotMinecraftDir { id } => {
                let Some(file_name) = path.file_name() else {
                    return;
                };
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    let Some(name) = file_name.to_str() else {
                        return;
                    };
                    match name {
                        "saves" => {
                            after_debounce_effects.world_changes.entry(id)
                                .or_insert_with(FolderChanges::no_changes)
                                .dirty_all();
                            if instance.worlds_state.is_not_unloaded() {
                                self.file_watching.write().watch_filesystem(path.clone(), WatchTarget::InstanceSavesDir { id });
                            }
                            return;
                        },
                        "servers.dat" => {
                            let mut ignore = self.server_dat_ignore.write();
                            if let Some(count) = ignore.get_mut(&id) {
                                if *count > 1 {
                                    *count -= 1;
                                } else {
                                    ignore.remove(&id);
                                }
                            } else {
                                after_debounce_effects.server_dat_changes.insert(id);
                            }
                            return;
                        },
                        _ => {},
                    }
                    for folder in ContentFolder::iter() {
                        if name == folder.path().as_str() {
                            after_debounce_effects.content_changes.entry((id, folder))
                                .or_insert_with(FolderChanges::no_changes)
                                .dirty_all();
                            if instance.content_state[folder].load_state.is_not_unloaded() {
                                self.file_watching.write().watch_filesystem(path.clone(), WatchTarget::InstanceContentDir { id, folder });
                            }
                            return;
                        }
                    }
                }
            },
            WatchTarget::InvalidInstanceDir => {
                let Some(file_name) = path.file_name() else {
                    return;
                };
                if file_name == "info_v1.json" {
                    self.load_instance_from_path(parent_path, true, true);
                }
            },
            WatchTarget::InstanceWorldDir { id } => {
                // If a file inside the world folder is changed (e.g. icon.png), mark the world (parent) as dirty
                let Some(file_name) = path.file_name() else {
                    return;
                };
                if file_name == "level.dat" || file_name == "icon.png" {
                    after_debounce_effects.world_changes.entry(id)
                        .or_insert_with(FolderChanges::no_changes)
                        .dirty_path(parent_path.into());
                }
            },
            WatchTarget::InstanceSavesDir { id } => {
                // If a world folder is added to the saves directory, mark the world (path) as dirty
                after_debounce_effects.world_changes.entry(id)
                    .or_insert_with(FolderChanges::no_changes)
                    .dirty_path(path.clone());
            },
            WatchTarget::InstanceContentDir { id, folder } => {
                after_debounce_effects.content_changes.entry((id, folder))
                    .or_insert_with(FolderChanges::no_changes)
                    .dirty_path(path.clone());
            },
            WatchTarget::SkinLibraryDir => {
                after_debounce_effects.skin_manager_changes.dirty_path(path.clone());
            },
        }
    }

    async fn filesystem_handle_child_removed(
        self: &Arc<Self>,
        parent: WatchTarget,
        parent_path: &Path,
        path: &Arc<Path>,
        after_debounce_effects: &mut AfterDebounceEffects,
    ) {
        match parent {
            WatchTarget::InstanceDir { id } => {
                let Some(file_name) = path.file_name() else {
                    return;
                };
                if file_name == "info_v1.json" {
                    self.remove_instance(id);
                    self.file_watching.write().watch_filesystem(parent_path.into(), WatchTarget::InvalidInstanceDir);
                }
            },
            WatchTarget::InstanceDotMinecraftDir { id } => {
                let Some(file_name) = path.file_name() else {
                    return;
                };
                if file_name == "servers.dat" {
                    let mut ignore = self.server_dat_ignore.write();
                    if let Some(count) = ignore.get_mut(&id) {
                        if *count > 1 {
                            *count -= 1;
                        } else {
                            ignore.remove(&id);
                        }
                    } else {
                        after_debounce_effects.server_dat_changes.insert(id);
                    }
                }
            }
            WatchTarget::InstanceWorldDir { id } => {
                let Some(file_name) = path.file_name() else {
                    return;
                };
                if file_name == "level.dat" || file_name == "icon.png" {
                    after_debounce_effects.world_changes.entry(id)
                        .or_insert_with(FolderChanges::no_changes)
                        .dirty_path(parent_path.into());
                }
            },
            WatchTarget::InstanceSavesDir { id } => {
                after_debounce_effects.world_changes.entry(id)
                    .or_insert_with(FolderChanges::no_changes)
                    .dirty_path(path.clone());
            },
            WatchTarget::InstanceContentDir { id, folder } => {
                after_debounce_effects.content_changes.entry((id, folder))
                    .or_insert_with(FolderChanges::no_changes)
                    .dirty_path(path.clone());
            },
            WatchTarget::SkinLibraryDir => {
                after_debounce_effects.skin_manager_changes.dirty_path(path.clone());
            },
            _ => {},
        }
    }
}

fn get_simple_event(event: notify::Event) -> Option<FilesystemEvent> {
    match event.kind {
        EventKind::Create(_) => {
            if event.paths[0].extension() == Some(OsStr::new("new")) {
                return None;
            }
            Some(FilesystemEvent::Change(event.paths[0].clone().into()))
        },
        EventKind::Modify(modify_kind) => match modify_kind {
            ModifyKind::Any => {
                if event.paths[0].is_dir() || event.paths[0].extension() == Some(OsStr::new("new")) {
                    return None;
                }
                Some(FilesystemEvent::Change(event.paths[0].clone().into()))
            },
            ModifyKind::Data(data_change) => {
                if event.paths[0].extension() == Some(OsStr::new("new")) {
                    return None;
                }
                if data_change == DataChange::Any || data_change == DataChange::Content {
                    Some(FilesystemEvent::Change(event.paths[0].clone().into()))
                } else {
                    None
                }
            },
            ModifyKind::Metadata(_) => None,
            ModifyKind::Name(rename_mode) => match rename_mode {
                RenameMode::Any => {
                    if event.paths[0].extension() == Some(OsStr::new("new")) {
                        return None;
                    }
                    let path = event.paths[0].clone().into();
                    if std::fs::exists(&path).unwrap_or(true) {
                        Some(FilesystemEvent::Change(path))
                    } else {
                        Some(FilesystemEvent::Remove(path))
                    }
                },
                RenameMode::To => {
                    if event.paths[0].extension() == Some(OsStr::new("new")) {
                        return None;
                    }
                    Some(FilesystemEvent::Change(event.paths[0].clone().into()))
                },
                RenameMode::From => {
                    if event.paths[0].extension() == Some(OsStr::new("new")) {
                        return None;
                    }
                    Some(FilesystemEvent::Remove(event.paths[0].clone().into()))
                },
                RenameMode::Both => {
                    if event.paths[0].extension() == Some(OsStr::new("new")) {
                        Some(FilesystemEvent::Change(event.paths[1].clone().into()))
                    } else {
                        Some(FilesystemEvent::Rename(event.paths[0].clone().into(), event.paths[1].clone().into()))
                    }
                },
                RenameMode::Other => None,
            },
            ModifyKind::Other => None,
        },
        EventKind::Remove(_) => {
            if event.paths[0].extension() == Some(OsStr::new("new")) {
                return None;
            }
            Some(FilesystemEvent::Remove(event.paths[0].clone().into()))
        },
        EventKind::Any => None,
        EventKind::Access(_) => None,
        EventKind::Other => None,
    }
}
