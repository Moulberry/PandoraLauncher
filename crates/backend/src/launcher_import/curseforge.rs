use std::{path::{Path, PathBuf}, sync::Arc};

use bridge::{import::ImportFromOtherLauncherJob, modal_action::{ModalAction, ProgressTracker}};
use schema::{instance::{InstanceConfiguration, InstanceMemoryConfiguration}, loader::Loader};
use serde::Deserialize;

use crate::{BackendState, write_safe};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseforgeInstance {
    manifest: CurseforgeManifest,
    is_memory_override: bool,
    allocated_memory: u32,
    // memory_allocated_type: u8,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseforgeManifest {
    minecraft: CurseforgeManifestMinecraft,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseforgeManifestMinecraft {
    version: String,
    mod_loaders: Vec<CurseforgeModLoader>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseforgeModLoader {
    id: String,
    primary: bool,
}


fn try_load_from_curseforge(config_path: &Path) -> Option<InstanceConfiguration> {
    let instance_cfg_bytes = std::fs::read(config_path).ok()?;
    let instance_cfg = serde_json::from_slice::<CurseforgeInstance>(&instance_cfg_bytes).ok()?;

    let loader_details = instance_cfg.manifest.minecraft.mod_loaders.iter().filter(|loader| loader.primary).nth(0)?;
    let loader = Loader::from_name(&loader_details.id.split("-").next()?);
    let mut configuration = InstanceConfiguration::new(instance_cfg.manifest.minecraft.version.into(), loader);
    if instance_cfg.is_memory_override {
        configuration.memory = Some(InstanceMemoryConfiguration {
            enabled: true,
            min: 0,
            max: instance_cfg.allocated_memory,
        })
    }

    Some(configuration)
}

pub fn import_from_curseforge(backend: &BackendState, import_job: ImportFromOtherLauncherJob, modal_action: ModalAction) {
    import_instances_from_curseforge(backend, &import_job, &modal_action);
}

#[derive(Debug)]
struct CurseforgeInstanceToImport {
    pandora_path: PathBuf,
    config_path: PathBuf,
    folder: Arc<Path>,
}

pub fn import_instances_from_curseforge(backend: &BackendState, import_job: &ImportFromOtherLauncherJob, modal_action: &ModalAction) {
    if import_job.paths.is_empty() {
        return;
    }

    let all_tracker = ProgressTracker::new("Importing instances".into(), backend.send.clone());
    modal_action.trackers.push(all_tracker.clone());
    all_tracker.notify();

    let mut to_import = Vec::new();

    for folder in import_job.paths.iter() {
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

        let curseforge_config = folder.join("minecraftinstance.json");
        if !curseforge_config.exists() {
            continue;
        }

        to_import.push(CurseforgeInstanceToImport {
            pandora_path,
            config_path: curseforge_config,
            folder: folder.clone()
        });
    }

    all_tracker.set_total(to_import.len());

    for to_import in to_import {
        let title = format!("Importing {}", to_import.folder.file_name().unwrap().to_string_lossy());
        let tracker = ProgressTracker::new(title.into(), backend.send.clone());
        modal_action.trackers.push(tracker.clone());
        tracker.notify();

        let Some(configuration) = try_load_from_curseforge(&to_import.config_path) else {
            tracker.set_finished(bridge::modal_action::ProgressTrackerFinishType::Error);
            log::error!("Failed to load config path from curseforge for {:?}", to_import.folder.file_name().unwrap());
            tracker.notify();
            continue;
        };

        let Ok(configuration_bytes) = serde_json::to_vec(&configuration) else {
            tracker.set_finished(bridge::modal_action::ProgressTrackerFinishType::Error);
            tracker.notify();
            continue;
        };

        _ = std::fs::create_dir_all(&to_import.pandora_path);
        let target_dot_minecraft = to_import.pandora_path.join(".minecraft");

        let copy_options = fs_extra::dir::CopyOptions::default().copy_inside(true);
        _ = fs_extra::dir::copy_with_progress(to_import.folder, &target_dot_minecraft, &copy_options, |state| {
            tracker.set_total(state.total_bytes as usize);
            tracker.set_count(state.copied_bytes as usize);
            tracker.notify();

            fs_extra::dir::TransitProcessResult::ContinueOrAbort
        });

        // remove old configuration, rename icon path.
        // if this errors we just fall back on default icon, it's fine.
        if let Ok(mut logo_dir) = target_dot_minecraft.join("profileImage").read_dir() {
            if let Some(file_path) = logo_dir.next() {
                _ = std::fs::rename(&file_path.unwrap().path(), &to_import.pandora_path.join("icon.png"));
            }
        }
        _ = std::fs::remove_file(&target_dot_minecraft.join("minecraftinstance.json"));

        let info_path = to_import.pandora_path.join("info_v1.json");
        _ = write_safe(&info_path, &configuration_bytes);

        all_tracker.add_count(1);
        all_tracker.notify();

        tracker.set_finished(bridge::modal_action::ProgressTrackerFinishType::Fast);
        tracker.notify();
    }

}
