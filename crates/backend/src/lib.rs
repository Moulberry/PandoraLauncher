#![deny(unused_must_use)]

mod backend;
use std::{ffi::OsString, io::Write, path::{Path, PathBuf}, sync::Arc};

pub use backend::*;
use bridge::instance::InstanceContentSummary;
use rand::RngCore;
use serde::Deserialize;
use sha1::{Digest, Sha1};

mod backend_filesystem;
mod backend_handler;

mod account;
mod arcfactory;
mod directories;
mod install_content;
mod instance;
mod java_manifest;
mod launch;
mod launch_wrapper;
mod lockfile;
mod log_reader;
mod metadata;
mod mod_metadata;
mod id_slab;
mod persistent;
mod shortcut;
mod syncing;
mod update;

pub(crate) fn is_single_component_path(path: &str) -> bool {
    let path = std::path::Path::new(path);
    let mut components = path.components().peekable();

    if let Some(first) = components.peek() && !matches!(first, std::path::Component::Normal(_)) {
        return false;
    }

    components.count() == 1
}

pub(crate) fn check_sha1_hash(path: &Path, expected_hash: [u8; 20]) -> std::io::Result<bool> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let _ = std::io::copy(&mut file, &mut hasher)?;

    let actual_hash = hasher.finalize();

    Ok(expected_hash == *actual_hash)
}

#[derive(Debug, thiserror::Error)]
pub enum IoOrSerializationError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub(crate) fn read_json<T: for <'de> Deserialize<'de>>(path: &Path) -> Result<T, IoOrSerializationError> {
    let data = std::fs::read(path)?;
    Ok(serde_json::from_slice(&data)?)
}

pub(crate) fn write_safe(path: &Path, content: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let mut temp = path.to_path_buf();
    temp.add_extension(format!("{}", rand::thread_rng().next_u32()));
    temp.add_extension("new");

    let mut temp_file = std::fs::File::create(&temp)?;

    temp_file.write_all(content)?;
    temp_file.flush()?;
    temp_file.sync_all()?;

    drop(temp_file);

    if let Err(err) = std::fs::rename(&temp, path) {
        _ = std::fs::remove_file(&temp);
        return Err(err);
    }

    Ok(())
}

pub(crate) fn pandora_aux_path(id: &Option<Arc<str>>, name: &Option<Arc<str>>, path: &Path) -> Option<PathBuf> {
    let name = id.as_ref().or(name.as_ref());

    if let Some(name) = name {
        let name = name.trim_ascii();
        if !name.is_empty() {
            let mut path = path.parent()?.join(format!(".{name}"));
            path.add_extension("aux");
            path.add_extension("json");
            return Some(path);
        }
    }

    let mut new_path = path.to_path_buf();

    if let Some(extension) = new_path.extension() {
        if extension == "disabled" {
            new_path.set_extension("");
        }
    }

    let mut new_filename = OsString::new();
    new_filename.push(".");
    new_filename.push(new_path.file_name()?);
    new_path.set_file_name(new_filename);

    new_path.add_extension("aux");
    new_path.add_extension("json");

    Some(new_path)
}

pub(crate) fn pandora_aux_path_for_content(content: &InstanceContentSummary) -> Option<PathBuf> {
    pandora_aux_path(&content.content_summary.id, &content.content_summary.name, &content.path)
}

pub(crate) fn create_content_library_path(content_library_dir: &Path, expected_hash: [u8; 20], extension: Option<&str>) -> PathBuf {
    let hash_as_str = hex::encode(expected_hash);

    let hash_folder = content_library_dir.join(&hash_as_str[..2]);
    let mut path = hash_folder.join(hash_as_str);

    if let Some(extension) = extension {
        path.set_extension(extension);
    }

    path
}
