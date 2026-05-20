// SPDX-License-Identifier: Apache-2.0 or MIT
//
// Copyright 2021 Sony Group Corporation
//

use crate::error::{Result, SeccompError};
use crate::ScmpArch;
use libseccomp_sys::*;
use std::ffi::CStr;
use std::fmt;
use std::os::raw::c_char;

#[cfg(feature = "const-syscall")]
cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        mod x86_64;
        use x86_64::SYSCALLS;
    } else if #[cfg(target_arch = "aarch64")] {
        mod aarch64;
        use aarch64::SYSCALLS;
    } else if #[cfg(target_arch = "arm")] {
        mod arm;
        use arm::SYSCALLS;
    } else if #[cfg(target_arch = "loongarch64")] {
        mod loongarch64;
        use loongarch64::SYSCALLS;
    } else if #[cfg(target_arch = "m68k")] {
        mod m68k;
        use m68k::SYSCALLS;
    } else if #[cfg(target_arch = "mips")] {
        mod mips;
        use mips::SYSCALLS;
    } else if #[cfg(target_arch = "mips64")] {
        mod mips64;
        use mips64::SYSCALLS;
    } else if #[cfg(target_arch = "powerpc")] {
        mod powerpc;
        use powerpc::SYSCALLS;
    } else if #[cfg(target_arch = "powerpc64")] {
        mod powerpc64;
        use powerpc64::SYSCALLS;
    } else if #[cfg(target_arch = "riscv64")] {
        mod riscv64;
        use riscv64::SYSCALLS;
    } else if #[cfg(target_arch = "s390x")] {
        mod s390x;
        use s390x::SYSCALLS;
    } else if #[cfg(target_arch = "x86")] {
        mod x86;
        use x86::SYSCALLS;
    } else {
        compile_error!("Looks like your target_arch is not supported by libseccomp.");
    }
}

/// A raw syscall as used by the OS.
pub type RawSyscall = libc::c_int;

/// Represents a syscall number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScmpSyscall {
    nr: i32,
}
impl ScmpSyscall {
    pub(crate) fn to_sys(self) -> i32 {
        self.nr
    }

    pub(crate) fn from_sys(nr: i32) -> Self {
        Self { nr }
    }

    /// Resolves a syscall name to `ScmpSyscall`.
    ///
    /// This function returns a `ScmpSyscall` that can be passed to
    /// [`add_rule`](crate::ScmpFilterContext::add_rule) like functions.
    /// Or `ScmpSyscall::from(libseccomp_sys::__NR_SCMP_ERROR)` if name is unknown.
    ///
    /// Unlike [`from_name`](Self::from_name) this function does not any FFI call
    /// and can therefore be `const`.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of a syscall
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let syscall = ScmpSyscall::new("chroot");
    /// ```
    #[cfg(feature = "const-syscall")]
    #[cfg_attr(docsrs, doc(cfg(feature = "const-syscall")))]
    pub const fn new(name: &str) -> Self {
        let mut i = 0;
        let nr = loop {
            if i >= SYSCALLS.len() {
                break libseccomp_sys::__NR_SCMP_ERROR;
            }
            if strcmp(SYSCALLS[i].0, name) {
                break SYSCALLS[i].1;
            }
            i += 1;
        };

        Self { nr }
    }

    /// Resolves a syscall name to `ScmpSyscall`.
    ///
    /// This function returns a `ScmpSyscall` that can be passed to
    /// [`add_rule`](crate::ScmpFilterContext::add_rule) like functions.
    ///
    /// This function corresponds to
    /// [`seccomp_syscall_resolve_name`](https://man7.org/linux/man-pages/man3/seccomp_syscall_resolve_name.3.html).
    ///
    /// # Arguments
    ///
    /// * `name` - The name of a syscall
    ///
    /// # Errors
    ///
    ///  If an invalid string for the syscall name is specified or a syscall with that
    ///  name is not found, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let syscall = ScmpSyscall::from_name("chroot")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_name(name: &str) -> Result<Self> {
        Self::from_name_by_arch(name, ScmpArch::Native)
    }

    /// Resolves a syscall name to `ScmpSyscall`.
    ///
    /// NOTE: If you call this function with a foreign architecture token and pass the result
    /// to [`add_rule*`](crate::ScmpFilterContext::add_rule) functions you get unexpected results.
    ///
    /// This function returns a `ScmpSyscall` for the specified architecture.
    ///
    /// This function corresponds to
    /// [`seccomp_syscall_resolve_name_arch`](https://man7.org/linux/man-pages/man3/seccomp_syscall_resolve_name_arch.3.html).
    ///
    /// # Arguments
    ///
    /// * `name` - The name of a syscall
    /// * `arch` - An architecture token
    ///
    /// # Errors
    ///
    ///  If an invalid string for the syscall name is specified or a syscall with that
    ///  name is not found, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let syscall = ScmpSyscall::from_name_by_arch("chroot", ScmpArch::Aarch64)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_name_by_arch(name: &str, arch: ScmpArch) -> Result<Self> {
        Self::from_name_by_impl(seccomp_syscall_resolve_name_arch, name, arch)
    }

    /// Resolves a syscall name to `ScmpSyscall`.
    ///
    /// This function returns a `ScmpSyscall` for the specified architecture
    /// rewritten if necessary.
    ///
    /// This function corresponds to
    /// [`seccomp_syscall_resolve_name_rewrite`](https://man7.org/linux/man-pages/man3/seccomp_syscall_resolve_name_rewrite.3.html).
    ///
    /// # Arguments
    ///
    /// * `name` - The name of a syscall
    /// * `arch` - An architecture token
    ///
    /// # Errors
    ///
    ///  If an invalid string for the syscall name is specified or a syscall with that
    ///  name is not found, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let syscall = ScmpSyscall::from_name_by_arch_rewrite("socketcall", ScmpArch::X32)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_name_by_arch_rewrite(name: &str, arch: ScmpArch) -> Result<Self> {
        Self::from_name_by_impl(seccomp_syscall_resolve_name_rewrite, name, arch)
    }

    fn from_name_by_impl(
        resolve_name_func: unsafe extern "C" fn(arch_token: u32, name: *const c_char) -> i32,
        name: &str,
        arch: ScmpArch,
    ) -> Result<Self> {
        let mut buf = [0_u8; 64];
        if name.as_bytes().contains(&b'\0') {
            return Err(SeccompError::with_msg(format!(
                "Bad syscall name '{name}': Contains nul-byte.",
            )));
        }
        if buf.len() <= name.len() {
            return Err(SeccompError::with_msg(format!(
                "Bad syscall name '{name}': Too long.",
            )));
        }
        buf[..name.len()].copy_from_slice(name.as_bytes());
        debug_assert_eq!(buf[name.len()], b'\0');

        let nr = unsafe { resolve_name_func(arch.to_sys(), buf.as_slice().as_ptr().cast()) };
        if nr == __NR_SCMP_ERROR {
            return Err(SeccompError::with_msg(format!(
                "Could not resolve syscall name {}",
                name
            )));
        }

        Ok(Self { nr })
    }

    /// Resolves this `ScmpSyscall` to it's name for the native architecture.
    ///
    /// This function returns a string containing the name of the syscall.
    ///
    /// This function corresponds to
    /// [`seccomp_syscall_resolve_num_arch`](https://man7.org/linux/man-pages/man3/seccomp_syscall_resolve_num_arch.3.html).
    ///
    /// # Errors
    ///
    /// If the syscall is unrecognized or an issue is encountered getting the
    /// name of the syscall, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// assert_eq!(
    ///     ScmpSyscall::from_name("mount")?.get_name()?,
    ///     String::from("mount")
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_name(self) -> Result<String> {
        Self::get_name_by_arch(self, ScmpArch::Native)
    }

    /// Resolves this `ScmpSyscall` to it's name for a given architecture.
    ///
    /// This function returns a string containing the name of the syscall.
    ///
    /// This function corresponds to
    /// [`seccomp_syscall_resolve_num_arch`](https://man7.org/linux/man-pages/man3/seccomp_syscall_resolve_num_arch.3.html).
    ///
    /// # Arguments
    ///
    /// * `arch` - A valid architecture token
    ///
    /// # Errors
    ///
    /// If the syscall is unrecognized or an issue is encountered getting the
    /// name of the syscall, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// assert_eq!(
    ///     ScmpSyscall::from_name_by_arch("mount", ScmpArch::Mips)?
    ///         .get_name_by_arch(ScmpArch::Mips)?,
    ///     String::from("mount"),
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_name_by_arch(self, arch: ScmpArch) -> Result<String> {
        let ret = unsafe { seccomp_syscall_resolve_num_arch(arch.to_sys(), self.to_sys()) };
        if ret.is_null() {
            return Err(SeccompError::with_msg(format!(
                "Could not resolve syscall number {}",
                self.nr
            )));
        }

        let name = unsafe { CStr::from_ptr(ret) }.to_str()?.to_string();
        unsafe { libc::free(ret as *mut libc::c_void) };

        Ok(name)
    }

    /// Returns `true` if this syscall is `__NR_SCMP_ERROR` (`-1`), otherwise `false`.
    pub fn is_error(&self) -> bool {
        self.nr == libseccomp_sys::__NR_SCMP_ERROR
    }

    /// Returns `true` if this syscall is `__NR_SCMP_UNDEF` (`-2`), otherwise `false`.
    pub fn is_undef(&self) -> bool {
        self.nr == libseccomp_sys::__NR_SCMP_UNDEF
    }

    /// Creates a `ScmpSyscall` from a `RawSyscall`.
    pub fn from_raw_syscall(raw_syscall: RawSyscall) -> Self {
        Self::from(raw_syscall)
    }

    /// Returns the `RawSyscall` of this `ScmpSyscall`.
    pub fn as_raw_syscall(self) -> RawSyscall {
        self.nr
    }
}

impl From<RawSyscall> for ScmpSyscall {
    /// Creates a `ScmpSyscall` from the specified syscall number.
    ///
    /// # Arguments
    ///
    /// * `nr` - The number of syscall
    fn from(nr: RawSyscall) -> Self {
        Self::from_sys(nr)
    }
}

impl From<ScmpSyscall> for RawSyscall {
    /// Gets the syscall number of a syscall.
    ///
    /// # Arguments
    ///
    /// * `syscall` - The syscall
    fn from(syscall: ScmpSyscall) -> RawSyscall {
        syscall.nr
    }
}

impl PartialEq<RawSyscall> for ScmpSyscall {
    fn eq(&self, other: &RawSyscall) -> bool {
        self.nr == *other
    }
}

impl PartialEq<ScmpSyscall> for RawSyscall {
    fn eq(&self, other: &ScmpSyscall) -> bool {
        *self == other.nr
    }
}

impl fmt::Display for ScmpSyscall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.nr)
    }
}

/// Compare two strings
///
/// This is a helper function because `&str == &str` is not `const` yet.
///
/// This function returns the same as `lhs == rhs`.
#[cfg(feature = "const-syscall")]
const fn strcmp(lhs: &str, rhs: &str) -> bool {
    if lhs.len() != rhs.len() {
        return false;
    }
    let (lhs, rhs) = (lhs.as_bytes(), rhs.as_bytes());
    let mut i = 0;
    while i < lhs.len() && i < rhs.len() {
        if lhs[i] != rhs[i] {
            return false;
        }
        i += 1;
    }
    true
}
