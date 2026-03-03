use std::{io::Cursor, path::{Path, PathBuf}};

use bridge::{import::ImportFromOtherLauncher, modal_action::{ModalAction, ProgressTracker}, safe_path::SafePath};
use image::ImageFormat;
use schema::{instance::InstanceConfiguration, loader::Loader};

use crate::BackendState;


struct ModrinthInstanceToImport {
    pandora_path: PathBuf,
    instance_configuration: InstanceConfiguration,
    icon_path: Option<String>,
    minecraft_folder: PathBuf,
}

pub fn import_instances_from_modrinth(backend: &BackendState, modrinth: &Path, modal_action: &ModalAction) -> rusqlite::Result<()> {
    let all_tracker = ProgressTracker::new("Importing instances".into(), backend.send.clone());
    modal_action.trackers.push(all_tracker.clone());
    all_tracker.notify();

    let profiles = modrinth.join("profiles");
    let app_db = modrinth.join("app.db");

    if !app_db.exists() {
        return Ok(());
    }

    let conn = rusqlite::Connection::open(app_db)?;

    let mut stmt = conn.prepare("SELECT path, icon_path, game_version, mod_loader FROM profiles")?;
    let mut query = stmt.query([])?;

    let mut to_import = Vec::new();

    while let Ok(Some(row)) = query.next() {
        let path: String = row.get(0)?;

        if SafePath::new(&path).is_none() {
            modal_action.set_error_message(format!("Refusing to load instance with illegal path: {}", path).into());
            return Ok(());
        }

        let profile = profiles.join(&path);
        if !profile.is_dir() {
            continue;
        }

        let icon_path: Option<String> = row.get(1)?;
        let game_version: String = row.get(2)?;
        let mod_loader: String = row.get(3)?;

        let mut loader = Loader::from_name(&mod_loader);
        if loader == Loader::Unknown {
            loader = Loader::Vanilla;
        }

        let instance_configuration = InstanceConfiguration::new(game_version.into(), loader);

        to_import.push(ModrinthInstanceToImport {
            pandora_path: backend.directories.instances_dir.join(path),
            instance_configuration,
            icon_path,
            minecraft_folder: profile,
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
        let copy_options = fs_extra::dir::CopyOptions::default().copy_inside(true);
        _ = fs_extra::dir::copy_with_progress(to_import.minecraft_folder, target_dot_minecraft, &copy_options, |state| {
            tracker.set_total(state.total_bytes as usize);
            tracker.set_count(state.copied_bytes as usize);
            tracker.notify();
            fs_extra::dir::TransitProcessResult::ContinueOrAbort
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

pub fn read_profiles_from_modrinth_db(data_dir: &Path) -> rusqlite::Result<Option<ImportFromOtherLauncher>> {
    let modrinth = data_dir.join("ModrinthApp");
    let profiles = modrinth.join("profiles");
    let app_db = modrinth.join("app.db");

    if !app_db.exists() {
        return Ok(None);
    }

    let conn = rusqlite::Connection::open(app_db)?;

    let mut stmt = conn.prepare("SELECT path FROM profiles")?;
    let mut query = stmt.query([])?;

    let mut paths = Vec::new();

    while let Ok(Some(row)) = query.next() {
        let path: String = row.get(0)?;
        let profile = profiles.join(path);
        if profile.is_dir() {
            paths.push(profile);
        }
    }

    Ok(Some(ImportFromOtherLauncher {
        can_import_accounts: false,
        paths,
    }))
}
