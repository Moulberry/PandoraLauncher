use std::path::{Path, PathBuf};
use bridge::modal_action::{ModalAction, ProgressTracker};
use schema::{assets_index::{AssetObject, AssetsIndex}, modrinth::{ModrinthHit, ModrinthProjectVersion}, version::{AssetIndexLink, GameDownloads, GameLibrary, GameLogging, JavaVersion, LaunchArguments}};
use serde::Deserialize;
use uuid::Uuid;
use crate::BackendState;

/// Going to just get the types converted before deleting a bunch probably...
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtLauncherInstance {
    uuid: Uuid,
    launcher: Launcher,
    id: String,
    compliance_level: usize,
    java_version: JavaVersion,
    arguments: LaunchArguments,
    #[serde(rename = "typ")]
    modpack_type: String,
    time: String,
    release_time: String,
    minimum_launcher_version: String,
    asset_index: AssetIndexLink,
    assets: String,
    downloads: Vec<GameDownloads>,
    logging: GameLogging,
    libraries: GameLibrary
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Launcher {
    name: String,
    pack: String,
    description: String,
    pack_id: usize,
    external_pack_id: usize,
    version: String,
    enable_curse_forge_integration: bool,
    enable_editing_mods: bool,
    loader_version: LoaderVersion,
    required_memory: usize,
    required_perm_gen: usize,
    quick_play: QuickPlay,
    is_dev: bool,
    is_playable: bool,
    assets_map_to_resources: bool,
    curse_forge_project: Option<CurseForgeProject>,
    curse_forge_project_description: Option<String>,
    curse_forge_file: Option<CurseForgeFile>,
    override_paths: Vec<String>,
    check_for_updates: bool,
    mods: Vec<Mod>,
    ignored_updates: Vec<String>,
    ignore_all_updates: bool,
    vanilla_instance: bool,
    last_played: usize,
    num_plays: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoaderVersion {
    version: String,
    raw_version: String,
    recommended: bool,
    #[serde(rename = "type")]
    loader_type: String,
    // downloadables: Vec<>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuickPlay {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeCategory {
	name: String,
	slug: String,
	url: String,
	date_modified: String,
	game_id: usize,
	is_class: bool,
	id: usize,
	icon_url: String,
	parent_category_id: usize,
	class_id: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeProject {
    id: usize,
    #[serde(rename = "name")]
    project_name: String,
    authors: Vec<CurseForgeAuthor>,
    game_id: usize,
    summary: String,
    categories: Vec<CurseForgeCategory>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeAuthor {
    id: usize,
    name: String,
    url: String,
}


#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeFileDependency {
	file_id: usize,
	mod_id: usize,
	relation_typee: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeFileModule {
	fingerprint: usize,
	name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeFileHash {
	value: String,
	algo: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SortableGameVersion {
	game_version_padded: String,
	game_version: String,
	game_version_release_date: String,
	game_version_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeFile {
	id: usize,
	game_id: usize,
	is_available: bool,
	display_name: String,
	file_name: String,
	release_type: usize,
	file_status: usize,
	file_date: String,
	file_length: usize,
	dependencies: Vec<CurseForgeFileDependency>,
	alternate_file_id: usize,
	modules: Vec<CurseForgeFileModule>,
	is_server_pack: bool,
	hashes: Vec<CurseForgeFileHash>,
	sortable_game_versions: Vec<SortableGameVersion>,
	game_versions: Vec<String>,
	file_fingerprint: usize,
	mod_id: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Mod {
    name: String,
    version: String,
    optional: bool,
    file: String,
    #[serde(rename = "type")]
    mod_type: String,
    description: String,
    disabled: bool,
    user_added: bool,
    was_selected: bool,
    skipped: bool,
    curse_forge_project_id: Option<usize>,
    curse_forge_file_id: Option<usize>,
    curse_forge_project: Option<CurseForgeProject>,
    curse_forge_file: Option<CurseForgeFile>,
    modrinth_project: Option<ModrinthHit>,
    modrinth_version: Option<ModrinthProjectVersion>
}






pub fn import_from_atlauncher(backend: &BackendState, path: &Path, import_accounts: bool, import_instance: bool, modal_action: ModalAction) {
	if import_accounts {
		import_accounts_from_atlauncher(backend, path, &modal_action);
	}
	if import_instance {
		import_instances_from_atlauncher(backend, path, &modal_action);
	}
}

fn import_accounts_from_atlauncher(backend: &BackendState, path: &Path, modal_action: &ModalAction) {
	// todo!();
	return;
}

struct AtLauncherInstanceToImport {
	pandora_path: PathBuf,
	atlauncher_instance_cfg: PathBuf,
	folder: PathBuf,
}

fn import_instances_from_atlauncher(backend: &BackendState, path: &Path, modal_action: &ModalAction) {
	let all_tracker = ProgressTracker::new("Importing instances".into(), backend.send.clone());
    modal_action.trackers.push(all_tracker.clone());
    all_tracker.notify();

    let Ok(read_dir) = std::fs::read_dir(path.join("instances")) else {
        all_tracker.set_finished(bridge::modal_action::ProgressTrackerFinishType::Error);
        all_tracker.notify();
        return;
    };

    let mut to_import = Vec::new();

    for entry in read_dir {
        let Ok(entry) = entry else {
            continue;
        };
        let folder = entry.path();
        if !folder.is_dir() {
            continue;
        }

        let Some(filename) = folder.file_name() else {
            continue;
        };

        let pandora_path = backend.directories.instances_dir.join(filename);
        if pandora_path.exists() {
           continue;
        }

        let atlauncher_instance_cfg = folder.join("instance.json");
        if !atlauncher_instance_cfg.exists() {
            continue;
        }

        to_import.push(AtLauncherInstanceToImport {
            pandora_path,
            atlauncher_instance_cfg,
            folder,
        });
    }

    all_tracker.set_total(to_import.len());
    all_tracker.set_finished(bridge::modal_action::ProgressTrackerFinishType::Normal);
    all_tracker.notify();
}
