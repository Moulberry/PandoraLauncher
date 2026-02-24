use std::{collections::BTreeMap, path::{Path, PathBuf}, sync::Arc, time::SystemTime};

use bridge::{message::{SyncState, SyncTargetState}, safe_path::SafePath};
use once_cell::sync::Lazy;
use relative_path::PathExt;
use rustc_hash::FxHashMap;
use schema::backend_config::SyncTargets;

use crate::{directories::LauncherDirectories, BackendStateInstances};

pub fn apply_to_instance(sync_targets: &SyncTargets, directories: &LauncherDirectories, dot_minecraft: Arc<Path>) {
    _ = std::fs::create_dir_all(&dot_minecraft);

    let mut dir_iterator = walkdir::WalkDir::new(&dot_minecraft).into_iter();
    while let Some(Ok(entry)) = dir_iterator.next() {
        if entry.file_type().is_dir() {
            let Ok(relative) = entry.path().relative_to(&dot_minecraft) else {
                dir_iterator.skip_current_dir();
                continue;
            };
            if sync_targets.folders.contains(relative.as_str()) {
                dir_iterator.skip_current_dir();
                continue;
            }
            let Some(safe_relative) = SafePath::from_relative_path(&relative) else {
                dir_iterator.skip_current_dir();
                continue;
            };
            let target_dir = safe_relative.to_path(&directories.synced_dir);
            if !target_dir.is_dir() {
                dir_iterator.skip_current_dir();
                continue;
            }

            #[cfg(windows)]
            {
                let Ok(target) = junction::get_target(entry.path()) else {
                    continue;
                };

                if target.starts_with(&directories.synced_dir) {
                    dir_iterator.skip_current_dir();
                    _ = junction::delete(entry.path());
                    continue;
                }
            }
        }

        #[cfg(unix)]
        if entry.file_type().is_symlink() {
            let Ok(relative) = entry.path().relative_to(&dot_minecraft) else {
                continue;
            };
            if sync_targets.folders.contains(relative.as_str()) {
                continue;
            }
            let Ok(target) = std::fs::read_link(entry.path()) else {
                continue;
            };

            if target.starts_with(&directories.synced_dir) {
                _ = std::fs::remove_file(entry.path());
            }
        }
    }

    for file_target in sync_targets.files.iter() {
        if &**file_target == "options.txt" {
            let fallback = &directories.synced_dir.join("fallback_options.txt");
            let target = dot_minecraft.join("options.txt");
            let combined = create_combined_options_txt(fallback, &target, directories);
            _ = crate::write_safe(&fallback, combined.as_bytes());
            _ = crate::write_safe(&target, combined.as_bytes());
        } else if let Some(path) = SafePath::new(file_target) {
            if let Some(latest) = find_latest(&path, directories) {
                let target = path.to_path(&dot_minecraft);
                if latest != target {
                    if let Some(parent) = target.parent() {
                        _ = std::fs::create_dir_all(parent);
                    }
                    _ = std::fs::copy(latest, target);
                }
            }
        } else {
            log::warn!("Skipping file sync target because it is not a safe path: {}", file_target);
        }
    }

    for folder_target in sync_targets.folders.iter() {
        let Some(path) = SafePath::new(folder_target) else {
            log::warn!("Skipping folder sync target because it is not a safe path: {}", folder_target);
            continue;
        };

        let target_dir = path.to_path(&directories.synced_dir);
        let path = path.to_path(&dot_minecraft);

        if !path.exists() {
            _ = std::fs::create_dir_all(&target_dir);
            if let Some(parent) = path.parent() {
                _ = std::fs::create_dir_all(parent);
            }
            _ = linking::link_dir(&target_dir, &path);
        }
    }
}

fn find_latest(filename: &SafePath, directories: &LauncherDirectories) -> Option<PathBuf> {
    let mut latest_time = SystemTime::UNIX_EPOCH;
    let mut latest_path = None;

    let read_dir = std::fs::read_dir(&directories.instances_dir).ok()?;

    for entry in read_dir {
        let Ok(entry) = entry else {
            continue;
        };

        let path = filename.to_path(&entry.path().join(".minecraft"));

        if let Ok(metadata) = std::fs::metadata(&path) {
            let mut time = SystemTime::UNIX_EPOCH;

            if let Ok(created) = metadata.created() {
                time = time.max(created);
            }
            if let Ok(modified) = metadata.modified() {
                time = time.max(modified);
            }

            if latest_path.is_none() || time > latest_time {
                latest_time = time;
                latest_path = Some(path);
            }
        }
    }

    latest_path
}

fn create_combined_options_txt(fallback: &Path, current: &Path, directories: &LauncherDirectories) -> String {
    let mut values = read_options_txt(fallback);

    let Ok(read_dir) = std::fs::read_dir(&directories.instances_dir) else {
        return create_options_txt(values);
    };

    let mut paths = Vec::new();

    for entry in read_dir {
        let Ok(entry) = entry else {
            continue;
        };

        let mut path = entry.path();
        path.push(".minecraft");
        path.push("options.txt");

        let mut time = SystemTime::UNIX_EPOCH;

        if let Ok(metadata) = std::fs::metadata(&path) {
            if let Ok(created) = metadata.created() {
                time = time.max(created);
            }
            if let Ok(modified) = metadata.modified() {
                time = time.max(modified);
            }
        }

        paths.push((time, path));
    }

    paths.sort_by_key(|(time, _)| *time);

    for (_, path) in paths {
        let mut new_values = read_options_txt(&path);

        if path != current {
            new_values.remove("resourcePacks");
            new_values.remove("incompatibleResourcePacks");
        }

        for (key, value) in new_values {
            values.insert(key, value);
        }
    }

    create_options_txt(values)
}

fn create_options_txt(values: FxHashMap<String, String>) -> String {
    let mut options = String::new();

    for (key, value) in values {
        options.push_str(&key);
        options.push(':');
        options.push_str(&value);
        options.push('\n');
    }

    options
}

fn read_options_txt(path: &Path) -> FxHashMap<String, String> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return FxHashMap::default();
    };

    let mut values = FxHashMap::default();
    for line in content.split('\n') {
        let line = line.trim_ascii();
        if let Some((key, value)) = line.split_once(':') {
            values.insert(key.to_string(), value.to_string());
        }
    }
    values
}

pub fn get_sync_state(sync_targets: &SyncTargets, instances: &mut BackendStateInstances, directories: &LauncherDirectories) -> std::io::Result<SyncState> {
    let mut dot_minecraft_paths = Vec::new();

    for instance in instances.instances.iter_mut() {
        if !instance.configuration.get().disable_file_syncing {
            dot_minecraft_paths.push(instance.dot_minecraft_path.clone());
        }
    }

    let total = dot_minecraft_paths.len();
    let mut entries = BTreeMap::default();

    for file_target in sync_targets.files.iter() {
        if let Some(safe_file_target) = SafePath::new(file_target) {
            let mut cannot_sync_count = 0;

            for dot_minecraft in &dot_minecraft_paths {
                let target = safe_file_target.to_path(dot_minecraft);
                if target.is_dir() {
                    cannot_sync_count += 1;
                }
            }

            entries.insert(file_target.clone(), SyncTargetState {
                enabled: true,
                is_file: true,
                sync_count: total.saturating_sub(cannot_sync_count),
                cannot_sync_count,
            });
        } else {
            entries.insert(file_target.clone(), SyncTargetState {
                enabled: true,
                is_file: true,
                sync_count: 0,
                cannot_sync_count: total,
            });
        }
    }

    let mut disabled = Vec::new();
    for default_folder in DEFAULT_FOLDERS.iter() {
        if !sync_targets.folders.contains(default_folder) {
            disabled.push(default_folder.clone());
        }
    }

    let enabled_iter = sync_targets.folders.iter().map(|f| (f, true));
    let disabled_iter = disabled.iter().map(|f| (f, false));

    for (folder_target, enabled) in enabled_iter.chain(disabled_iter) {
        let Some(safe_path) = SafePath::new(folder_target) else {
            entries.insert(folder_target.clone(), SyncTargetState {
                enabled,
                is_file: false,
                sync_count: 0,
                cannot_sync_count: total,
            });
            continue;
        };

        let target_dir = safe_path.to_path(&directories.synced_dir);

        let mut sync_count = 0;
        let mut cannot_sync_count = 0;

        for dot_minecraft in &dot_minecraft_paths {
            let path = safe_path.to_path(dot_minecraft);

            if linking::is_targeting(&target_dir, &path) {
                sync_count += 1;
            } else if path.exists() {
                cannot_sync_count += 1;
            }
        }

        entries.insert(folder_target.clone(), SyncTargetState {
            enabled,
            is_file: false,
            sync_count,
            cannot_sync_count,
        });
    }

    Ok(SyncState {
        sync_folder: directories.synced_dir.clone(),
        targets: entries,
        total_count: total,
    })
}

static DEFAULT_FOLDERS: Lazy<Vec<Arc<str>>> = Lazy::new(|| {
    [
        "saves",
        "config",
        "screenshots",
        "resourcepacks",
        "shaderpacks",
        "flashback",
        "Distant_Horizons_server_data",
        ".voxy",
        "xaero",
        ".bobby",
        "schematics",
    ].into_iter().map(Arc::from).collect()
});

pub fn enable_all(name: &str, is_file: bool, instances: &mut BackendStateInstances, directories: &LauncherDirectories) -> std::io::Result<bool> {
    if is_file {
        return Ok(true);
    }

    let Some(safe_path) = SafePath::new(name) else {
        log::warn!("Skipping folder sync because it is not a safe path: {}", name);
        return Ok(false);
    };

    let mut paths = Vec::new();
    for instance in instances.instances.iter_mut() {
        if !instance.configuration.get().disable_file_syncing {
            paths.push(safe_path.to_path(&instance.dot_minecraft_path));
        }
    }

    let target_dir = safe_path.to_path(&directories.synced_dir);

    // Exclude links that already point to target_dir
    paths.retain(|path| {
        !linking::is_targeting(&target_dir, &path)
    });

    for path in &paths {
        if path.exists() {
            return Ok(false);
        }
    }

    std::fs::create_dir_all(&target_dir)?;
    for path in &paths {
        if let Some(parent) = path.parent() {
            _ = std::fs::create_dir_all(parent);
        }
        linking::link_dir(&target_dir, path)?;
    }

    Ok(true)
}

pub fn disable_all(name: &str, is_file: bool, directories: &LauncherDirectories) -> std::io::Result<()> {
    if is_file {
        return Ok(());
    }

    let Some(safe_path) = SafePath::new(name) else {
        log::warn!("Skipping folder sync because it is not a safe path: {}", name);
        return Ok(());
    };

    let mut paths = Vec::new();
    let read_dir = std::fs::read_dir(&directories.instances_dir)?;
    for entry in read_dir {
        paths.push(safe_path.to_path(&entry?.path().join(".minecraft")));
    }

    let target_dir = safe_path.to_path(&directories.synced_dir);

    for path in &paths {
        linking::unlink_dir_if_targeting(&target_dir, path)?;
    }

    Ok(())
}

#[cfg(unix)]
mod linking {
    use std::path::Path;

    pub fn link_dir(original: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(original, link)
    }

    pub fn is_targeting(original: &Path, link: &Path) -> bool {
        let Ok(target) = std::fs::read_link(link) else {
            return false;
        };

        target == original
    }

    pub fn unlink_dir_if_targeting(original: &Path, link: &Path) -> std::io::Result<()> {
        let Ok(target) = std::fs::read_link(link) else {
            return Ok(());
        };

        if target == original {
            std::fs::remove_file(link)?;
        }

        Ok(())
    }
}

#[cfg(windows)]
mod linking {
    use std::path::Path;

    pub fn link_dir(original: &Path, link: &Path) -> std::io::Result<()> {
        junction::create(original, link)
    }

    pub fn is_targeting(original: &Path, link: &Path) -> bool {
        let Ok(target) = junction::get_target(link) else {
            return false;
        };

        target == original
    }

    pub fn unlink_dir_if_targeting(original: &Path, link: &Path) -> std::io::Result<()> {
        let Ok(target) = junction::get_target(link) else {
            return Ok(());
        };

        if target == original {
            junction::delete(link)?;
        }

        Ok(())
    }
}
