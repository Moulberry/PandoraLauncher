use std::{collections::HashSet, ffi::OsStr, path::Path, sync::Arc};

use bridge::{instance::InstanceID, message::MessageToFrontend};
use notify::{
    EventKind,
    event::{CreateKind, DataChange, ModifyKind, RemoveKind, RenameMode},
};
use rustc_hash::FxHashSet;
use strum::IntoEnumIterator;

use crate::{BackendState, WatchTarget, instance::ContentFolder};

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
    reload_immediately: FxHashSet<(InstanceID, ContentFolder)>,
}

impl BackendState {
    pub async fn handle_filesystem(&mut self, result: notify_debouncer_full::DebounceEventResult) {
        match result {
            Ok(events) => {
                let mut after_debounce_effects = AfterDebounceEffects {
                    reload_immediately: Default::default(),
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
                for (instance_id, folder) in after_debounce_effects.reload_immediately {
                    tokio::task::spawn(self.clone().load_instance_content(instance_id, folder));
                }
            },
            Err(_) => {
                log::error!("An error occurred while watching the filesystem! The launcher might be out-of-sync with your files!");
                self.send.send_error("An error occurred while watching the filesystem! The launcher might be out-of-sync with your files!");
            },
        }
    }

    async fn handle_filesystem_change_event(
        &mut self,
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
        &mut self,
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
        &mut self,
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
        &mut self,
        target: WatchTarget,
        _path: &Arc<Path>,
        _after_debounce_effects: &mut AfterDebounceEffects,
    ) -> bool {
        match target {
            WatchTarget::ServersDat { id } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.mark_servers_dirty();
                }
                true
            },
            _ => false,
        }
    }

    async fn filesystem_handle_removed(
        &mut self,
        target: WatchTarget,
        path: &Arc<Path>,
        _after_debounce_effects: &mut AfterDebounceEffects,
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

                instance_state.reload_immediately.clear();

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
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.mark_world_dirty(Some(path.clone()));
                }
                true
            },
            WatchTarget::InstanceSavesDir { id } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.mark_world_dirty(None);
                }
                true
            },
            WatchTarget::ServersDat { id } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.mark_servers_dirty();
                }
                // Minecraft moves the servers.dat to servers.dat_old and then back,
                // so lets just re-listen immediately
                self.file_watching.write().watch_filesystem(path.clone(), target);
                true
            },
            WatchTarget::InstanceContentDir { id, folder } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.content_state[folder].mark_dirty(None);
                }
                true
            },
            WatchTarget::InstanceDotMinecraftDir { id } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.mark_world_dirty(None);
                    instance.mark_servers_dirty();
                    for folder in ContentFolder::iter() {
                        instance.content_state[folder].mark_dirty(None);
                    }
                }
                true
            },
        }
    }

    async fn filesystem_handle_renamed(
        &mut self,
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
                    instance.on_root_renamed(to);

                    self.send.send_info(format!("Instance '{}' renamed to '{}'", old_name, instance.name));
                    self.send.send(instance.create_modify_message());

                    let mut file_watching = self.file_watching.write();
                    file_watching.watch_filesystem(to.clone(), WatchTarget::InstanceDir { id });
                    if instance.watching_dot_minecraft {
                        file_watching.watch_filesystem(instance.dot_minecraft_path.clone(), WatchTarget::InstanceDotMinecraftDir { id });
                    }
                    if instance.watching_saves_dir {
                        file_watching.watch_filesystem(instance.saves_path.clone(), WatchTarget::InstanceSavesDir { id });
                    }
                    if instance.watching_server_dat {
                        file_watching.watch_filesystem(instance.server_dat_path.clone(), WatchTarget::ServersDat { id });
                    }
                    for folder in ContentFolder::iter() {
                        if instance.content_state[folder].watching_path {
                            file_watching.watch_filesystem(instance.content_state[folder].path.clone(), WatchTarget::InstanceContentDir { id, folder });
                        }
                    }
                    true
                } else {
                    false
                }
            },
            _ => false,
        }
    }

    async fn filesystem_handle_child_change(
        &mut self,
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
                }
            }
            WatchTarget::InstancesDir => {
                if path.is_dir() {
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
                } else if file_name == ".minecraft"
                    && let Some(instance) = self.instance_state.write().instances.get_mut(id)
                {
                    instance.mark_world_dirty(None);
                    instance.mark_servers_dirty();
                    for folder in ContentFolder::iter() {
                        instance.content_state[folder].mark_dirty(None);
                    }

                    let mut file_watching = self.file_watching.write();
                    if instance.watching_dot_minecraft {
                        file_watching.watch_filesystem(path.clone(), WatchTarget::InstanceDotMinecraftDir { id });
                    }
                    if instance.watching_saves_dir {
                        file_watching.watch_filesystem(instance.saves_path.clone(), WatchTarget::InstanceSavesDir { id });
                    }
                    if instance.watching_server_dat {
                        file_watching.watch_filesystem(instance.server_dat_path.clone(), WatchTarget::ServersDat { id });
                    }
                    for folder in ContentFolder::iter() {
                        if instance.content_state[folder].watching_path {
                            file_watching.watch_filesystem(instance.content_state[folder].path.clone(), WatchTarget::InstanceContentDir { id, folder });
                        }
                    }
                }
            },
            WatchTarget::ServersDat { .. } => {},
            WatchTarget::InstanceDotMinecraftDir { id } => {
                let Some(file_name) = path.file_name() else {
                    return;
                };
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    let Some(name) = file_name.to_str() else {
                        return;
                    };

                    for folder in ContentFolder::iter() {
                        if name == folder.path().as_str() && instance.content_state[folder].watching_path {
                            instance.content_state[folder].mark_dirty(None);
                            self.file_watching.write().watch_filesystem(path.clone(), WatchTarget::InstanceContentDir { id, folder });
                            return;
                        }
                    }
                    match name {
                        "saves" if instance.watching_saves_dir => {
                            instance.mark_world_dirty(None);
                            self.file_watching.write().watch_filesystem(path.clone(), WatchTarget::InstanceSavesDir { id });
                        },
                        "servers.dat" if instance.watching_server_dat => {
                            instance.mark_servers_dirty();
                            self.file_watching.write().watch_filesystem(path.clone(), WatchTarget::ServersDat { id });
                        },
                        _ => {},
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
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.mark_world_dirty(Some(parent_path.into()));
                }
            },
            WatchTarget::InstanceSavesDir { id } => {
                // If a world folder is added to the saves directory, mark the world (path) as dirty
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.mark_world_dirty(Some(path.clone()));
                }
            },
            WatchTarget::InstanceContentDir { id, folder } => {
                let mut instance_state = self.instance_state.write();
                if let Some(instance) = instance_state.instances.get_mut(id) {
                    instance.content_state[folder].mark_dirty(Some(path.clone()));
                    if instance_state.reload_immediately.remove(&(id, folder)) {
                        after_debounce_effects.reload_immediately.insert((id, folder));
                    }
                }
            },
        }
    }

    async fn filesystem_handle_child_removed(
        &mut self,
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
            WatchTarget::InstanceWorldDir { id } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.mark_world_dirty(Some(parent_path.into()));
                }
            },
            WatchTarget::InstanceSavesDir { id } => {
                if let Some(instance) = self.instance_state.write().instances.get_mut(id) {
                    instance.mark_world_dirty(Some(path.clone()));
                }
            },
            WatchTarget::InstanceContentDir { id, folder } => {
                let mut instance_state = self.instance_state.write();
                if let Some(instance) = instance_state.instances.get_mut(id) {
                    instance.content_state[folder].mark_dirty(Some(path.clone()));
                    if instance_state.reload_immediately.remove(&(id, folder)) {
                        after_debounce_effects.reload_immediately.insert((id, folder));
                    }
                }
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
                if event.paths[0].extension() == Some(OsStr::new("new")) {
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
