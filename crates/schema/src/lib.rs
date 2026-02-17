use serde::Deserialize;

pub mod assets_index;
pub mod auxiliary;
pub mod backend_config;
pub mod content;
pub mod fabric_launch;
pub mod fabric_loader_manifest;
pub mod fabric_mod;
pub mod forge;
pub mod forge_mod;
pub mod instance;
pub mod java_runtime_component;
pub mod java_runtimes;
pub mod loader;
pub mod maven;
pub mod modification;
pub mod modrinth;
pub mod mrpack;
pub mod pandora_update;
pub mod resourcepack;
pub mod version;
pub mod version_manifest;
pub mod syncing;

pub fn try_deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'de> + Default,
    D: serde::Deserializer<'de>,
{
    Ok(T::deserialize(serde_json::Value::deserialize(deserializer)?).unwrap_or_default())
}

fn skip_if_default<T: Default + PartialEq>(value: &T) -> bool {
    value == &T::default()
}

fn skip_if_none<T>(value: &Option<T>) -> bool {
    value.is_none()
}

fn default_true() -> bool {
    true
}
