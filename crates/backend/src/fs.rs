use std::{borrow::Cow, ffi::{OsStr, OsString}, io::{Error, ErrorKind, Write}, path::{Path, PathBuf}, sync::Arc};

use bridge::instance::InstanceContentSummary;
use rand::RngCore;
use rustc_hash::FxHashSet;
use serde::Deserialize;
use sha1::{Digest, Sha1};
use uuid::Uuid;

pub fn is_single_component_path_str(path: &str) -> bool {
    is_single_component_path(std::path::Path::new(path))
}

pub fn is_single_component_path(path: &Path) -> bool {
    let mut components = path.components().peekable();

    if let Some(first) = components.peek() && !matches!(first, std::path::Component::Normal(_)) {
        return false;
    }

    components.count() == 1
}

pub fn unique_name<'a>(parent: &Path, original_name: &'a str, is_dir: bool) -> Cow<'a, str> {
    if !parent.join(original_name).exists() {
        return Cow::Borrowed(original_name);
    }

    let candidate = Path::new(original_name);
    let (stem, ext) = if is_dir {
        (Cow::Borrowed(original_name), String::new())
    } else {
        let stem = candidate.file_stem().unwrap_or_default().to_string_lossy();
        let ext = candidate.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        (stem, ext)
    };

    const MAX_RETRIES: u32 = 100;
    for i in 1..MAX_RETRIES {
        let numbered = format!("{stem} ({i}){ext}");
        if !parent.join(&numbered).exists() {
            return Cow::Owned(numbered);
        }
    }

    Cow::Owned(format!("{stem} ({}){ext}", Uuid::new_v4()))
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
    create_content_library_path_osstrext(content_library_dir, expected_hash, extension.map(OsStr::new))
}

pub(crate) fn create_content_library_path_osstrext(content_library_dir: &Path, expected_hash: [u8; 20], extension: Option<&OsStr>) -> PathBuf {
    let hash_as_str = hex::encode(expected_hash);

    let hash_folder = content_library_dir.join(&hash_as_str[..2]);
    let mut path = hash_folder.join(hash_as_str);

    if let Some(extension) = extension {
        path.set_extension(extension);
    }

    path
}

#[derive(Debug)]
pub struct FolderChanges {
    all_dirty: bool,
    paths: FxHashSet<Arc<Path>>,
}

impl FolderChanges {
    pub fn no_changes() -> Self {
        Self { all_dirty: false, paths: Default::default() }
    }

    pub fn all_dirty() -> Self {
        Self { all_dirty: true, paths: Default::default() }
    }

    pub fn is_empty(&self) -> bool {
        !self.all_dirty && self.paths.is_empty()
    }

    pub fn dirty_path(&mut self, path: Arc<Path>) {
        if self.all_dirty {
            return;
        }
        self.paths.insert(path);
    }

    pub fn take(&mut self) -> (bool, FxHashSet<Arc<Path>>) {
        if self.all_dirty {
            self.all_dirty = false;
            self.paths.clear();
            (true, Default::default())
        } else {
            (false, std::mem::take(&mut self.paths))
        }
    }

    pub fn dirty_all(&mut self) {
        self.all_dirty = true;
        self.paths.clear();
    }

    pub fn apply_to(self, other: &mut FolderChanges) {
        if other.all_dirty {
            return;
        }
        if self.all_dirty {
            other.all_dirty = true;
            other.paths.clear();
        } else {
            other.paths.extend(self.paths);
        }
    }
}

pub fn copy_content_recursive(from: &Path, to: &Path, strict: bool, progress: &dyn Fn(u64, u64)) -> std::io::Result<()> {
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

    let mut total_bytes = 0;

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
            #[cfg(windows)]
            if let Ok(target) = junction::get_target(&path) {
                if let Ok(internal) = target.strip_prefix(&from) {
                    internal_junctions.push((relative.to_path_buf(), internal.to_path_buf()));
                } else {
                    external_junctions.push((relative.to_path_buf(), target));
                }
                continue;
            }
            if file_type.is_symlink() {
                let target = std::fs::read_link(&path)?;
                if let Ok(internal) = target.strip_prefix(&from) {
                    internal_symlinks.push((relative.to_path_buf(), internal.to_path_buf()));
                } else {
                    external_symlinks.push((relative.to_path_buf(), target));
                }
            } else if file_type.is_file() {
                let metadata = entry.metadata()?;
                files.push((relative.to_path_buf(), path));
                total_bytes += metadata.len();

            } else if file_type.is_dir() {
                if depth >= 256 {
                    return Err(ErrorKind::QuotaExceeded.into());
                }

                directories.push(relative.to_path_buf());
                directories_to_visit.push((path, depth+1));
            }
        }
    }
    (progress)(0, total_bytes);

    for directory in directories {
        _ = std::fs::create_dir(to.join(directory));
    }
    let mut copied_bytes = 0;
    for (relative, copy_from) in files {
        let dest = to.join(relative);
        match std::fs::copy(copy_from, dest) {
            Ok(bytes) => copied_bytes += bytes,
            Err(err) => if strict {
                return Err(err);
            },
        }
        (progress)(copied_bytes, total_bytes);
    }
    if strict && copied_bytes != total_bytes {
        return Err(Error::new(ErrorKind::Other,
            format!("Expected copy size did not match. Expected to copy {total_bytes} bytes, copied {copied_bytes} instead")));
    }
    for (relative, internal) in internal_symlinks {
        let dest = to.join(relative);
        let target = to.join(internal);
        if let Err(err) = symlink_dir_or_file(&target, &dest) && strict {
            return Err(err);
        }
    }
    for (relative, target) in external_symlinks {
        let dest = to.join(relative);
        if let Err(err) = symlink_dir_or_file(&target, &dest) && strict {
            return Err(err);
        }
    }
    #[cfg(windows)]
    for (relative, internal) in internal_junctions {
        let dest = to.join(relative);
        let target = to.join(internal);
        if let Err(err) = junction::create(&target, &dest) && strict {
            return Err(err);
        }
    }
    #[cfg(windows)]
    for (relative, target) in external_junctions {
        let dest = to.join(relative);
        if let Err(err) = junction::create(&target, &dest) && strict {
            return Err(err);
        }
    }
    Ok(())
}

pub fn symlink_dir_or_file(original: &Path, link: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        if !original.exists() {
            return Err(ErrorKind::NotFound.into());
        }
        std::os::unix::fs::symlink(original, link)
    }
    #[cfg(windows)]
    {
        let metadata = original.metadata()?;
        if metadata.is_dir() {
            std::os::windows::fs::symlink_dir(original, link)
        } else if metadata.is_file() {
            std::os::windows::fs::symlink_file(original, link)
        } else {
            return Err(ErrorKind::NotFound.into());
        }
    }
    #[cfg(not(any(windows, unix)))]
    compile_error!("Unsupported platform: can't symlink");
}

pub fn fastcopy(from: &Path, to: &Path, reflink: bool, mut hard_link: bool) -> std::io::Result<()> {
    match std::fs::remove_file(to) {
        Ok(()) => {},
        Err(err) if err.kind() == ErrorKind::NotFound => {},
        Err(err) => return Err(err),
    }

    if reflink {
        let Err(err) = reflink_copy::reflink(from, to) else {
            return Ok(());
        };

        if err.kind() == ErrorKind::CrossesDevices {
            // If the paths are on different devices, hard linking will also fail
            hard_link = false;
        }
    }

    if hard_link {
        if let Err(err) = std::fs::hard_link(from, to) {
            if err.kind() != ErrorKind::CrossesDevices {
                return Err(err);
            }
        } else {
            return Ok(());
        }
    }

    std::fs::copy(from, to).map(|_| ())
}

pub fn rename_with_fallback_across_devices(from: &Path, to: &Path) -> std::io::Result<()> {
    // Remove empty 'to' directory to ensure consistent behaviour across unix and windows
    if let Err(err) = std::fs::remove_dir(to) && !matches!(err.kind(), ErrorKind::NotADirectory | ErrorKind::NotFound) {
        return Err(err);
    }
    if let Err(err) = std::fs::rename(from, to) {
        if err.kind() == ErrorKind::CrossesDevices {
            #[cfg(windows)]
            if let Ok(true) = junction::exists(&from) {
                // Junction points cannot be made across devices, so return the CrossesDevices error
                return Err(err);
            }
            // Obviously this is racy, but this is the best we can do
            let file_type = std::fs::symlink_metadata(from)?.file_type();
            if file_type.is_symlink() {
                let target = std::fs::read_link(from)?;
                symlink_dir_or_file(&target, to)?;
                _ = std::fs::remove_file(from);
            } else if file_type.is_dir() {
                std::fs::create_dir(to)?;
                if let Err(err) = copy_content_recursive(from, to, true, &|_, _| {}) {
                    _ = std::fs::remove_dir_all(to);
                    return Err(err);
                } else {
                    _ = std::fs::remove_dir_all(from);
                    return Ok(());
                }
            } else if file_type.is_file() {
                std::fs::copy(from, to)?;
                _ = std::fs::remove_file(from);
            } else {
                return Err(Error::new(ErrorKind::Other, format!("{from:?} is not a symlink, file or folder")));
            }
            return Ok(());
        }
        Err(err)
    } else {
        Ok(())
    }
}

#[cfg(unix)]
pub struct FileMetadata(std::fs::Metadata);

#[cfg(windows)]
pub struct FileMetadata {
    number_of_links: u32,
    low_precision_id: (u32, u32, u32),
    high_precision_id: Option<(u64, [u8; 16])>,
}

#[cfg(unix)]
impl FileMetadata {
    pub fn new(path: &Path) -> std::io::Result<Self> {
        let metadata = std::fs::metadata(path)?;
        if metadata.is_dir() {
            return Err(std::io::ErrorKind::IsADirectory.into());
        }
        Ok(Self(metadata))
    }

    pub fn is_same(&self, other: &FileMetadata) -> bool {
        use std::os::unix::fs::MetadataExt;
        self.0.dev() == other.0.dev() && self.0.ino() == other.0.ino()
    }

    pub fn number_of_links(&self) -> u64 {
        use std::os::unix::fs::MetadataExt;
        self.0.nlink()
    }
}

#[cfg(windows)]
impl FileMetadata {
    pub fn new(path: &Path) -> std::io::Result<Self> {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Storage::FileSystem::{FILE_ID_INFO, BY_HANDLE_FILE_INFORMATION};

        let file = std::fs::OpenOptions::new().open(path)?;
        let handle = windows::Win32::Foundation::HANDLE(file.as_raw_handle());

        let mut file_info: BY_HANDLE_FILE_INFORMATION = Default::default();

        unsafe {
            windows::Win32::Storage::FileSystem::GetFileInformationByHandle(
                handle,
                &mut file_info as *mut BY_HANDLE_FILE_INFORMATION as *mut _,
            )?;
        }

        let mut metadata = Self {
            number_of_links: file_info.nNumberOfLinks,
            low_precision_id: (file_info.dwVolumeSerialNumber, file_info.nFileIndexHigh, file_info.nFileIndexLow),
            high_precision_id: None,
        };

        let mut file_id_info: FILE_ID_INFO = Default::default();
        let result = unsafe {
            windows::Win32::Storage::FileSystem::GetFileInformationByHandleEx(
                handle,
                windows::Win32::Storage::FileSystem::FileIdInfo,
                &mut file_id_info as *mut FILE_ID_INFO as *mut _,
                std::mem::size_of::<FILE_ID_INFO>() as u32,
            )
        };
        if result.is_ok() {
            metadata.high_precision_id = Some((file_id_info.VolumeSerialNumber, file_id_info.FileId.Identifier));
        }

        Ok(metadata)
    }

    pub fn is_same(&self, other: &FileMetadata) -> bool {
        self.low_precision_id == other.low_precision_id && self.high_precision_id == other.high_precision_id
    }

    pub fn number_of_links(&self) -> u64 {
        self.number_of_links as u64
    }
}
