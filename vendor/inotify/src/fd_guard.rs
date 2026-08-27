use std::{
    ops::Deref,
    os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, RawFd},
    sync::atomic::{AtomicBool, Ordering},
};

use inotify_sys as ffi;

use crate::util;

/// A RAII guard around a `RawFd` that closes it automatically on drop.
#[derive(Debug)]
pub struct FdGuard {
    pub(crate) fd: RawFd,
    pub(crate) close_on_drop: AtomicBool,
}

impl FdGuard {
    /// Indicate that the wrapped file descriptor should _not_ be closed
    /// when the guard is dropped.
    ///
    /// This should be called in cases where ownership of the wrapped file
    /// descriptor has been "moved" out of the guard.
    ///
    /// This is factored out into a separate function to ensure that it's
    /// always used consistently.
    #[inline]
    pub fn should_not_close(&self) {
        self.close_on_drop.store(false, Ordering::Release);
    }
}

impl Deref for FdGuard {
    type Target = RawFd;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.fd
    }
}

impl Drop for FdGuard {
    fn drop(&mut self) {
        if self.close_on_drop.load(Ordering::Acquire) {
            unsafe {
                ffi::close(self.fd);
            }
        }
    }
}

impl FromRawFd for FdGuard {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        FdGuard {
            fd,
            close_on_drop: AtomicBool::new(true),
        }
    }
}

impl IntoRawFd for FdGuard {
    fn into_raw_fd(self) -> RawFd {
        self.should_not_close();
        self.fd
    }
}

impl AsRawFd for FdGuard {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl AsFd for FdGuard {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.fd) }
    }
}

impl PartialEq for FdGuard {
    fn eq(&self, other: &FdGuard) -> bool {
        let initial = self.fd == other.fd;
        if initial {
            return true;
        }
        // This allows comparing duplicated Inotify descriptors that point to the
        // same Inotify instance, which allows for scenarios where an Inotify
        // wrapper both owns a file descriptor for control purposes and spawns a
        // second thread that needs a separate unowned descriptor to use the `epoll-rs`
        // crate.
        let current_process = std::process::id();
        kcmp_file_descriptors(current_process, self.fd, other.fd)
    }
}

fn kcmp_file_descriptors(current_process: u32, fd1: RawFd, fd2: RawFd) -> bool {
    match util::libc_convert(unsafe { kcmp_file(current_process, fd1, fd2) }) {
        Err(_) => false,
        Ok(cmp) => cmp == 0,
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe fn kcmp_file(current_process: u32, fd1: RawFd, fd2: RawFd) -> i64 {
    const KCMP_FILE: i32 = 0;

    libc::syscall(
        libc::SYS_kcmp,
        current_process,
        current_process,
        KCMP_FILE,
        fd1,
        fd2,
    ) as i64
}

#[cfg(target_os = "freebsd")]
unsafe fn kcmp_file(current_process: u32, fd1: RawFd, fd2: RawFd) -> i64 {
    libc::kcmp(
        current_process as libc::pid_t,
        current_process as libc::pid_t,
        libc::KCMP_FILE,
        fd1 as libc::c_ulong,
        fd2 as libc::c_ulong,
    ) as i64
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_os = "freebsd")))]
compile_error!("FdGuard equality requires kcmp support; add a target-specific kcmp_file implementation for this platform");
