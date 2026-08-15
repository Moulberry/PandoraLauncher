//! Some file systems implement COW (copy on write) functionality in order to speed up file copies.
//! On a high level, the new file does not actually get copied, but shares the same on-disk data
//! with the source file. As soon as one of the files is modified, the actual copying is done by
//! the underlying OS.
//!
//! This library exposes a single function, `reflink`, which attempts to copy a file using the
//! underlying OSs' block cloning capabilities. The function signature is identical to `std::fs::copy`.
//!
//! At the moment Linux, Android, OSX, iOS, and Windows are supported.
//!
//! Note: On Windows, the integrity information features are only available on Windows Server editions
//! starting from Windows Server 2012. Client versions of Windows do not support these features.
//! [More Information](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-fsctl_set_integrity_information)
//!
//! As soon as other OSes support the functionality, support will be added.

mod reflink_block;
mod sys;

use std::fs;
use std::io;
use std::io::ErrorKind;
use std::path::Path;

/// Copies a file using COW semantics.
///
/// For compatibility reasons with macOS, the target file will be created using `OpenOptions::create_new`.
/// If you want to overwrite existing files, make sure you manually delete the target file first
/// if it exists.
///
/// ```rust
/// match reflink_copy::reflink("src.txt", "dest.txt") {
///     Ok(()) => println!("file has been reflinked"),
///     Err(e) => println!("error while reflinking: {:?}", e)
/// }
/// ```
///
/// # Implementation details per platform
///
/// ## Linux / Android
///
/// Uses `ioctl_ficlone`. Supported file systems include btrfs and XFS (and maybe more in the future).
/// NOTE that it generates a temporary file and is not atomic.
///
/// ## MacOS / OS X / iOS
///
/// Uses `clonefile` library function. This is supported on OS X Version >=10.12 and iOS version >= 10.0
/// This will work on APFS partitions (which means most desktop systems are capable).
/// If src names a directory, the directory hierarchy is cloned as if each item was cloned individually.
///
/// ## Windows
///
/// Uses ioctl `FSCTL_DUPLICATE_EXTENTS_TO_FILE`.
///
/// Supports ReFS on Windows Server and Windows Dev Drives. *Important note*: The windows implementation is currently
/// untested and probably buggy. Contributions/testers with access to a Windows Server or Dev Drives are welcome.
/// [More Information on Dev Drives](https://learn.microsoft.com/en-US/windows/dev-drive/#how-does-dev-drive-work)
///
/// NOTE that it generates a temporary file and is not atomic.
#[inline(always)]
pub fn reflink(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    #[cfg_attr(feature = "tracing", tracing_attributes::instrument(name = "reflink"))]
    fn inner(from: &Path, to: &Path) -> io::Result<()> {
        sys::reflink(from, to).map_err(|err| {
            // Linux and Windows will return an inscrutable error when `from` is a directory or a
            // symlink, so add the real problem to the error. We need to use `fs::symlink_metadata`
            // here because `from.is_file()` traverses symlinks.
            //
            // According to https://www.manpagez.com/man/2/clonefile/, Macos otoh can reflink files,
            // directories and symlinks, so the original error is fine.
            if !cfg!(any(
                target_os = "macos",
                target_os = "ios",
                target_os = "tvos",
                target_os = "watchos"
            )) && !fs::symlink_metadata(from).map_or(false, |m| m.is_file())
            {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("the source path is not an existing regular file: {}", err),
                )
            } else {
                err
            }
        })
    }

    inner(from.as_ref(), to.as_ref())
}

/// Attempts to reflink a file. If the operation fails, a conventional copy operation is
/// attempted as a fallback.
///
/// If the function reflinked a file, the return value will be `Ok(None)`.
///
/// If the function copied a file, the return value will be `Ok(Some(written))`.
///
/// If target file already exists, operation fails with [`ErrorKind::AlreadyExists`].
///
/// ```rust
/// match reflink_copy::reflink_or_copy("src.txt", "dest.txt") {
///     Ok(None) => println!("file has been reflinked"),
///     Ok(Some(written)) => println!("file has been copied ({} bytes)", written),
///     Err(e) => println!("an error occured: {:?}", e)
/// }
/// ```
///
/// # Implementation details per platform
///
/// ## MacOS / OS X / iOS
///
/// If src names a directory, the directory hierarchy is cloned as if each item was cloned
/// individually. This method does not provide a fallback for directories, so the fallback will also
/// fail if reflinking failed. Macos supports reflinking symlinks, which is supported by the
/// fallback.
#[inline(always)]
pub fn reflink_or_copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<Option<u64>> {
    #[cfg_attr(
        feature = "tracing",
        tracing_attributes::instrument(name = "reflink_or_copy")
    )]
    fn inner(from: &Path, to: &Path) -> io::Result<Option<u64>> {
        if let Err(err) = sys::reflink(from, to) {
            match err.kind() {
                ErrorKind::NotFound | ErrorKind::PermissionDenied | ErrorKind::AlreadyExists => {
                    return Err(err);
                }
                _ => {}
            }

            #[cfg(feature = "tracing")]
            tracing::warn!(?err, "Failed to reflink, fallback to fs::copy");

            fs::copy(from, to).map(Some).map_err(|err| {
                // Both regular files and symlinks to regular files can be copied, so unlike
                // `reflink` we don't want to report invalid input on both files and symlinks
                if from.is_file() {
                    err
                } else {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("the source path is not an existing regular file: {}", err),
                    )
                }
            })
        } else {
            Ok(None)
        }
    }

    inner(from.as_ref(), to.as_ref())
}
/// Checks whether reflink is supported on the filesystem for the specified source and target paths.
///
/// This function verifies that both paths are on the same volume and that the filesystem supports
/// reflink.
///
/// > Note: Currently the function works only for windows. It returns `Ok(ReflinkSupport::Unknown)`
/// > for any other platform.
///
/// # Example
/// ```
/// fn main() -> std::io::Result<()> {
///     let support = reflink_copy::check_reflink_support("C:\\path\\to\\file", "C:\\path\\to\\another_file")?;
///     println!("{support:?}");
///     let support = reflink_copy::check_reflink_support("path\\to\\folder", "path\\to\\another_folder")?;
///     println!("{support:?}");
///     Ok(())
/// }
/// ```
#[cfg_attr(not(windows), allow(unused_variables))]
pub fn check_reflink_support(
    from: impl AsRef<Path>,
    to: impl AsRef<Path>,
) -> io::Result<ReflinkSupport> {
    #[cfg(windows)]
    return sys::check_reflink_support(from, to);
    #[cfg(not(windows))]
    Ok(ReflinkSupport::Unknown)
}

/// Enum indicating the reflink support status.
#[derive(Debug, PartialEq, Eq)]
pub enum ReflinkSupport {
    /// Reflink is supported.
    Supported,
    /// Reflink is not supported.
    NotSupported,
    /// Reflink support is unconfirmed.
    Unknown,
}

pub use reflink_block::ReflinkBlockBuilder;
