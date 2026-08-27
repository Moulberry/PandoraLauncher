use std::borrow::Cow;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::path::PathBuf;

pub trait SysReadDirEntry {
    /// Gets the file name of the directory entry, not the full path.
    fn file_name(&self) -> OsString;
    /// Gets the full path of the directory entry.
    fn path(&self) -> PathBuf;
}

pub trait SysMetadata {
    /// Gets if the path is a symlink.
    fn is_symlink(&self) -> bool;
    /// Gets if the path is a file.
    fn is_file(&self) -> bool;
}

/// Represents the system that `which` interacts with to get information
/// about the environment and file system.
///
/// ### How to use in Wasm without WASI
///
/// WebAssembly without WASI does not have a filesystem, but using this crate is possible in `wasm32-unknown-unknown` targets by disabling default features:
///
/// ```toml
/// which = { version = "...", default-features = false }
/// ```
///
// Then providing your own implementation of the `which::sys::Sys` trait:
///
/// ```rs
/// use which::WhichConfig;
///
/// struct WasmSys;
///
/// impl which::sys::Sys for WasmSys {
///     // it is up to you to implement this trait based on the
///     // environment you are running WebAssembly in
/// }
///
/// let paths = WhichConfig::new_with_sys(WasmSys)
///     .all_results()
///     .unwrap()
///     .collect::<Vec<_>>();
/// ```
pub trait Sys {
    type ReadDirEntry: SysReadDirEntry;
    type Metadata: SysMetadata;

    /// Check if the current platform is Windows.
    ///
    /// This can be set to true in wasm32-unknown-unknown targets that
    /// are running on Windows systems.
    fn is_windows(&self) -> bool;
    /// Gets the current working directory.
    fn current_dir(&self) -> io::Result<PathBuf>;
    /// Gets the home directory of the current user.
    fn home_dir(&self) -> Option<PathBuf>;
    /// Splits a platform-specific PATH variable into a list of paths.
    fn env_split_paths(&self, paths: &OsStr) -> Vec<PathBuf>;
    /// Gets the value of the PATH environment variable.
    fn env_path(&self) -> Option<OsString>;
    /// Gets the value of the PATHEXT environment variable. If not on Windows, simply return None.
    fn env_path_ext(&self) -> Option<OsString>;
    /// Gets and parses the PATHEXT environment variable on Windows.
    ///
    /// Override this to enable caching the parsed PATHEXT.
    ///
    /// Note: This will only be called when `is_windows()` returns `true`
    /// and isn't conditionally compiled with `#[cfg(windows)]` so that it
    /// can work in Wasm.
    fn env_windows_path_ext(&self) -> Cow<'static, [String]> {
        Cow::Owned(parse_path_ext(self.env_path_ext()))
    }
    /// Gets the metadata of the provided path, following symlinks.
    fn metadata(&self, path: &Path) -> io::Result<Self::Metadata>;
    /// Gets the metadata of the provided path, not following symlinks.
    fn symlink_metadata(&self, path: &Path) -> io::Result<Self::Metadata>;
    /// Reads the directory entries of the provided path.
    fn read_dir(
        &self,
        path: &Path,
    ) -> io::Result<Box<dyn Iterator<Item = io::Result<Self::ReadDirEntry>>>>;
    /// Checks if the provided path is a valid executable.
    fn is_valid_executable(&self, path: &Path) -> io::Result<bool>;
}

impl SysReadDirEntry for std::fs::DirEntry {
    fn file_name(&self) -> OsString {
        self.file_name()
    }

    fn path(&self) -> PathBuf {
        self.path()
    }
}

impl SysMetadata for std::fs::Metadata {
    fn is_symlink(&self) -> bool {
        self.file_type().is_symlink()
    }

    fn is_file(&self) -> bool {
        self.file_type().is_file()
    }
}

#[cfg(feature = "real-sys")]
#[derive(Default, Clone, Copy)]
pub struct RealSys;

#[cfg(feature = "real-sys")]
impl RealSys {
    #[inline]
    pub(crate) fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        #[allow(clippy::disallowed_methods)] // ok, sys implementation
        std::fs::canonicalize(path)
    }
}

#[cfg(feature = "real-sys")]
impl Sys for RealSys {
    type ReadDirEntry = std::fs::DirEntry;
    type Metadata = std::fs::Metadata;

    #[inline]
    fn is_windows(&self) -> bool {
        // Again, do not change the code to directly use `#[cfg(windows)]`
        // because we want to allow people to implement this code in Wasm
        // and then tell at runtime if running on a Windows system.
        cfg!(windows)
    }

    #[inline]
    fn current_dir(&self) -> io::Result<PathBuf> {
        #[allow(clippy::disallowed_methods)] // ok, sys implementation
        std::env::current_dir()
    }

    #[inline]
    fn home_dir(&self) -> Option<PathBuf> {
        #[allow(clippy::disallowed_methods)] // ok, sys implementation
        #[allow(deprecated)] // only deprecated <1.85
        std::env::home_dir()
    }

    #[inline]
    fn env_split_paths(&self, paths: &OsStr) -> Vec<PathBuf> {
        #[allow(clippy::disallowed_methods)] // ok, sys implementation
        std::env::split_paths(paths).collect()
    }

    fn env_windows_path_ext(&self) -> Cow<'static, [String]> {
        use std::sync::OnceLock;

        // Sample %PATHEXT%: .COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC
        // PATH_EXTENSIONS is then [".COM", ".EXE", ".BAT", …].
        // (In one use of PATH_EXTENSIONS we skip the dot, but in the other we need it;
        // hence its retention.)
        static PATH_EXTENSIONS: OnceLock<Vec<String>> = OnceLock::new();
        let path_extensions = PATH_EXTENSIONS.get_or_init(|| parse_path_ext(self.env_path_ext()));
        Cow::Borrowed(path_extensions)
    }

    #[inline]
    fn env_path(&self) -> Option<OsString> {
        #[allow(clippy::disallowed_methods)] // ok, sys implementation
        std::env::var_os("PATH")
    }

    #[inline]
    fn env_path_ext(&self) -> Option<OsString> {
        #[allow(clippy::disallowed_methods)] // ok, sys implementation
        std::env::var_os("PATHEXT")
    }

    #[inline]
    fn read_dir(
        &self,
        path: &Path,
    ) -> io::Result<Box<dyn Iterator<Item = io::Result<Self::ReadDirEntry>>>> {
        #[allow(clippy::disallowed_methods)] // ok, sys implementation
        let iter = std::fs::read_dir(path)?;
        Ok(Box::new(iter))
    }

    #[inline]
    fn metadata(&self, path: &Path) -> io::Result<Self::Metadata> {
        #[allow(clippy::disallowed_methods)] // ok, sys implementation
        std::fs::metadata(path)
    }

    #[inline]
    fn symlink_metadata(&self, path: &Path) -> io::Result<Self::Metadata> {
        #[allow(clippy::disallowed_methods)] // ok, sys implementation
        std::fs::symlink_metadata(path)
    }

    #[cfg(any(unix, target_os = "wasi", target_os = "redox"))]
    fn is_valid_executable(&self, path: &Path) -> io::Result<bool> {
        use std::ffi::CString;
        #[cfg(any(unix, target_os = "redox"))]
        use std::os::unix::ffi::OsStrExt;
        #[cfg(target_os = "wasi")]
        use std::os::wasi::ffi::OsStrExt;

        let path = CString::new(path.as_os_str().as_bytes())?;
        if unsafe { libc::access(path.as_ptr(), libc::X_OK) } == 0 {
            Ok(true)
        } else {
            Err(io::Error::last_os_error())
        }
    }

    #[cfg(windows)]
    fn is_valid_executable(&self, path: &Path) -> io::Result<bool> {
        use std::os::windows::ffi::OsStrExt;

        use crate::win_ffi;
        let w_str: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut binary_type = 0u32;
        unsafe {
            let success = win_ffi::GetBinaryTypeW(w_str.as_ptr(), &mut binary_type);
            if success != 0 {
                Ok(true)
            } else {
                Err(io::Error::from_raw_os_error(win_ffi::GetLastError() as i32))
            }
        }
    }

    #[cfg(not(any(unix, windows, target_os = "wasi", target_os = "redox")))]
    fn is_valid_executable(&self, _path: &Path) -> io::Result<bool> {
        Ok(false)
    }
}

impl<T> Sys for &T
where
    T: Sys,
{
    type ReadDirEntry = T::ReadDirEntry;

    type Metadata = T::Metadata;

    fn is_windows(&self) -> bool {
        (*self).is_windows()
    }

    fn current_dir(&self) -> io::Result<PathBuf> {
        (*self).current_dir()
    }

    fn home_dir(&self) -> Option<PathBuf> {
        (*self).home_dir()
    }

    fn env_split_paths(&self, paths: &OsStr) -> Vec<PathBuf> {
        (*self).env_split_paths(paths)
    }

    fn env_path(&self) -> Option<OsString> {
        (*self).env_path()
    }

    fn env_path_ext(&self) -> Option<OsString> {
        (*self).env_path_ext()
    }

    fn metadata(&self, path: &Path) -> io::Result<Self::Metadata> {
        (*self).metadata(path)
    }

    fn symlink_metadata(&self, path: &Path) -> io::Result<Self::Metadata> {
        (*self).symlink_metadata(path)
    }

    fn read_dir(
        &self,
        path: &Path,
    ) -> io::Result<Box<dyn Iterator<Item = io::Result<Self::ReadDirEntry>>>> {
        (*self).read_dir(path)
    }

    fn is_valid_executable(&self, path: &Path) -> io::Result<bool> {
        (*self).is_valid_executable(path)
    }
}

fn parse_path_ext(pathext: Option<OsString>) -> Vec<String> {
    pathext
        .and_then(|pathext| {
            // If tracing feature enabled then this lint is incorrect, so disable it.
            #[allow(clippy::manual_ok_err)]
            match pathext.into_string() {
                Ok(pathext) => Some(pathext),
                Err(_) => {
                    #[cfg(feature = "tracing")]
                    tracing::error!("pathext is not valid unicode");
                    None
                }
            }
        })
        .map(|pathext| {
            pathext
                .split(';')
                .filter_map(|s| {
                    if s.as_bytes().first() == Some(&b'.') {
                        Some(s.to_owned())
                    } else {
                        // Invalid segment; just ignore it.
                        #[cfg(feature = "tracing")]
                        tracing::debug!("PATHEXT segment \"{s}\" missing leading dot, ignoring");
                        None
                    }
                })
                .collect()
        })
        // PATHEXT not being set or not being a proper Unicode string is exceedingly
        // improbable and would probably break Windows badly. Still, don't crash:
        .unwrap_or_default()
}
