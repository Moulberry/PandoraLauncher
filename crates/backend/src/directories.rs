use std::{path::{Path, PathBuf}, sync::Arc};

pub struct LauncherDirectories {
    pub instances_dir: Arc<Path>,

    pub synced_dir: Arc<Path>,
    pub skin_library_dir: Arc<Path>,

    pub metadata_dir: Arc<Path>,

    pub assets_root_dir: Arc<Path>,
    pub assets_index_dir: Arc<Path>,
    pub assets_objects_dir: Arc<Path>,
    pub virtual_legacy_assets_dir: Arc<Path>,

    pub libraries_dir: Arc<Path>,
    pub log_configs_dir: Arc<Path>,
    pub runtime_base_dir: Arc<Path>,

    pub content_library_dir: Arc<Path>,
    pub content_meta_dir: Arc<Path>,

    pub temp_dir: Arc<Path>,
    pub temp_natives_base_dir: Arc<Path>,

    pub root_launcher_dir: Arc<Path>,
    pub config_json: Arc<Path>,
    pub accounts_json: Arc<Path>,
}

impl LauncherDirectories {
    pub fn new(launcher_dir: PathBuf) -> Self {
        let instances_dir = launcher_dir.join("instances");

        let synced_dir = launcher_dir.join("synced");
        let skin_library_dir = launcher_dir.join("skins");

        let metadata_dir = launcher_dir.join("metadata");

        let assets_root_dir = launcher_dir.join("assets");
        let assets_index_dir = assets_root_dir.join("indexes");
        let assets_objects_dir = assets_root_dir.join("objects");
        let virtual_legacy_assets_dir = assets_index_dir.join("virtual").join("legacy");

        let libraries_dir = launcher_dir.join("libraries");

        let log_configs_dir = launcher_dir.join("logconfigs");

        let runtime_base_dir = launcher_dir.join("runtime");

        let content_library_dir = launcher_dir.join("contentlibrary");
        let content_meta_dir = launcher_dir.join("contentmeta");

        let temp_dir = launcher_dir.join("temp");
        let temp_natives_base_dir = temp_dir.join("natives");

        let config_json = launcher_dir.join("config.json");
        let accounts_json = launcher_dir.join("accounts.json");

        Self {
            instances_dir: instances_dir.into(),

            synced_dir: synced_dir.into(),
            skin_library_dir: skin_library_dir.into(),

            metadata_dir: metadata_dir.into(),

            assets_root_dir: assets_root_dir.into(),
            assets_index_dir: assets_index_dir.into(),
            assets_objects_dir: assets_objects_dir.into(),
            virtual_legacy_assets_dir: virtual_legacy_assets_dir.into(),

            libraries_dir: libraries_dir.into(),
            log_configs_dir: log_configs_dir.into(),
            runtime_base_dir: runtime_base_dir.into(),

            content_library_dir: content_library_dir.into(),
            content_meta_dir: content_meta_dir.into(),

            temp_dir: temp_dir.into(),
            temp_natives_base_dir: temp_natives_base_dir.into(),

            root_launcher_dir: launcher_dir.into(),
            config_json: config_json.into(),
            accounts_json: accounts_json.into(),
        }
    }
}
