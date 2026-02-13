use std::{io::Write, path::Path, sync::Arc, time::Duration};

use gpui::{App, SharedString, Task};
use rand::RngCore;
use schema::{modrinth::ModrinthProjectType, instance::{InstanceMemoryConfiguration, InstanceJvmFlagsConfiguration, InstanceJvmBinaryConfiguration}};
use serde::{Deserialize, Serialize};

use crate::ui::SerializedPageType;

struct InterfaceConfigHolder {
    config: InterfaceConfig,
    write_task: Option<Task<()>>,
    path: Arc<Path>,
}

impl gpui::Global for InterfaceConfigHolder {}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct InterfaceConfig {
    #[serde(default, deserialize_with = "schema::try_deserialize")]
    pub active_theme: SharedString,
    #[serde(default, deserialize_with = "schema::try_deserialize")]
    pub main_window_bounds: WindowBounds,
    #[serde(default, deserialize_with = "schema::try_deserialize")]
    pub sidebar_width: f32,
    #[serde(default, deserialize_with = "schema::try_deserialize")]
    pub main_page: SerializedPageType,
    #[serde(default, deserialize_with = "schema::try_deserialize")]
    pub page_path: Vec<SerializedPageType>,
    #[serde(default, deserialize_with = "schema::try_deserialize")]
    pub quick_delete_mods: bool,
    #[serde(default, deserialize_with = "schema::try_deserialize")]
    pub quick_delete_instance: bool,
    #[serde(default, deserialize_with = "schema::try_deserialize")]
    pub modrinth_install_normally: bool,
    #[serde(default, deserialize_with = "schema::try_deserialize")]
    pub modrinth_page_project_type: ModrinthProjectType,
    #[serde(default, deserialize_with = "schema::try_deserialize")]
    pub hide_main_window_on_launch: bool,
    #[serde(default)]
    pub global_override_enabled: bool,
    #[serde(default)]
    pub global_memory: Option<InstanceMemoryConfiguration>,
    #[serde(default)]
    pub global_jvm_flags: Option<InstanceJvmFlagsConfiguration>,
    #[serde(default)]
    pub global_jvm_binary: Option<InstanceJvmBinaryConfiguration>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WindowBounds {
    #[default]
    Inherit,
    Windowed {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
    Maximized {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
    Fullscreen {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
}

impl InterfaceConfig {
    pub fn init(cx: &mut App, path: Arc<Path>) {
        cx.set_global(InterfaceConfigHolder {
            config: try_read_json(&path),
            write_task: None,
            path,
        });
    }

    pub fn get(cx: &App) -> &Self {
        &cx.global::<InterfaceConfigHolder>().config
    }

    pub fn force_save(cx: &mut App) {
        cx.global_mut::<InterfaceConfigHolder>().write_to_disk();
    }

    pub fn get_mut(cx: &mut App) -> &mut Self {
        if cx.global::<InterfaceConfigHolder>().write_task.is_none() {
            let task = cx.spawn(async |app| {
                gpui::Timer::after(Duration::from_secs(5)).await;
                _ = app.update_global::<InterfaceConfigHolder, _>(|holder, _| {
                    holder.write_to_disk();
                });
            });

            let holder = cx.global_mut::<InterfaceConfigHolder>();
            holder.write_task = Some(task);
            &mut holder.config
        } else {
            &mut cx.global_mut::<InterfaceConfigHolder>().config
        }
    }
}

impl InterfaceConfigHolder {
    fn write_to_disk(&mut self) {
        self.write_task = None;
        let Ok(bytes) = serde_json::to_vec(&self.config) else {
            return;
        };
        _ = write_safe(&self.path, &bytes);
    }
}

pub(crate) fn try_read_json<T: std::fmt::Debug + Default + for <'de> Deserialize<'de>>(path: &Path) -> T {
    let Ok(data) = std::fs::read(path) else {
        return T::default();
    };
    serde_json::from_slice(&data).unwrap_or_default()
}

pub(crate) fn write_safe(path: &Path, content: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let mut temp = path.to_path_buf();
    temp.add_extension(format!("{}", rand::thread_rng().next_u32()));
    temp.add_extension("new");

    let mut temp_file = std::fs::File::create(&temp)?;

    temp_file.write_all(content)?;
    temp_file.flush()?;
    temp_file.sync_all()?;

    drop(temp_file);

    if let Err(err) = std::fs::rename(&temp, path) {
        _ = std::fs::remove_file(&temp);
        return Err(err);
    }

    Ok(())
}
