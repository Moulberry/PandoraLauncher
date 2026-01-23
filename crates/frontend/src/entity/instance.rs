use std::{path::Path, sync::Arc};

use bridge::{
    instance::{InstanceID, InstanceContentSummary, InstanceServerSummary, InstanceStatus, InstanceWorldSummary},
    message::AtomicBridgeDataLoadState,
};
use gpui::{prelude::*, *};
use gpui_component::select::SelectItem;
use indexmap::IndexMap;
use schema::{instance::InstanceConfiguration, loader::Loader};

pub struct InstanceEntries {
    pub entries: IndexMap<InstanceID, Entity<InstanceEntry>>,
}

impl InstanceEntries {
    pub fn add(
        entity: &Entity<Self>,
        id: InstanceID,
        name: SharedString,
        dot_minecraft_folder: Arc<Path>,
        configuration: InstanceConfiguration,
        worlds_state: Arc<AtomicBridgeDataLoadState>,
        servers_state: Arc<AtomicBridgeDataLoadState>,
        mods_state: Arc<AtomicBridgeDataLoadState>,
        resource_packs_state: Arc<AtomicBridgeDataLoadState>,
        cx: &mut App,
    ) {
        entity.update(cx, |entries, cx| {
            let mut instance = InstanceEntry {
                id,
                name,
                title: "".into(),
                dot_minecraft_folder,
                configuration,
                status: InstanceStatus::NotRunning,
                worlds_state,
                worlds: cx.new(|_| [].into()),
                servers_state,
                servers: cx.new(|_| [].into()),
                mods_state,
                mods: cx.new(|_| [].into()),
                resource_packs_state,
                resource_packs: cx.new(|_| [].into()),
            };
            instance.title = instance.create_title().into();

            entries.entries.insert_before(0, id, cx.new(|_| instance.clone()));
            cx.emit(InstanceAddedEvent { instance });
        });
    }

    pub fn find_id_by_name(entity: &Entity<Self>, name: &SharedString, cx: &App) -> Option<InstanceID> {
        for (id, entry) in &entity.read(cx).entries {
            if &entry.read(cx).name == name {
                return Some(*id);
            }
        }
        None
    }

    pub fn find_name_by_id(entity: &Entity<Self>, id: InstanceID, cx: &App) -> Option<SharedString> {
        if let Some(entry) = entity.read(cx).entries.get(&id) {
            return Some(entry.read(cx).name.clone())
        }
        None
    }

    pub fn find_title_by_id(entity: &Entity<Self>, id: InstanceID, cx: &App) -> Option<SharedString> {
        if let Some(entry) = entity.read(cx).entries.get(&id) {
            return Some(entry.read(cx).title())
        }
        None
    }

    pub fn remove(entity: &Entity<Self>, id: InstanceID, cx: &mut App) {
        entity.update(cx, |entries, cx| {
            if let Some(_) = entries.entries.shift_remove(&id) {
                cx.emit(InstanceRemovedEvent { id });
            }
        });
    }

    pub fn modify(
        entity: &Entity<Self>,
        id: InstanceID,
        name: SharedString,
        dot_minecraft_folder: Arc<Path>,
        configuration: InstanceConfiguration,
        status: InstanceStatus,
        cx: &mut App,
    ) {
        entity.update(cx, |entries, cx| {
            if let Some(instance) = entries.entries.get_mut(&id) {
                let cloned = instance.update(cx, |instance, cx| {
                    instance.name = name.clone();
                    instance.dot_minecraft_folder = dot_minecraft_folder.clone();
                    instance.configuration = configuration.clone();
                    instance.status = status;
                    instance.title = instance.create_title().into();
                    cx.notify();

                    instance.clone()
                });

                cx.emit(InstanceModifiedEvent { instance: cloned });
            }
        });
    }

    pub fn set_worlds(
        entity: &Entity<Self>,
        id: InstanceID,
        worlds: Arc<[InstanceWorldSummary]>,
        cx: &mut App,
    ) {
        entity.update(cx, |entries, cx| {
            if let Some(instance) = entries.entries.get_mut(&id) {
                instance.update(cx, |instance, cx| {
                    instance.worlds.update(cx, |existing_worlds, cx| {
                        *existing_worlds = worlds;
                        cx.notify();
                    })
                });
            }
        });
    }

    pub fn set_servers(
        entity: &Entity<Self>,
        id: InstanceID,
        servers: Arc<[InstanceServerSummary]>,
        cx: &mut App,
    ) {
        entity.update(cx, |entries, cx| {
            if let Some(instance) = entries.entries.get_mut(&id) {
                instance.update(cx, |instance, cx| {
                    instance.servers.update(cx, |existing_servers, cx| {
                        *existing_servers = servers;
                        cx.notify();
                    })
                });
            }
        });
    }

    pub fn set_mods(entity: &Entity<Self>, id: InstanceID, mods: Arc<[InstanceContentSummary]>, cx: &mut App) {
        entity.update(cx, |entries, cx| {
            if let Some(instance) = entries.entries.get_mut(&id) {
                instance.update(cx, |instance, cx| {
                    instance.mods.update(cx, |existing_mods, cx| {
                        *existing_mods = mods;
                        cx.notify();
                    })
                });
            }
        });
    }

    pub fn set_resource_packs(entity: &Entity<Self>, id: InstanceID, resource_packs: Arc<[InstanceContentSummary]>, cx: &mut App) {
        entity.update(cx, |entries, cx| {
            if let Some(instance) = entries.entries.get_mut(&id) {
                instance.update(cx, |instance, cx| {
                    instance.resource_packs.update(cx, |existing_resource_packs, cx| {
                        *existing_resource_packs = resource_packs;
                        cx.notify();
                    })
                });
            }
        });
    }

    pub fn move_to_top(entity: &Entity<Self>, id: InstanceID, cx: &mut App) {
        entity.update(cx, |entries, cx| {
            if let Some(index) = entries.entries.get_index_of(&id) {
                entries.entries.move_index(index, 0);
                let (_, entry) = entries.entries.get_index(0).unwrap();
                cx.emit(InstanceMovedToTopEvent {
                    instance: entry.read(cx).clone(),
                });
            }
        });
    }
}

#[derive(Clone)]
pub struct InstanceEntry {
    pub id: InstanceID,
    pub name: SharedString,
    pub title: SharedString,
    pub dot_minecraft_folder: Arc<Path>,
    pub configuration: InstanceConfiguration,
    pub status: InstanceStatus,
    pub worlds_state: Arc<AtomicBridgeDataLoadState>,
    pub worlds: Entity<Arc<[InstanceWorldSummary]>>,
    pub servers_state: Arc<AtomicBridgeDataLoadState>,
    pub servers: Entity<Arc<[InstanceServerSummary]>>,
    pub mods_state: Arc<AtomicBridgeDataLoadState>,
    pub mods: Entity<Arc<[InstanceContentSummary]>>,
    pub resource_packs_state: Arc<AtomicBridgeDataLoadState>,
    pub resource_packs: Entity<Arc<[InstanceContentSummary]>>,
}

impl SelectItem for InstanceEntry {
    type Value = Self;

    fn title(&self) -> SharedString {
        self.name.clone()
    }

    fn value(&self) -> &Self::Value {
        &self
    }
}

impl PartialEq for InstanceEntry {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl InstanceEntry {
    pub fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn create_title(&self) -> String {
        if self.name == &*self.configuration.minecraft_version {
            if self.configuration.loader == Loader::Vanilla {
                format!("{}", self.name)
            } else {
                format!("{} ({:?})", self.name, self.configuration.loader)
            }
        } else if self.configuration.loader == Loader::Vanilla {
            format!("{} ({})", self.name, self.configuration.minecraft_version)
        } else {
            format!("{} ({:?} {})", self.name, self.configuration.loader, self.configuration.minecraft_version)
        }
    }
}

impl EventEmitter<InstanceAddedEvent> for InstanceEntries {}

pub struct InstanceAddedEvent {
    pub instance: InstanceEntry,
}

impl EventEmitter<InstanceMovedToTopEvent> for InstanceEntries {}

pub struct InstanceMovedToTopEvent {
    pub instance: InstanceEntry,
}

impl EventEmitter<InstanceModifiedEvent> for InstanceEntries {}

pub struct InstanceModifiedEvent {
    pub instance: InstanceEntry,
}

impl EventEmitter<InstanceRemovedEvent> for InstanceEntries {}

pub struct InstanceRemovedEvent {
    pub id: InstanceID,
}
