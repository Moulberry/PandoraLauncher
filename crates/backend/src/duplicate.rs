use std::{io::{Error, ErrorKind, Read, Write}, path::Path, sync::Arc};
use sha1::Digest;

use bridge::{
    instance::InstanceID,
    modal_action::{ModalAction, ProgressTracker, ProgressTrackerFinishType},
    safe_path::SafePath,
};

use crate::{BackendState, create_content_library_path, hard_link_or_copy, is_single_component_path_str, symlink_dir_or_file};
use crate::export::{is_mod_file, is_resourcepack_file, is_shaderpack_file};

fn is_content_file(rel: &Path) -> bool {
    let Ok(rel) = rel.strip_prefix(".minecraft") else {
        return false;
    };
    let Some(rel) = SafePath::from_std_path(&rel) else {
        return false;
    };
    is_mod_file(&rel) || is_resourcepack_file(&rel) || is_shaderpack_file(&rel)
}

fn content_library_extension(path: &Path) -> Option<&str> {
    let filename = path.file_name().and_then(|s| s.to_str())?;
    let base = if filename.ends_with(".disabled") {
        &filename[..filename.len() - ".disabled".len()]
    } else {
        filename
    };
    let dot = base.rfind('.')?;
    Some(&base[dot + 1..])
}

fn hash_file(path: &Path, check_cancel: &dyn Fn() -> std::io::Result<()>) -> std::io::Result<[u8; 20]> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = sha1::Sha1::default();
    let mut buf = vec![0_u8; 128 * 1024];
    loop {
        check_cancel()?;
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hasher.finalize().into())
}

fn copy_file(from: &Path, to: &Path, check_cancel: &dyn Fn() -> std::io::Result<()>) -> std::io::Result<u64> {
    let mut buf = vec![0_u8; 128 * 1024];
    let mut src = std::fs::File::open(from)?;
    let mut dst = std::fs::File::create(to)?;
    let mut total = 0_u64;
    loop {
        check_cancel()?;
        let read = src.read(&mut buf)?;
        if read == 0 {
            return Ok(total);
        }
        dst.write_all(&buf[..read])?;
        total += read as u64;
    }
}

fn duplicate_with_content_library(
    from: &Path,
    to: &Path,
    content_library_dir: &Path,
    progress: &dyn Fn(u64, u64),
    check_cancel: &dyn Fn() -> std::io::Result<()>,
) -> std::io::Result<()> {
    let from = from.canonicalize()?;
    if !from.is_dir() {
        return Err(ErrorKind::NotADirectory.into());
    }
    if !to.is_dir() {
        return Err(ErrorKind::AlreadyExists.into());
    }

    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut internal_symlinks = Vec::new();
    let mut external_symlinks = Vec::new();
    #[cfg(windows)]
    let mut internal_junctions = Vec::new();
    #[cfg(windows)]
    let mut external_junctions = Vec::new();

    let mut directories_to_visit = Vec::new();
    directories_to_visit.push((from.to_path_buf(), 0));

    while let Some((directory, depth)) = directories_to_visit.pop() {
        let read_dir = std::fs::read_dir(directory)?;
        for entry in read_dir {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            let Ok(relative) = path.strip_prefix(&from) else {
                return Err(Error::new(ErrorKind::Other, format!("{path:?} is not a child of {from:?}")));
            };
            if file_type.is_symlink() {
                let target = std::fs::read_link(&path)?;
                if let Ok(internal) = target.strip_prefix(&from) {
                    internal_symlinks.push((relative.to_path_buf(), internal.to_path_buf()));
                } else {
                    external_symlinks.push((relative.to_path_buf(), target));
                }
            } else if file_type.is_file() {
                let eligible = is_content_file(&relative);
                files.push((relative.to_path_buf(), path, eligible));
            } else if file_type.is_dir() {
                #[cfg(windows)]
                if let Ok(target) = junction::get_target(&path) {
                    if let Ok(internal) = target.strip_prefix(&from) {
                        internal_junctions.push((relative.to_path_buf(), internal.to_path_buf()));
                    } else {
                        external_junctions.push((relative.to_path_buf(), target));
                    }
                    continue;
                }

                if depth >= 256 {
                    return Err(ErrorKind::QuotaExceeded.into());
                }

                directories.push(relative.to_path_buf());
                directories_to_visit.push((path, depth + 1));
            }
        }
    }

    let total_files = files.len() as u64;
    progress(0, total_files);

    for directory in directories {
        _ = std::fs::create_dir(to.join(directory));
    }

    let mut files_done = 0_u64;
    for (relative, source_path, is_library_eligible) in &files {
        check_cancel()?;
        let dest = to.join(relative);

        if *is_library_eligible {
            if let Ok(hash) = hash_file(source_path, check_cancel) {
                let ext = content_library_extension(source_path);
                let lib_path = create_content_library_path(content_library_dir, hash, ext);
                if lib_path.exists() {
                    if hard_link_or_copy(&lib_path, &dest).is_ok() {
                        files_done += 1;
                        progress(files_done, total_files);
                        continue;
                    }
                }
            }
        }

        copy_file(source_path, &dest, check_cancel)?;
        files_done += 1;
        progress(files_done, total_files);
    }

    for (relative, internal) in &internal_symlinks {
        let dest = to.join(relative);
        let target = to.join(internal);
        if let Err(err) = symlink_dir_or_file(&target, &dest) {
            return Err(err);
        }
    }
    for (relative, target) in &external_symlinks {
        let dest = to.join(relative);
        if let Err(err) = symlink_dir_or_file(&target, &dest) {
            return Err(err);
        }
    }
    #[cfg(windows)]
    for (relative, internal) in &internal_junctions {
        let dest = to.join(relative);
        let target = to.join(internal);
        if let Err(err) = junction::create(&target, &dest) {
            return Err(err);
        }
    }
    #[cfg(windows)]
    for (relative, target) in &external_junctions {
        let dest = to.join(relative);
        if let Err(err) = junction::create(&target, &dest) {
            return Err(err);
        }
    }

    Ok(())
}

pub async fn duplicate_instance(
    backend: Arc<BackendState>,
    id: InstanceID,
    name: &str,
    modal_action: ModalAction,
) {
    if !is_single_component_path_str(name) {
        modal_action.set_error_message(format!("Unable to duplicate instance, name must not be a path: {name}").into());
        modal_action.set_finished();
        return;
    }
    if !sanitize_filename::is_sanitized_with_options(name, sanitize_filename::OptionsForCheck { windows: true, ..Default::default() }) {
        modal_action.set_error_message(format!("Unable to duplicate instance, name is invalid: {name}").into());
        modal_action.set_finished();
        return;
    }
    if backend.instance_state.read().instances.iter().any(|i| i.name == name) {
        modal_action.set_error_message("Unable to duplicate instance, name is already used".to_string().into());
        modal_action.set_finished();
        return;
    }

    let source = {
        let mut state = backend.instance_state.write();
        let Some(instance) = state.instances.get_mut(id) else {
            modal_action.set_error_message("Unable to duplicate instance, unknown id".to_string().into());
            modal_action.set_finished();
            return;
        };
        instance.root_path.clone()
    };

    let dest = backend.directories.instances_dir.join(name);

    if let Err(err) = std::fs::create_dir(&dest) {
        modal_action.set_error_message(format!("Unable to create instance directory: {err}").into());
        modal_action.set_finished();
        return;
    }

    let tracker = ProgressTracker::new("Copying instance files...".into(), backend.send.clone());
    modal_action.trackers.push(tracker.clone());

    let result = duplicate_with_content_library(&source, &dest, &backend.directories.content_library_dir, &|current, total| {
        tracker.set_count(current as usize);
        tracker.set_total(total as usize);
        tracker.notify();
    }, &|| {
        if modal_action.has_requested_cancel() {
            tracker.set_title("Cancelling...".into());
            tracker.notify();
            Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Operation cancelled"))
        } else {
            Ok(())
        }
    });

    match result {
        Ok(()) => {
            tracker.set_finished(ProgressTrackerFinishType::Normal);
            tracker.notify();
        },
        Err(error) => {
            let _ = std::fs::remove_dir_all(&dest);
            if modal_action.has_requested_cancel() {
                tracker.set_finished(ProgressTrackerFinishType::Fast);
                tracker.notify();
            } else {
                tracker.set_finished(ProgressTrackerFinishType::Error);
                tracker.notify();
                modal_action.set_error_message(error.to_string().into());
            }
        },
    }

    modal_action.set_finished();
}
