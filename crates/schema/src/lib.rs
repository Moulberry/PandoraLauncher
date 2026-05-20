use serde::Deserialize;

pub mod assets_index;
pub mod auxiliary;
pub mod backend_config;
pub mod content;
pub mod curseforge;
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
pub mod minecraft_profile;
pub mod modification;
pub mod modrinth;
pub mod mrpack;
pub mod pandora_update;
pub mod resourcepack;
pub mod server_status;
pub mod text_component;
pub mod unique_bytes;
pub mod version;
pub mod version_manifest;

pub fn try_deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'de> + Default,
    D: serde::Deserializer<'de>,
{
    Ok(T::deserialize(serde_json::Value::deserialize(deserializer)?).unwrap_or_default())
}

pub fn skip_if_default<T: Default + PartialEq>(value: &T) -> bool {
    value == &T::default()
}

pub fn skip_if_none<T>(value: &Option<T>) -> bool {
    value.is_none()
}

pub fn default_true() -> bool {
    true
}

pub fn single_or_seq<'de, T, D>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    if let Ok(value) = T::deserialize(value.clone()) {
        Ok(vec![value])
    } else if let Ok(value) = <Vec<T>>::deserialize(value) {
        Ok(value)
    } else {
        Ok(Vec::new())
    }
}
