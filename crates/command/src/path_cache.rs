use std::{ffi::{OsStr, OsString}, path::{Path, PathBuf}, sync::Arc, time::{Duration, Instant}};

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;

struct CommandPathCacheEntry {
    expiry: Instant,
    path: Option<Arc<Path>>,
}

static COMMAND_PATH_CACHE: Lazy<RwLock<FxHashMap<OsString, CommandPathCacheEntry>>> = Lazy::new(Default::default);

fn illegal_char(b: u8) -> bool {
    b < 0x1f || matches!(b, b'/' | b'?' | b'<' | b'>' | b'\\' | b':' | b'*' | b'|' | b'"')
}

pub fn get_command_path_cached(command: &OsStr) -> Option<Arc<Path>> {
    if command.as_encoded_bytes().iter().any(|b| illegal_char(*b)) {
        return None;
    }

    let now = Instant::now();

    let mut cache = COMMAND_PATH_CACHE.upgradable_read();
    if let Some(entry) = cache.get(command) {
        if entry.expiry > now && entry.path.as_deref().map(Path::exists).unwrap_or(true) {
            return entry.path.clone();
        }
    }

    let path = find_command(command).map(Into::into);
    let expiry = if path.is_none() {
        now + Duration::from_secs(5)
    } else {
        now + Duration::from_secs(60)
    };

    cache.with_upgraded(|cache| {
        cache.insert(command.into(), CommandPathCacheEntry {
            expiry,
            path: path.clone()
        })
    });

    path
}

pub fn get_command_path(command: &OsStr) -> Option<Arc<Path>> {
    if command.as_encoded_bytes().iter().any(|b| illegal_char(*b)) {
        return None;
    }

    let path = find_command(command).map(Into::into);
    let expiry = if path.is_none() {
        Instant::now() + Duration::from_secs(5)
    } else {
        Instant::now() + Duration::from_secs(60)
    };

    COMMAND_PATH_CACHE.write().insert(command.into(), CommandPathCacheEntry {
        expiry,
        path: path.clone()
    });

    path
}

fn find_command(command: &OsStr) -> Option<PathBuf> {
    for mut path in std::env::split_paths(&std::env::var_os("PATH")?) {
        if !path.is_absolute() {
            continue;
        }
        path.push(command);
        if std::fs::exists(&path).unwrap_or(false) {
            return Some(path);
        }
    }
    None
}
