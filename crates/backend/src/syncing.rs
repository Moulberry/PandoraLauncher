use std::{collections::HashSet, ffi::OsStr, fmt::Debug, path::{Component, Path, PathBuf}, sync::Arc, time::SystemTime};

use bridge::message::SyncState;
use enum_map::EnumMap;
use enumset::EnumSet;
use rustc_hash::FxHashMap;
use schema::{backend_config::SyncTarget, syncing::{ChildrenSync, CopyDeleteSync, CopySaveSync, CustomScriptSync, SymlinkSync, SyncLink, SyncType}};
use strum::IntoEnumIterator;

use crate::directories::LauncherDirectories;

pub trait Syncer: Debug + Send + Sync {
    fn link_inner(&self, source: PathBuf, target: PathBuf);
    fn unlink_inner(&self, source: PathBuf, target: PathBuf);
    fn get_link(&self) -> &SyncLink;
}

impl dyn Syncer {
    pub fn link(&self, source_prefix: &Path, target_prefix: &Path) {
        let Some(source) = to_absolute_path(source_prefix, &self.get_link().source) else { return; };
        let Some(target) = to_absolute_path(target_prefix, &self.get_link().target) else { return; };
        self.link_inner(source, target);
    }

    pub fn unlink(&self, source_prefix: &Path, target_prefix: &Path) {
        let Some(source) = to_absolute_path(source_prefix, &self.get_link().source) else { return; };
        let Some(target) = to_absolute_path(target_prefix, &self.get_link().target) else { return; };
        self.unlink_inner(source, target);
    }
}

impl Into<Box<dyn Syncer>> for SyncType {
    fn into(self) -> Box<dyn Syncer> {
        match self {
            SyncType::Symlink(sync) => Box::new(sync),
            SyncType::CopySave(sync) => Box::new(sync),
            SyncType::CopyDelete(sync) => Box::new(sync),
            SyncType::Children(sync) => Box::new(sync),
            SyncType::CustomScript(sync) => Box::new(sync),
        }
    }
}

fn to_absolute_path(prefix: &Path, suffix: &Path) -> Option<PathBuf> {
    if !suffix.is_relative() { return None; }
    if !prefix.is_absolute() { return None; }
    if suffix.components().any(|c| c == Component::ParentDir) { return None; }

    return Some(prefix.join(suffix))
}

impl Syncer for SymlinkSync {
    fn link_inner(&self, source: PathBuf, target: PathBuf) {
        _ = linking::link(&source, &target);
    }

    fn unlink_inner(&self, source: PathBuf, target: PathBuf) {
        _ = linking::unlink_if_targeting(&source, &target);
    }

    fn get_link(&self) -> &SyncLink {
        &self.link
    }
}

impl Syncer for CopySaveSync {
    fn link_inner(&self, source: PathBuf, target: PathBuf) {
        _ = std::fs::copy(&source, &target);
    }

    fn unlink_inner(&self, source: PathBuf, target: PathBuf) {
        _ = std::fs::copy(&target, &source);
    }

    fn get_link(&self) -> &SyncLink {
        &self.link
    }
}

impl Syncer for CopyDeleteSync {
    fn link_inner(&self, source: PathBuf, target: PathBuf) {
        _ = std::fs::copy(&source, &target);
    }

    fn unlink_inner(&self, source: PathBuf, target: PathBuf) {
        _ = std::fs::remove_file(&target);
    }

    fn get_link(&self) -> &SyncLink {
        &self.link
    }
}

fn source_to_target_path(keep_name: bool, source_path: &Path, link_target: &Path) -> Option<PathBuf> {
    let name = source_path.file_name().unwrap_or_else(|| OsStr::new(""));
    let target_base_path = link_target.join(&name);

    if keep_name {
        if target_base_path.try_exists().unwrap_or(true) { return None; }
        return Some(target_base_path)
    } else {
        let mut err_count: u8 = 0;
        loop {
            if err_count == 255 { return None; }

            let number = rand::random::<u32>();
            let target_path = target_base_path.with_added_extension(format!("{number:0>8x}.plsync"));

            if !target_path.try_exists().unwrap_or(true) { return Some(target_path); }
            err_count += 1;
        }
    }
}

impl Syncer for ChildrenSync {
    fn link_inner(&self, source: PathBuf, target: PathBuf) {
        if !source.is_dir() || !target.is_dir() { return; }

        let Ok(dir) = source.read_dir() else { return; };
        for entry in dir.flatten() {
            let source_path = entry.path();
            let Some(target_path) = source_to_target_path(self.keep_name, &source_path, &target) else { continue };

            _ = linking::link(&source_path, &target_path);
        }
    }

    fn unlink_inner(&self, source: PathBuf, target: PathBuf) {
        if !source.is_dir() || !target.is_dir() { return; }

        let mut sources: HashSet<PathBuf> = HashSet::new();

        let Ok(dir) = source.read_dir() else { return; };
        for entry in dir.flatten() {
            sources.insert(entry.path());
        }

        let Ok(dir) = target.read_dir() else { return; };
        for entry in dir.flatten() {
            let target_path = entry.path();

            if !target_path.is_symlink() { continue; }

            let Ok(source_path) = target_path.read_link() else { continue; };
            if !sources.contains(&source_path) { continue; }

            let target_path_no_extension = if !self.keep_name {
                let mut target_path = target_path.clone();

                let Some(extension) = target_path.extension() else { continue; };
                if extension != "plsync" { continue; };
                target_path.set_extension("");

                let Some(extension) = target_path.extension() else { continue; };
                if extension.len() != 8 { continue; };
                target_path.set_extension("");

                target_path
            } else {
                target_path.clone()
            };

            let Some(source_file_name) = source_path.file_name() else { continue; };
            let Some(target_file_name) = target_path_no_extension.file_name() else { continue; };
            if source_file_name != target_file_name { continue; }

            _ = std::fs::remove_file(target_path);
        }
    }

    fn get_link(&self) -> &SyncLink {
        &self.link
    }
}

impl Syncer for CustomScriptSync {
    fn link_inner(&self, source: PathBuf, target: PathBuf) {
        todo!()
    }

    fn unlink_inner(&self, source: PathBuf, target: PathBuf) {
        todo!()
    }

    fn get_link(&self) -> &SyncLink {
        &self.link
    }
}

pub fn apply_to_instance(sync_targets: EnumSet<SyncTarget>, directories: &LauncherDirectories, dot_minecraft: Arc<Path>) {
    _ = std::fs::create_dir_all(&dot_minecraft);

    for target in SyncTarget::iter() {
        let want = sync_targets.contains(target);

        if let Some(sync_folder) = target.get_folder() {
            let non_hidden_sync_folder = if sync_folder.starts_with(".") {
                &sync_folder[1..]
            } else {
                sync_folder
            };

            let target_dir = directories.synced_dir.join(non_hidden_sync_folder);

            let path = dot_minecraft.join(sync_folder);

            if want {
                if !path.exists() {
                    _ = linking::link(&target_dir, &path);
                }
            } else {
                _ = linking::unlink_if_targeting(&target_dir, &path);
            }
        } else if want {
            match target {
                SyncTarget::Options => {
                    let fallback = &directories.synced_dir.join("fallback_options.txt");
                    let target = dot_minecraft.join("options.txt");
                    let combined = create_combined_options_txt(fallback, &target, directories);
                    _ = crate::write_safe(&fallback, combined.as_bytes());
                    _ = crate::write_safe(&target, combined.as_bytes());
                },
                SyncTarget::Servers => {
                    if let Some(latest) = find_latest("servers.dat", directories) {
                        let target = dot_minecraft.join("servers.dat");
                        if latest != target {
                            _ = std::fs::copy(latest, target);
                        }
                    }
                },
                SyncTarget::Commands => {
                    if let Some(latest) = find_latest("command_history.txt", directories) {
                        let target = dot_minecraft.join("command_history.txt");
                        if latest != target {
                            _ = std::fs::copy(latest, target);
                        }
                    }
                },
                SyncTarget::Hotbars => {
                    if let Some(latest) = find_latest("hotbar.nbt", directories) {
                        let target = dot_minecraft.join("hotbar.nbt");
                        if latest != target {
                            _ = std::fs::copy(latest, target);
                        }
                    }
                },
                _ => {
                    log::error!("Don't know how to sync {target:?}")
                }
            }
        }
    }
}

fn find_latest(filename: &'static str, directories: &LauncherDirectories) -> Option<PathBuf> {
    let mut latest_time = SystemTime::UNIX_EPOCH;
    let mut latest_path = None;

    let read_dir = std::fs::read_dir(&directories.instances_dir).ok()?;

    for entry in read_dir {
        let Ok(entry) = entry else {
            continue;
        };

        let mut path = entry.path();
        path.push(".minecraft");
        path.push(filename);

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

pub fn get_sync_state(want_sync: EnumSet<SyncTarget>, directories: &LauncherDirectories) -> std::io::Result<SyncState> {
    let mut paths = Vec::new();

    let read_dir = std::fs::read_dir(&directories.instances_dir)?;
    for entry in read_dir {
        let mut path = entry?.path();
        path.push(".minecraft");
        paths.push(path);
    }

    let total = paths.len();
    let mut synced = EnumMap::default();
    let mut cannot_sync = EnumMap::default();

    for target in SyncTarget::iter() {
        let want = want_sync.contains(target);

        let Some(sync_folder) = target.get_folder() else {
            if want {
                synced[target] = total;
            }
            continue;
        };

        let non_hidden_sync_folder = if sync_folder.starts_with(".") {
            &sync_folder[1..]
        } else {
            sync_folder
        };

        let target_dir = directories.synced_dir.join(non_hidden_sync_folder);

        let mut synced_count = 0;
        let mut cannot_sync_count = 0;

        for path in &paths {
            let path = path.join(sync_folder);

            if linking::is_targeting(&target_dir, &path) {
                synced_count += 1;
            } else if path.exists() {
                cannot_sync_count += 1;
            }
        }

        synced[target] = synced_count;
        cannot_sync[target] = cannot_sync_count;
    }

    Ok(SyncState {
        sync_folder: Some(directories.synced_dir.clone()),
        want_sync,
        total,
        synced,
        cannot_sync
    })
}

pub fn enable_all(target: SyncTarget, directories: &LauncherDirectories) -> std::io::Result<bool> {
    let Some(sync_folder) = target.get_folder() else {
        return Ok(true);
    };

    let mut paths = Vec::new();

    let read_dir = std::fs::read_dir(&directories.instances_dir)?;
    for entry in read_dir {
        let mut path = entry?.path();
        path.push(".minecraft");
        path.push(sync_folder);
        paths.push(path);
    }

    let non_hidden_sync_folder = if sync_folder.starts_with(".") {
        &sync_folder[1..]
    } else {
        sync_folder
    };

    let target_dir = directories.synced_dir.join(non_hidden_sync_folder);

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
        linking::link(&target_dir, path)?;
    }

    Ok(true)
}

pub fn disable_all(target: SyncTarget, directories: &LauncherDirectories) -> std::io::Result<()> {
    let Some(sync_folder) = target.get_folder() else {
        return Ok(());
    };

    let mut paths = Vec::new();

    let read_dir = std::fs::read_dir(&directories.instances_dir)?;
    for entry in read_dir {
        let mut path = entry?.path();
        path.push(".minecraft");
        path.push(sync_folder);
        paths.push(path);
    }

    let non_hidden_sync_folder = if sync_folder.starts_with(".") {
        &sync_folder[1..]
    } else {
        sync_folder
    };

    let target_dir = directories.synced_dir.join(non_hidden_sync_folder);

    for path in &paths {
        linking::unlink_if_targeting(&target_dir, path)?;
    }

    Ok(())
}

#[cfg(unix)]
mod linking {
    use std::path::Path;

    pub fn link(original: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(original, link)
    }

    pub fn is_targeting(original: &Path, link: &Path) -> bool {
        let Ok(target) = std::fs::read_link(link) else {
            return false;
        };

        target == original
    }

    pub fn unlink_if_targeting(original: &Path, link: &Path) -> std::io::Result<()> {
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

    pub fn link(original: &Path, link: &Path) -> std::io::Result<()> {
        junction::create(original, link)
    }

    pub fn is_targeting(original: &Path, link: &Path) -> bool {
        let Ok(target) = junction::get_target(link) else {
            return false;
        };

        target == original
    }

    pub fn unlink_if_targeting(original: &Path, link: &Path) -> std::io::Result<()> {
        let Ok(target) = junction::get_target(link) else {
            return Ok(());
        };

        if target == original {
            junction::delete(link)?;
        }

        Ok(())
    }
}
