use std::{collections::HashSet, path::Path, sync::Arc};

use serde::{Deserialize, Serialize};
use ustr::Ustr;

use crate::loader::Loader;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InstanceConfiguration {
    pub minecraft_version: Ustr,
    pub loader: Loader,
    #[serde(default, skip_serializing_if = "crate::skip_if_none")]
    pub preferred_loader_version: Option<Ustr>,
    #[serde(default, deserialize_with = "crate::try_deserialize", skip_serializing_if = "is_default_memory_configuration")]
    pub memory: Option<InstanceMemoryConfiguration>,
    #[serde(default, deserialize_with = "crate::try_deserialize", skip_serializing_if = "is_default_jvm_flags_configuration")]
    pub jvm_flags: Option<InstanceJvmFlagsConfiguration>,
    #[serde(default, deserialize_with = "crate::try_deserialize", skip_serializing_if = "is_default_jvm_binary_configuration")]
    pub jvm_binary: Option<InstanceJvmBinaryConfiguration>,
    #[serde(default, deserialize_with = "crate::try_deserialize", skip_serializing_if = "is_default_sync_configuration")]
    pub sync: Option<InstanceSyncConfiguration>,
    #[serde(default, deserialize_with = "crate::try_deserialize", skip_serializing_if = "is_default_linux_wrapper_configuration")]
    pub linux_wrapper: Option<InstanceLinuxWrapperConfiguration>,
    #[serde(default, deserialize_with = "crate::try_deserialize", skip_serializing_if = "is_default_system_libraries_configuration")]
    pub system_libraries: Option<InstanceSystemLibrariesConfiguration>,
    #[serde(default, deserialize_with = "crate::try_deserialize", skip_serializing_if = "crate::skip_if_none")]
    pub instance_fallback_icon: Option<Ustr>,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct InstanceMemoryConfiguration {
    pub enabled: bool,
    pub min: u32,
    pub max: u32,
}

impl InstanceMemoryConfiguration {
    pub const DEFAULT_MIN: u32 = 512;
    pub const DEFAULT_MAX: u32 = 4096;
}

impl Default for InstanceMemoryConfiguration {
    fn default() -> Self {
        Self {
            enabled: false,
            min: Self::DEFAULT_MIN,
            max: Self::DEFAULT_MAX
        }
    }
}

fn is_default_memory_configuration(config: &Option<InstanceMemoryConfiguration>) -> bool {
    if let Some(config) = config {
        !config.enabled
            && config.min == InstanceMemoryConfiguration::DEFAULT_MIN
            && config.max == InstanceMemoryConfiguration::DEFAULT_MAX
    } else {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct InstanceJvmFlagsConfiguration {
    pub enabled: bool,
    pub flags: Arc<str>,
}

fn is_default_jvm_flags_configuration(config: &Option<InstanceJvmFlagsConfiguration>) -> bool {
    if let Some(config) = config {
        !config.enabled && config.flags.trim_ascii().is_empty()
    } else {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct InstanceJvmBinaryConfiguration {
    pub enabled: bool,
    pub path: Option<Arc<Path>>,
}

fn is_default_jvm_binary_configuration(config: &Option<InstanceJvmBinaryConfiguration>) -> bool {
    if let Some(config) = config {
        !config.enabled && config.path.is_none()
    } else {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InstanceSyncConfiguration {
    pub enabled: bool,
    pub sync_ids: HashSet<u64>,
}

fn is_default_sync_configuration(config: &Option<InstanceSyncConfiguration>) -> bool {
    if let Some(config) = config {
        !config.enabled && config.sync_ids.is_empty()
    } else {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct InstanceLinuxWrapperConfiguration {
    #[serde(default, deserialize_with = "crate::try_deserialize")]
    pub use_mangohud: bool,
    #[serde(default, deserialize_with = "crate::try_deserialize")]
    pub use_gamemode: bool,
    #[serde(default = "crate::default_true", deserialize_with = "crate::try_deserialize")]
    pub use_discrete_gpu: bool,
}

impl Default for InstanceLinuxWrapperConfiguration {
    fn default() -> Self {
        Self {
            use_mangohud: false,
            use_gamemode: false,
            use_discrete_gpu: true,
        }
    }
}

fn is_default_linux_wrapper_configuration(config: &Option<InstanceLinuxWrapperConfiguration>) -> bool {
    if let Some(config) = config {
        !config.use_mangohud && !config.use_gamemode && config.use_discrete_gpu
    } else {
        true
    }
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct InstanceSystemLibrariesConfiguration {
    pub override_glfw: bool,
    pub glfw: LwjglLibraryPath,
    pub override_openal: bool,
    pub openal: LwjglLibraryPath,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub enum LwjglLibraryPath {
    #[default]
    None,
    Auto(Arc<Path>),
    Explicit(Arc<Path>),
}

fn is_default_system_libraries_configuration(config: &Option<InstanceSystemLibrariesConfiguration>) -> bool {
    if let Some(config) = config {
        matches!(config.glfw, LwjglLibraryPath::None) && matches!(config.openal, LwjglLibraryPath::None)
    } else {
        true
    }
}

impl LwjglLibraryPath {
    pub fn get_or_auto(self, auto: &Option<Arc<Path>>) -> Option<Arc<Path>> {
        match self {
            LwjglLibraryPath::None => auto.clone(),
            LwjglLibraryPath::Auto(path) => {
                if path.exists() {
                    Some(path)
                } else {
                    auto.clone()
                }
            },
            LwjglLibraryPath::Explicit(path) => {
                Some(path)
            },
        }
    }

    pub fn get_path(&self) -> Option<&Path> {
        match self {
            LwjglLibraryPath::None => None,
            LwjglLibraryPath::Auto(path) => Some(&**path),
            LwjglLibraryPath::Explicit(path) => Some(&**path),
        }
    }
}
