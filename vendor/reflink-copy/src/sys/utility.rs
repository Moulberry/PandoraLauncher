#![allow(dead_code)]

use std::{
    fs::{remove_file, File, Permissions},
    io,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, RawFd};

#[derive(Debug)]
pub(super) struct AutoRemovedFile {
    // Option<File> uses File's niche, so this is zero cost
    inner: Option<File>,
    path: PathBuf,
}

impl AutoRemovedFile {
    pub fn create_new(path: &Path) -> io::Result<Self> {
        // pass O_EXCL to mimic macos behaviour
        let inner = File::options().write(true).create_new(true).open(path)?;

        Ok(Self {
            inner: Some(inner),
            path: path.into(),
        })
    }

    #[cfg(unix)]
    pub fn as_raw_fd(&self) -> RawFd {
        self.as_inner_file().as_raw_fd()
    }

    pub fn persist(mut self, perm: Permissions) -> io::Result<()> {
        self.inner.take().unwrap().set_permissions(perm)
    }

    pub fn as_inner_file(&self) -> &File {
        self.inner.as_ref().unwrap()
    }
}

#[cfg(unix)]
impl AsFd for AutoRemovedFile {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.as_inner_file().as_fd()
    }
}

impl Drop for AutoRemovedFile {
    fn drop(&mut self) {
        if self.inner.is_some() {
            if let Err(_err) = remove_file(&self.path) {
                #[cfg(feature = "tracing")]
                tracing::warn!(
                    ?_err,
                    "Failed to remove dest file {} on cleanup (failed to reflink)",
                    self.path.display(),
                );
            }
        }
    }
}
