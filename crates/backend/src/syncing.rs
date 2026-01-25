use std::{collections::HashSet, ffi::OsStr, path::{Path, PathBuf}, sync::Arc, time::SystemTime};

use bridge::message::SyncState;
use enum_map::EnumMap;
use enumset::EnumSet;
use rustc_hash::FxHashMap;
use schema::backend_config::SyncTarget;
use strum::IntoEnumIterator;

use crate::directories::LauncherDirectories;

struct SyncLink {
    source: Box<Path>,
    target: Box<Path>
}

trait Syncer {
    fn link(self);
    fn unlink(self);
}

struct SymlinkSync {
    link: SyncLink
}

impl Syncer for SymlinkSync {
    fn link(self) {
        _ = linking::link(&self.link.source, &self.link.target);
    }

    fn unlink(self) {
        _ = linking::unlink_if_targeting(&self.link.source, &self.link.target);
    }
}

struct CopySaveSync {
    link: SyncLink
}

impl Syncer for CopySaveSync {
    fn link(self) {
        _ = std::fs::copy(self.link.source, self.link.target);
    }

    fn unlink(self) {
        _ = std::fs::copy(self.link.target, self.link.source);
    }
}

struct CopyDeleteSync {
    link: SyncLink
}

impl Syncer for CopyDeleteSync {
    fn link(self) {
        _ = std::fs::copy(self.link.source, self.link.target);
    }

    fn unlink(self) {
        _ = std::fs::remove_file(self.link.target);
    }
}

struct ChildrenSync {
    target_dir: Box<Path>,
    sources: Box<[Box<Path>]>,
    source_dirs: Box<[Box<Path>]>,
    keep_name: bool
}

impl ChildrenSync {
    fn source_to_target_path(&self, source_path: &Path) -> Option<PathBuf> {
        let name = source_path.file_name().unwrap_or_else(|| OsStr::new(""));
        let target_base_path = self.target_dir.join(&name);
        
       return Some(if self.keep_name {
            if target_base_path.try_exists().unwrap_or(true) {
                return None;
            }
            target_base_path
        } else {
            let mut err_count: u8 = 0;
            loop {
                if err_count == 255 { return None; }
                
                let number = rand::random::<u32>();
                let target_path = target_base_path.with_added_extension(format!("{number:0>8x}.plsync"));
                
                if !target_path.try_exists().unwrap_or(true) { break target_path; }
                err_count += 1;
            }
        });
    }
}

impl Syncer for ChildrenSync {
    fn link(self) {
        if !self.target_dir.is_dir() { return; }
        
        for dir_path in &self.source_dirs {
            if !dir_path.exists() { continue; }
            
            let Ok(dir) = dir_path.read_dir() else { continue; };
            for r in dir {
                let Ok(entry) = r else { continue };
                let source_path = entry.path();
                
                let Some(target_path) = self.source_to_target_path(&source_path) else { continue };
                
                _ = linking::link(&source_path, &target_path);
            }
        }
        
        for source_path in &self.sources {
            if !source_path.exists() { continue; }
            
            let Some(target_path) = self.source_to_target_path(&source_path) else { continue };
            
            _ = linking::link(&source_path, &target_path);
        }
        
    }

    fn unlink(self) {
        let mut all_sources: HashSet<PathBuf> = HashSet::new();
        
        for dir_path in &self.source_dirs {
            if !dir_path.exists() { continue; }
            
            let Ok(dir) = dir_path.read_dir() else { continue; };
            for r in dir {
                let Ok(entry) = r else { continue };
                let source_path = entry.path();
                
                all_sources.insert(source_path);
            }
        }
        
        for source_path in &self.sources {
            if !source_path.exists() { continue; }
            all_sources.insert(source_path.to_path_buf());
        }
        
        let Ok(dir) = self.target_dir.read_dir() else { return; };
        for r in dir {
            let Ok(entry) = r else { continue };
            
            let target_path = entry.path();
            if target_path.is_symlink() {
                let Ok(source_path) = target_path.read_link() else { continue; };
                if all_sources.contains(&source_path) {
                    _ = std::fs::remove_file(target_path);
                }
            }
        }
    }
}

struct CustomScriptSync {
    link: SyncLink
}

impl Syncer for CustomScriptSync {
    fn link(self) {
        todo!()
    }

    fn unlink(self) {
        todo!()
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
