use std::convert::TryInto;
use std::os::unix::io::AsRawFd;
use std::{fs, io, path::Path};

use crate::sys::utility::AutoRemovedFile;

pub fn reflink(from: &Path, to: &Path) -> io::Result<()> {
    let src = fs::File::open(from)?;
    let permissions = src.metadata()?.permissions();

    // pass O_EXCL to mimic macos behaviour
    let dest = AutoRemovedFile::create_new(to)?;
    rustix::fs::ioctl_ficlone(&dest, &src)?;

    dest.persist(permissions)
}

#[cfg(target_os = "linux")]
pub(crate) fn reflink_block(
    from: &fs::File,
    from_offset: u64,
    to: &fs::File,
    to_offset: u64,
    src_length: u64,
    _cluster_size: Option<std::num::NonZeroU64>,
) -> io::Result<()> {
    let ret = unsafe {
        libc::ioctl(
            to.as_raw_fd(),
            libc::FICLONERANGE.try_into().unwrap(),
            &libc::file_clone_range {
                src_fd: from.as_raw_fd().into(),
                src_offset: from_offset,
                src_length,
                dest_offset: to_offset,
            },
        )
    };

    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "android")]
pub(crate) use crate::sys::reflink_block_not_supported as reflink_block;
