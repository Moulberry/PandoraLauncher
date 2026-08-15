use std::path::Path;
use std::{fs, io};

use cfg_if::cfg_if;

mod utility;

cfg_if! {
    if #[cfg(unix)] {
        mod unix;
        pub use self::unix::reflink;
        pub(crate) use self::unix::reflink_block;
    } else if #[cfg(windows)] {
        mod windows_impl;
        pub use self::windows_impl::reflink;
        pub use self::windows_impl::check_reflink_support;
        pub(crate) use self::windows_impl::reflink_block;
    } else {
        pub use self::reflink_not_supported as reflink;
        pub(crate) use self::reflink_block_not_supported as reflink_block;
    }
}

#[allow(dead_code)]
pub fn reflink_not_supported(_from: &Path, _to: &Path) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[allow(dead_code)]
pub(crate) fn reflink_block_not_supported(
    _from: &fs::File,
    _from_offset: u64,
    _to: &fs::File,
    _to_offset: u64,
    _src_length: u64,
    _cluster_size: Option<std::num::NonZeroU64>,
) -> io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}
