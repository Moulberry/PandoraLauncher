/*!
Library for working with NTFS junctions.

Junction Points are a little known NTFS v5+ feature roughly equivalent to Unix
directory symbolic links.

They are supported in Windows 2000 and onwards, where a directory
serves as a symbolic link to another directory on the computer. For example,
if the directory `D:\SYMLINK` specified `C:\WINNT\SYSTEM32` as its target, then
an application accessing `D:\SYMLINK\DRIVERS` would in reality be accessing
`C:\WINNT\SYSTEM32\DRIVERS`.
*/
#![doc(html_root_url = "https://docs.rs/junction/~1")]
#![cfg(windows)]
#![deny(rust_2021_compatibility)]

mod internals;

#[cfg(test)]
mod tests;

use std::io;
use std::path::{Path, PathBuf};

/// Creates a junction point from the specified directory to the specified target directory.
///
/// N.B. Only works on NTFS.
///
/// # Error
///
/// This function may error if the `junction` path already exists.
///
/// # Example
///
/// ```rust
/// use std::io;
/// use std::path::Path;
/// # use std::fs;
/// # use junction::create;
/// fn main() -> io::Result<()> {
///     let tmpdir = tempfile::tempdir()?;
///     let target = tmpdir.path().join("target");
///     let junction = tmpdir.path().join("junction");
///     # fs::create_dir_all(&target)?;
///     create(&target, &junction)
/// }
/// ```
pub fn create<P, Q>(target: P, junction: Q) -> io::Result<()>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    internals::create(target.as_ref(), junction.as_ref())
}

/// Deletes a `junction` reparse point from the specified file or directory.
///
/// N.B. Only works on NTFS.
///
/// This function delete the junction point only, leaving the target directory
/// and its content as is. It does nothing if the `junction` point does not exist.
///
/// # Example
///
/// ```rust
/// use std::io;
/// use std::path::Path;
/// # use std::fs;
/// # use junction::{create, delete};
/// fn main() -> io::Result<()> {
///     let tmpdir = tempfile::tempdir()?;
///     let target = tmpdir.path().join("target");
///     let junction = tmpdir.path().join("junction");
///     # fs::create_dir_all(&target)?;
///     create(&target, &junction)?;
///     delete(&junction)
/// }
/// ```
pub fn delete<P: AsRef<Path>>(junction: P) -> io::Result<()> {
    internals::delete(junction.as_ref())
}

/// Determines whether the specified path exists and refers to a junction point.
///
/// # Example
///
/// ```rust
/// use std::io;
/// # use junction::exists;
/// fn main() -> io::Result<()> {
///     # #[cfg(feature = "unstable_admin")]
///     assert!(exists(r"C:\Users\Default User")?);
///     Ok(())
/// }
/// ```
pub fn exists<P: AsRef<Path>>(junction: P) -> io::Result<bool> {
    internals::exists(junction.as_ref())
}

/// Gets the target of the specified junction point.
///
/// N.B. Only works on NTFS.
///
/// # Example
///
/// ```rust
/// use std::io;
/// # use junction::get_target;
/// fn main() -> io::Result<()> {
///     # #[cfg(feature = "unstable_admin")]
///     assert_eq!(get_target(r"C:\Users\Default User")?.to_str(), Some(r"C:\Users\Default"));
///     Ok(())
/// }
/// ```
pub fn get_target<P: AsRef<Path>>(junction: P) -> io::Result<PathBuf> {
    internals::get_target(junction.as_ref())
}
