use cfg_if::cfg_if;

cfg_if! {
    // ioctl_ficlone / FICLONERANGE are not available on SPARC platforms
    if #[cfg(all(any(target_os = "linux", target_os = "android"), not(any(target_arch = "sparc", target_arch = "sparc64"))))] {
        mod linux;
        pub use linux::reflink;
        pub(crate) use linux::reflink_block;
    } else if #[cfg(any(target_os = "macos", target_os = "ios", target_os = "tvos", target_os = "watchos"))] {
        mod macos;
        pub use macos::reflink;
        pub(crate) use super::reflink_block_not_supported as reflink_block;
    } else {
        pub use super::reflink_not_supported as reflink;
        pub(crate) use super::reflink_block_not_supported as reflink_block;
    }
}
