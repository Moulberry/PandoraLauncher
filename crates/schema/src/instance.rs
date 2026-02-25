use std::{path::Path, sync::Arc};

use once_cell::sync::Lazy;
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
    #[serde(default, deserialize_with = "crate::try_deserialize", skip_serializing_if = "is_default_wrapper_command_configuration")]
    pub wrapper_command: Option<InstanceWrapperCommandConfiguration>,
    #[serde(default, deserialize_with = "crate::try_deserialize", skip_serializing_if = "is_default_jvm_flags_configuration")]
    pub jvm_flags: Option<InstanceJvmFlagsConfiguration>,
    #[serde(default, deserialize_with = "crate::try_deserialize", skip_serializing_if = "is_default_jvm_binary_configuration")]
    pub jvm_binary: Option<InstanceJvmBinaryConfiguration>,
    #[serde(default, deserialize_with = "crate::try_deserialize", skip_serializing_if = "is_default_linux_wrapper_configuration")]
    pub linux_wrapper: Option<InstanceLinuxWrapperConfiguration>,
    #[serde(default, deserialize_with = "crate::try_deserialize", skip_serializing_if = "is_default_system_libraries_configuration")]
    pub system_libraries: Option<InstanceSystemLibrariesConfiguration>,
    #[serde(default, deserialize_with = "crate::try_deserialize", skip_serializing_if = "crate::skip_if_none")]
    pub instance_fallback_icon: Option<Ustr>,
    #[serde(default, deserialize_with = "crate::try_deserialize")]
    pub disable_file_syncing: bool,
}

impl InstanceConfiguration {
    pub fn new(minecraft_version: Ustr, loader: Loader) -> Self {
        Self {
            minecraft_version,
            loader,
            preferred_loader_version: None,
            memory: None,
            wrapper_command: None,
            jvm_flags: None,
            jvm_binary: None,
            linux_wrapper: None,
            system_libraries: None,
            instance_fallback_icon: None,
            disable_file_syncing: false,
        }
    }
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
pub struct InstanceWrapperCommandConfiguration {
    pub enabled: bool,
    pub flags: Arc<str>,
}

fn is_default_wrapper_command_configuration(config: &Option<InstanceWrapperCommandConfiguration>) -> bool {
    if let Some(config) = config {
        !config.enabled && config.flags.trim_ascii().is_empty()
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct InstanceLinuxWrapperConfiguration {
    #[serde(default, deserialize_with = "crate::try_deserialize")]
    pub use_mangohud: bool,
    #[serde(default, deserialize_with = "crate::try_deserialize")]
    pub use_gamemode: bool,
    #[serde(default = "crate::default_true", deserialize_with = "crate::try_deserialize")]
    pub use_discrete_gpu: bool,
    #[serde(default, deserialize_with = "crate::try_deserialize")]
    pub disable_gl_threaded_optimizations: bool,
}

impl Default for InstanceLinuxWrapperConfiguration {
    fn default() -> Self {
        Self {
            use_mangohud: false,
            use_gamemode: false,
            use_discrete_gpu: true,
            disable_gl_threaded_optimizations: false
        }
    }
}

fn is_default_linux_wrapper_configuration(config: &Option<InstanceLinuxWrapperConfiguration>) -> bool {
    if let Some(config) = config {
        !config.use_mangohud && !config.use_gamemode && config.use_discrete_gpu && !config.disable_gl_threaded_optimizations
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
    Auto,
    AutoPreferred(Arc<Path>),
    Explicit(Arc<Path>),
}

fn is_default_system_libraries_configuration(config: &Option<InstanceSystemLibrariesConfiguration>) -> bool {
    if let Some(config) = config {
        matches!(config.glfw, LwjglLibraryPath::Auto) && matches!(config.openal, LwjglLibraryPath::Auto)
    } else {
        true
    }
}

impl LwjglLibraryPath {
    pub fn get_or_auto(self, auto: &Option<Arc<Path>>) -> Option<Arc<Path>> {
        match self {
            LwjglLibraryPath::Auto => auto.clone(),
            LwjglLibraryPath::AutoPreferred(preferred) => {
                if preferred.exists() {
                    Some(preferred)
                } else {
                    auto.clone()
                }
            },
            LwjglLibraryPath::Explicit(path) => {
                Some(path)
            },
        }
    }
}

pub static AUTO_LIBRARY_PATH_GLFW: Lazy<Option<Arc<Path>>> = Lazy::new(|| get_shared_library_path_for_name("glfw"));
pub static AUTO_LIBRARY_PATH_OPENAL: Lazy<Option<Arc<Path>>> = Lazy::new(|| get_shared_library_path_for_name("openal"));

#[cfg(not(unix))]
fn get_shared_library_path_for_name(name: &str) -> Option<Arc<Path>> {
    None
}

#[cfg(unix)]
fn get_shared_library_path_for_name(name: &str) -> Option<Arc<Path>> {
    let filename = format!("{}{}{}", std::env::consts::DLL_PREFIX, name, std::env::consts::DLL_SUFFIX);

    let search_paths = &[
        "/lib/",
        "/lib64/",
        "/usr/lib/",
        "/usr/lib64/",
        "/usr/local/lib/",
        #[cfg(target_os = "macos")]
        "/opt/homebrew/lib/"
    ];

    for search_path in search_paths {
        let path = Path::new(search_path).join(&filename);
        if path.exists() {
            return Some(path.into());
        }
    }

    None
}
