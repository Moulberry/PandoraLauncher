use crate::sys::Sys;
use crate::sys::SysMetadata;
use crate::{NonFatalError, NonFatalErrorHandler};
use std::path::Path;

pub fn is_valid<F: NonFatalErrorHandler>(
    sys: impl Sys,
    path: &Path,
    nonfatal_error_handler: &mut F,
) -> bool {
    exists(&sys, path, nonfatal_error_handler) && is_executable(&sys, path, nonfatal_error_handler)
}

fn is_executable<F: NonFatalErrorHandler>(
    sys: impl Sys,
    path: &Path,
    nonfatal_error_handler: &mut F,
) -> bool {
    if sys.is_windows() && path.extension().is_some() {
        true
    } else {
        let ret = sys
            .is_valid_executable(path)
            .map_err(|e| nonfatal_error_handler.handle(NonFatalError::Io(e)))
            .unwrap_or(false);
        #[cfg(feature = "tracing")]
        tracing::trace!("{} EXEC_OK = {ret}", path.display());
        ret
    }
}

fn exists<F: NonFatalErrorHandler>(
    sys: impl Sys,
    path: &Path,
    nonfatal_error_handler: &mut F,
) -> bool {
    {
        if sys.is_windows() {
            let ret = sys
                .symlink_metadata(path)
                .map(|metadata| {
                    #[cfg(feature = "tracing")]
                    tracing::trace!(
                        "{} is_file() = {}, is_symlink() = {}",
                        path.display(),
                        metadata.is_file(),
                        metadata.is_symlink()
                    );
                    metadata.is_file() || metadata.is_symlink()
                })
                .map_err(|e| {
                    nonfatal_error_handler.handle(NonFatalError::Io(e));
                })
                .unwrap_or(false);
            #[cfg(feature = "tracing")]
            tracing::trace!(
                "{} has_extension = {}, checker::exists() = {ret}",
                path.display(),
                path.extension().is_some()
            );
            ret
        } else {
            let ret = sys.metadata(path).map(|metadata| metadata.is_file());
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
}
