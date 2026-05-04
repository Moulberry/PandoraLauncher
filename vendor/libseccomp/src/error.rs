// SPDX-License-Identifier: Apache-2.0 or MIT
//
// Copyright 2021 Sony Group Corporation
//

use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::ops::Deref;

pub(crate) type Result<T> = std::result::Result<T, SeccompError>;

/// Errnos returned by the libseccomp API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
// https://github.com/seccomp/libseccomp/blob/3c0dedd45713d7928c459b6523b78f4cfd435269/src/api.c#L60
pub enum SeccompErrno {
    /// The library doesn't permit the particular operation.
    EACCES,
    /// There was a system failure beyond the control of libseccomp.
    ECANCELED,
    /// Architecture/ABI specific failure.
    EDOM,
    /// Failure regarding the existence of argument.
    EEXIST,
    /// Internal libseccomp failure.
    EFAULT,
    /// Invalid input to the libseccomp API.
    EINVAL,
    /// No matching entry found.
    ENOENT,
    /// Unable to allocate enough memory to perform the requested operation.
    ENOMEM,
    /// The library doesn't support the particular operation.
    EOPNOTSUPP,
    /// Provided buffer is too small.
    ERANGE,
    /// Unable to load the filter due to thread issues.
    ESRCH,
}

impl SeccompErrno {
    fn strerror(&self) -> &'static str {
        use SeccompErrno::*;

        match self {
            EACCES => "The library doesn't permit the particular operation",
            ECANCELED => "There was a system failure beyond the control of libseccomp",
            EDOM => "Architecture/ABI specific failure",
            EEXIST => "Failure regarding the existence of argument",
            EFAULT => "Internal libseccomp failure",
            EINVAL => "Invalid input to the libseccomp API",
            ENOENT => "No matching entry found",
            ENOMEM => "Unable to allocate enough memory to perform the requested operation",
            EOPNOTSUPP => "The library doesn't support the particular operation",
            ERANGE => "Provided buffer is too small",
            ESRCH => "Unable to load the filter due to thread issues",
        }
    }

    fn to_sysrawrc(self) -> i32 {
        use SeccompErrno::*;

        match self {
            EACCES => libc::EACCES,
            ECANCELED => libc::ECANCELED,
            EDOM => libc::EDOM,
            EEXIST => libc::EEXIST,
            EFAULT => libc::EFAULT,
            EINVAL => libc::EINVAL,
            ENOENT => libc::ENOENT,
            ENOMEM => libc::ENOMEM,
            EOPNOTSUPP => libc::EOPNOTSUPP,
            ERANGE => libc::ERANGE,
            ESRCH => libc::ESRCH,
        }
    }
}

impl fmt::Display for SeccompErrno {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.strerror())
    }
}

/// A list specifying different categories of error.
#[derive(Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub(crate) enum ErrorKind {
    /// An error that represents error code on failure of the libseccomp API.
    Errno(SeccompErrno),
    /// A system's raw error code.
    SysRawRc(i32),
    /// An invalid Architecture.
    InvalidArch(u32),
    /// An invalid Action.
    InvalidAction(u32),
    /// An invalid string in from_str.
    FromStr(String),
    /// A lower-level error that is caused by an error from a lower-level module.
    Source,
    /// A custom error that does not fall under any other error kind.
    Common(Cow<'static, str>),
}

/// The error type for libseccomp operations.
pub struct SeccompError {
    kind: ErrorKind,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl SeccompError {
    pub(crate) fn new(kind: ErrorKind) -> Self {
        Self { kind, source: None }
    }

    pub(crate) fn with_source<E>(kind: ErrorKind, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            kind,
            source: Some(Box::new(source)),
        }
    }

    pub(crate) fn with_msg<M>(msg: M) -> Self
    where
        M: Into<Cow<'static, str>>,
    {
        Self {
            kind: ErrorKind::Common(msg.into()),
            source: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn with_msg_and_source<M, E>(msg: M, source: E) -> Self
    where
        M: Into<Cow<'static, str>>,
        E: Error + Send + Sync + 'static,
    {
        Self {
            kind: ErrorKind::Common(msg.into()),
            source: Some(Box::new(source)),
        }
    }

    pub(crate) fn from_errno(raw_errno: i32) -> Self {
        let seccomp_errno = match -raw_errno {
            libc::EACCES => SeccompErrno::EACCES,
            libc::ECANCELED => SeccompErrno::ECANCELED,
            libc::EDOM => SeccompErrno::EDOM,
            libc::EEXIST => SeccompErrno::EEXIST,
            libc::EFAULT => SeccompErrno::EFAULT,
            libc::EINVAL => SeccompErrno::EINVAL,
            libc::ENOENT => SeccompErrno::ENOENT,
            libc::ENOMEM => SeccompErrno::ENOMEM,
            libc::EOPNOTSUPP => SeccompErrno::EOPNOTSUPP,
            libc::ERANGE => SeccompErrno::ERANGE,
            libc::ESRCH => SeccompErrno::ESRCH,
            _ => return Self::new(ErrorKind::SysRawRc(raw_errno)),
        };
        Self::new(ErrorKind::Errno(seccomp_errno))
    }

    /// Query the errno returned by the libseccomp API.
    pub fn errno(&self) -> Option<SeccompErrno> {
        if let ErrorKind::Errno(errno) = self.kind {
            Some(errno)
        } else {
            None
        }
    }

    /// Query the system's raw error code returned when something goes wrong
    /// in the libc and the kernel.
    ///
    /// This function will be useful for users who want to extract the system's
    /// error code directly returned by [`ScmpFilterAttr::ApiSysRawRc`](`crate::ScmpFilterAttr::ApiSysRawRc`)
    /// , or get the errno returned by the libseccomp API as a negative integer rather than [`SeccompErrno`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// match set_api(1000) {
    ///     Err(e) => {
    ///         eprintln!("Error: {e}");
    ///         if let Some(sys) = e.sysrawrc() {
    ///             eprintln!("The system's raw error code: {sys}");
    ///             assert_eq!(sys, -libc::EINVAL);
    ///         }
    ///     }
    ///     _ => println!("No error"),
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn sysrawrc(&self) -> Option<i32> {
        match self.kind {
            ErrorKind::SysRawRc(rc) => Some(rc),
            ErrorKind::Errno(errno) => Some(-errno.to_sysrawrc()),
            _ => None,
        }
    }

    /// Returns the raw ffi value of an unsupported Action/Arch.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new(ScmpAction::Allow)?;
    /// if let Err(err) = ctx.get_act_default() {
    ///     println!("{:#?}", err.raw_ffi_value())
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn raw_ffi_value(&self) -> Option<u32> {
        match self.kind {
            ErrorKind::InvalidArch(v) | ErrorKind::InvalidAction(v) => Some(v),
            _ => None,
        }
    }

    fn msg(&self) -> Cow<'_, str> {
        match &self.kind {
            ErrorKind::Errno(e) => e.strerror().into(),
            ErrorKind::SysRawRc(e) => {
                format!("The system's raw error code({}) was returned", e).into()
            }
            ErrorKind::InvalidArch(_) => "Parse error by invalid architecture".into(),
            ErrorKind::InvalidAction(_) => "Parse error by invalid action".into(),
            ErrorKind::FromStr(s) => format!("Error while parsing '{s}'").into(),
            ErrorKind::Source => self.source.as_ref().unwrap().to_string().into(),
            ErrorKind::Common(s) => s.deref().into(),
        }
    }
}

impl fmt::Display for SeccompError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = self.msg();

        match &self.source {
            Some(source) if self.kind != ErrorKind::Source => {
                write!(f, "{} caused by: {}", msg, source)
            }
            Some(_) | None => {
                write!(f, "{}", msg)
            }
        }
    }
}

impl fmt::Debug for SeccompError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("kind", &self.kind)
            .field("source", &self.source)
            .field("message", &self.msg())
            .finish()
    }
}

impl Error for SeccompError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.source {
            Some(error) => Some(error.as_ref()),
            None => None,
        }
    }
}

/* Does not work without specialization (RFC 1210) or negative trait bounds
impl<T: Error> From<T> for SeccompError {
    fn from(err: T) -> Self {
        Self::with_source(ErrorKind::Source, err)
    }
}
*/

macro_rules! impl_seccomperror_from {
    ($errty:ty) => {
        impl From<$errty> for SeccompError {
            fn from(err: $errty) -> Self {
                Self::with_source(ErrorKind::Source, err)
            }
        }
    };
}
impl_seccomperror_from!(std::ffi::NulError);
impl_seccomperror_from!(std::num::TryFromIntError);
impl_seccomperror_from!(std::str::Utf8Error);

#[cfg(test)]
mod tests {
    use super::ErrorKind::*;
    use super::*;
    use std::ffi::CString;

    const TEST_ERR_MSG: &str = "test error";
    const TEST_NULL_STR: &str = "f\0oo";
    const NULL_ERR_MSG: &str = "nul byte found in provided data at position: 1";

    #[test]
    fn test_msg() {
        let null_err = CString::new(TEST_NULL_STR).unwrap_err();

        // Errno
        assert_eq!(
            SeccompError::from_errno(-libc::EACCES).msg(),
            SeccompErrno::EACCES.strerror()
        );
        assert_eq!(
            SeccompError::from_errno(-libc::ECANCELED).msg(),
            SeccompErrno::ECANCELED.strerror()
        );
        assert_eq!(
            SeccompError::from_errno(-libc::EDOM).msg(),
            SeccompErrno::EDOM.strerror()
        );
        assert_eq!(
            SeccompError::from_errno(-libc::EEXIST).msg(),
            SeccompErrno::EEXIST.strerror()
        );
        assert_eq!(
            SeccompError::from_errno(-libc::EFAULT).msg(),
            SeccompErrno::EFAULT.strerror()
        );
        assert_eq!(
            SeccompError::from_errno(-libc::EINVAL).msg(),
            SeccompErrno::EINVAL.strerror()
        );
        assert_eq!(
            SeccompError::from_errno(-libc::ENOENT).msg(),
            SeccompErrno::ENOENT.strerror()
        );
        assert_eq!(
            SeccompError::from_errno(-libc::ENOMEM).msg(),
            SeccompErrno::ENOMEM.strerror()
        );
        assert_eq!(
            SeccompError::new(Errno(SeccompErrno::EOPNOTSUPP)).msg(),
            SeccompErrno::EOPNOTSUPP.strerror(),
        );
        assert_eq!(
            SeccompError::from_errno(-libc::ERANGE).msg(),
            SeccompErrno::ERANGE.strerror()
        );
        assert_eq!(
            SeccompError::from_errno(-libc::ESRCH).msg(),
            SeccompErrno::ESRCH.strerror()
        );

        // Common
        assert_eq!(
            SeccompError::new(Common(TEST_ERR_MSG.into())).msg(),
            TEST_ERR_MSG
        );

        // Source
        assert_eq!(
            SeccompError::with_source(Source, null_err).msg(),
            NULL_ERR_MSG
        );

        // SysRawRc
        assert_eq!(
            SeccompError::from_errno(-libc::EPIPE).msg(),
            format!("The system's raw error code({}) was returned", -libc::EPIPE)
        );

        // InvalidArch
        assert_eq!(
            SeccompError::new(InvalidArch(123)).msg(),
            "Parse error by invalid architecture",
        );

        // InvalidAction
        assert_eq!(
            SeccompError::new(InvalidAction(123)).msg(),
            "Parse error by invalid action",
        );

        // FromStr
        assert_eq!(
            SeccompError::new(FromStr("SCMP".to_string())).msg(),
            "Error while parsing 'SCMP'",
        );
    }

    #[test]
    fn test_source() {
        let null_err = CString::new(TEST_NULL_STR).unwrap_err();

        assert!(SeccompError::new(Errno(SeccompErrno::EACCES))
            .source()
            .is_none());
        assert!(
            SeccompError::with_source(Errno(SeccompErrno::EACCES), null_err)
                .source()
                .is_some()
        );
    }

    #[test]
    fn test_with_msg() {
        assert_eq!(SeccompError::with_msg(TEST_ERR_MSG).msg(), TEST_ERR_MSG);
        assert!(SeccompError::with_msg(TEST_ERR_MSG).source().is_none());
    }

    #[test]
    fn test_with_msg_and_source() {
        let null_err = CString::new(TEST_NULL_STR).unwrap_err();

        assert_eq!(
            SeccompError::with_msg_and_source(TEST_ERR_MSG, null_err.clone()).msg(),
            TEST_ERR_MSG
        );
        assert!(SeccompError::with_msg_and_source(TEST_ERR_MSG, null_err)
            .source()
            .is_some());
    }

    #[test]
    fn test_errno() {
        assert_eq!(
            SeccompError::from_errno(-libc::EACCES).errno().unwrap(),
            SeccompErrno::EACCES
        );
        assert!(SeccompError::from_errno(libc::EBADFD).errno().is_none());
    }

    #[test]
    fn test_sysrawrc() {
        let tests = &[
            // The EBADFD is not handled by SeccompErrno
            libc::EBADFD,
            // The following errnos are handled by SeccompErrno
            libc::EACCES,
            libc::ECANCELED,
            libc::EDOM,
            libc::EEXIST,
            libc::EFAULT,
            libc::EINVAL,
            libc::ENOENT,
            libc::ENOMEM,
            libc::EOPNOTSUPP,
            libc::ERANGE,
            libc::ESRCH,
        ];

        for errno in tests {
            assert_eq!(SeccompError::from_errno(-errno).sysrawrc().unwrap(), -errno);
        }
        assert!(SeccompError::with_msg("no errno").sysrawrc().is_none());
    }

    #[test]
    fn test_raw_ffi_value() {
        assert_eq!(
            SeccompError::new(InvalidArch(123)).raw_ffi_value().unwrap(),
            123
        );
        assert_eq!(
            SeccompError::new(InvalidAction(123))
                .raw_ffi_value()
                .unwrap(),
            123
        );
        assert!(SeccompError::new(Common("".into()))
            .raw_ffi_value()
            .is_none());
    }

    #[test]
    fn test_from() {
        let null_err = CString::new(TEST_NULL_STR).unwrap_err();
        let scmp_err = SeccompError::from(null_err.clone());

        assert_eq!(scmp_err.kind, ErrorKind::Source);
        assert_eq!(scmp_err.source().unwrap().to_string(), null_err.to_string());
    }

    #[test]
    fn test_display() {
        let null_err = CString::new(TEST_NULL_STR).unwrap_err();

        // fmt::Display for SeccompErrno
        assert_eq!(
            format!("{}", SeccompErrno::EACCES),
            SeccompErrno::EACCES.strerror()
        );

        // Errno without source
        assert_eq!(
            format!("{}", SeccompError::new(Errno(SeccompErrno::EACCES))),
            SeccompErrno::EACCES.strerror()
        );
        // Errno with source
        assert_eq!(
            format!(
                "{}",
                SeccompError::with_source(Errno(SeccompErrno::EACCES), null_err.clone())
            ),
            format!(
                "{} caused by: {}",
                SeccompErrno::EACCES.strerror(),
                NULL_ERR_MSG
            )
        );

        // Common without source
        assert_eq!(
            format!("{}", SeccompError::new(Common(TEST_ERR_MSG.into()))),
            TEST_ERR_MSG
        );
        // Common with source
        assert_eq!(
            format!(
                "{}",
                SeccompError::with_source(Common(TEST_ERR_MSG.into()), null_err.clone())
            ),
            format!("{} caused by: {}", TEST_ERR_MSG, NULL_ERR_MSG)
        );

        // Source
        assert_eq!(
            format!("{}", SeccompError::with_source(ErrorKind::Source, null_err)),
            NULL_ERR_MSG
        );
    }

    #[test]
    fn test_debug() {
        let null_err = CString::new(TEST_NULL_STR).unwrap_err();

        // Errno without source
        assert_eq!(
            format!("{:?}", SeccompError::new(Errno(SeccompErrno::EACCES))),
            format!(
                "Error {{ kind: Errno({}), source: {}, message: \"{}\" }}",
                "EACCES",
                "None",
                SeccompErrno::EACCES.strerror()
            )
        );
        // Errno with source
        assert_eq!(
            format!(
                "{:?}",
                SeccompError::with_source(Errno(SeccompErrno::EACCES), null_err),
            ),
            format!(
                "Error {{ kind: Errno({}), source: {}, message: \"{}\" }}",
                "EACCES",
                "Some(NulError(1, [102, 0, 111, 111]))",
                SeccompErrno::EACCES.strerror()
            )
        );
    }
}
