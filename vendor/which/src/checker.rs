use crate::finder::Checker;
use crate::{NonFatalError, NonFatalErrorHandler};
use std::fs;
use std::path::Path;

pub struct ExecutableChecker;

impl ExecutableChecker {
    pub fn new() -> ExecutableChecker {
        ExecutableChecker
    }
}

impl Checker for ExecutableChecker {
    #[cfg(any(unix, target_os = "wasi", target_os = "redox"))]
    fn is_valid<F: NonFatalErrorHandler>(
        &self,
        path: &Path,
        nonfatal_error_handler: &mut F,
    ) -> bool {
        use std::io;

        use rustix::fs as rfs;
        let ret = rfs::access(path, rfs::Access::EXEC_OK)
            .map_err(|e| {
                nonfatal_error_handler.handle(NonFatalError::Io(io::Error::from_raw_os_error(
                    e.raw_os_error(),
                )))
            })
            .is_ok();
        #[cfg(feature = "tracing")]
        tracing::trace!("{} EXEC_OK = {ret}", path.display());
        ret
    }

    #[cfg(windows)]
    fn is_valid<F: NonFatalErrorHandler>(
        &self,
        _path: &Path,
        _nonfatal_error_handler: &mut F,
    ) -> bool {
        true
    }
}

pub struct ExistedChecker;

impl ExistedChecker {
    pub fn new() -> ExistedChecker {
        ExistedChecker
    }
}

impl Checker for ExistedChecker {
    #[cfg(target_os = "windows")]
    fn is_valid<F: NonFatalErrorHandler>(
        &self,
        path: &Path,
        nonfatal_error_handler: &mut F,
    ) -> bool {
        let ret = fs::symlink_metadata(path)
            .map(|metadata| {
                let file_type = metadata.file_type();
                #[cfg(feature = "tracing")]
                tracing::trace!(
                    "{} is_file() = {}, is_symlink() = {}",
                    path.display(),
                    file_type.is_file(),
                    file_type.is_symlink()
                );
                file_type.is_file() || file_type.is_symlink()
            })
            .map_err(|e| {
                nonfatal_error_handler.handle(NonFatalError::Io(e));
            })
            .unwrap_or(false)
            && (path.extension().is_some() || matches_arch(path, nonfatal_error_handler));
        #[cfg(feature = "tracing")]
        tracing::trace!(
            "{} has_extension = {}, ExistedChecker::is_valid() = {ret}",
            path.display(),
            path.extension().is_some()
        );
        ret
    }

    #[cfg(not(target_os = "windows"))]
    fn is_valid<F: NonFatalErrorHandler>(
        &self,
        path: &Path,
        nonfatal_error_handler: &mut F,
    ) -> bool {
        let ret = fs::metadata(path).map(|metadata| metadata.is_file());
        #[cfg(feature = "tracing")]
        tracing::trace!("{} is_file() = {ret:?}", path.display());
        match ret {
            Ok(ret) => ret,
            Err(e) => {
                nonfatal_error_handler.handle(NonFatalError::Io(e));
                false
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn matches_arch<F: NonFatalErrorHandler>(path: &Path, nonfatal_error_handler: &mut F) -> bool {
    use std::io;

    let ret = winsafe::GetBinaryType(&path.display().to_string())
        .map_err(|e| {
            nonfatal_error_handler.handle(NonFatalError::Io(io::Error::from_raw_os_error(
                e.raw() as i32
            )))
        })
        .is_ok();
    #[cfg(feature = "tracing")]
    tracing::trace!("{} matches_arch() = {ret}", path.display());
    ret
}

pub struct CompositeChecker {
    existed_checker: ExistedChecker,
    executable_checker: ExecutableChecker,
}

impl CompositeChecker {
    pub fn new() -> CompositeChecker {
        CompositeChecker {
            executable_checker: ExecutableChecker::new(),
            existed_checker: ExistedChecker::new(),
        }
    }
}

impl Checker for CompositeChecker {
    fn is_valid<F: NonFatalErrorHandler>(
        &self,
        path: &Path,
        nonfatal_error_handler: &mut F,
    ) -> bool {
        self.existed_checker.is_valid(path, nonfatal_error_handler)
            && self
                .executable_checker
                .is_valid(path, nonfatal_error_handler)
    }
}
