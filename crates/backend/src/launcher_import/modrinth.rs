use std::{io::Cursor, path::{Path, PathBuf}, sync::Arc};

use bridge::{import::ImportFromOtherLauncherJob, modal_action::{ModalAction, ProgressTracker}};
use image::ImageFormat;
use rustc_hash::FxHashMap;
use schema::{instance::InstanceConfiguration, loader::Loader};

use crate::BackendState;

struct ModrinthInstanceToImport {
    pandora_path: PathBuf,
    instance_configuration: InstanceConfiguration,
    icon_path: Option<String>,
    minecraft_folder: Arc<Path>,
}

pub fn import_instances_from_modrinth(backend: &BackendState, import_job: ImportFromOtherLauncherJob, modal_action: &ModalAction) -> rusqlite::Result<()> {
    if import_job.paths.is_empty() {
        return Ok(());
    }

    let all_tracker = ProgressTracker::new("Importing instances".into(), backend.send.clone());
    modal_action.trackers.push(all_tracker.clone());
    all_tracker.notify();

    let app_db = import_job.root.join("app.db");
    if !app_db.exists() {
        return Ok(());
    }

    let conn = rusqlite::Connection::open(app_db)?;

    let mut stmt = conn.prepare("SELECT path, icon_path, game_version, mod_loader FROM profiles")?;
    let mut query = stmt.query([])?;

    let mut to_import = Vec::new();

    let mut name_to_path = FxHashMap::default();
    for path in import_job.paths.iter() {
        let Some(file_name) = path.file_name() else {
            continue;
        };
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        name_to_path.insert(file_name.to_string(), path.clone());
    }

    while let Ok(Some(row)) = query.next() {
        let filename: String = row.get(0)?;

        let pandora_path = backend.directories.instances_dir.join(&filename);
        if pandora_path.exists() {
           continue;
        }

        let Some(profile) = name_to_path.get(&filename) else {
            continue;
        };
        if !profile.is_dir() {
            continue;
        }

        let icon_path: Option<String> = row.get(1)?;
        let game_version: String = row.get(2)?;
        let mod_loader: String = row.get(3)?;

        let loader = Loader::from_name(&mod_loader).unwrap_or(Loader::Vanilla);

        let instance_configuration = InstanceConfiguration::new(game_version.into(), loader);

        to_import.push(ModrinthInstanceToImport {
            pandora_path,
            instance_configuration,
            icon_path,
            minecraft_folder: profile.clone(),
        });
    }

    all_tracker.set_total(to_import.len());

    for to_import in to_import {
        let title = format!("Importing {}", to_import.pandora_path.file_name().unwrap().to_string_lossy());
        let tracker = ProgressTracker::new(title.into(), backend.send.clone());
        modal_action.trackers.push(tracker.clone());
        tracker.notify();

        let Ok(configuration_bytes) = serde_json::to_vec(&to_import.instance_configuration) else {
            tracker.set_finished(bridge::modal_action::ProgressTrackerFinishType::Error);
            tracker.notify();
            continue;
        };

        _ = std::fs::create_dir_all(&to_import.pandora_path);

        // Copy .minecraft folder
        let target_dot_minecraft = to_import.pandora_path.join(".minecraft");

        _ = std::fs::create_dir_all(&target_dot_minecraft);
        _ = crate::copy_content_recursive(&to_import.minecraft_folder, &target_dot_minecraft, false, &|copied, total| {
            tracker.set_total(total as usize);
            tracker.set_count(copied as usize);
            tracker.notify();
        });

        // Copy icon
        if let Some(icon_path) = to_import.icon_path {
            let icon_path = Path::new(&icon_path);

            if let Ok(icon_bytes) = std::fs::read(icon_path) {
                if let Ok(format) = image::guess_format(&icon_bytes) {
                    if format == ImageFormat::Png {
                        _ = crate::write_safe(&to_import.pandora_path.join("icon.png"), &icon_bytes);
                    } else if let Ok(image) = image::load_from_memory_with_format(&icon_bytes, format) {
                        let mut png_bytes = Vec::new();
                        let mut cursor = Cursor::new(&mut png_bytes);
                        if image.write_to(&mut cursor, image::ImageFormat::Png).is_ok() {
                            _ = crate::write_safe(&to_import.pandora_path.join("icon.png"), &png_bytes);
                        }
                    }
                }
            }
        }

        // Write info_v1.json
        let info_path = to_import.pandora_path.join("info_v1.json");
        _ = crate::write_safe(&info_path, &configuration_bytes);

        all_tracker.add_count(1);
        all_tracker.notify();

        tracker.set_finished(bridge::modal_action::ProgressTrackerFinishType::Fast);
        tracker.notify();
    }

    all_tracker.set_finished(bridge::modal_action::ProgressTrackerFinishType::Normal);
    all_tracker.notify();

    Ok(())
}

pub fn read_profiles_from_modrinth_db(modrinth: &Path) -> rusqlite::Result<Option<Vec<Arc<Path>>>> {
    let app_db = modrinth.join("app.db");

    if !app_db.exists() {
        log::warn!("app.db doesn't exist in modrinth folder");
        return Ok(None);
    }

    let conn = rusqlite::Connection::open(app_db)?;

    let mut stmt = conn.prepare("SELECT path FROM profiles")?;
    let mut query = stmt.query([])?;

    let mut paths = Vec::new();

    let profiles = modrinth.join("profiles");
    while let Ok(Some(row)) = query.next() {
        let path: String = row.get(0)?;
        let profile = profiles.join(path);
        if profile.is_dir() {
            paths.push(profile.into());
        } else {
            log::warn!("Modrinth profile folder {:?} doesn't exist", profile);
        }
    }

    Ok(Some(paths))
}
