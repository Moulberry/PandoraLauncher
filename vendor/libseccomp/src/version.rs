// SPDX-License-Identifier: Apache-2.0 or MIT
//
// Copyright 2021 Sony Group Corporation
//

use crate::error::{Result, SeccompError};
use libseccomp_sys::*;
use std::fmt;

/// Represents the version information of the libseccomp library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScmpVersion {
    /// The major version
    pub major: u32,
    /// The minor version
    pub minor: u32,
    /// The micro version
    pub micro: u32,
}

impl ScmpVersion {
    /// Gets the version of the currently loaded libseccomp library.
    ///
    /// This function returns `ScmpVersion` that represents the currently
    /// loaded libseccomp version.
    ///
    /// This function corresponds to
    /// [`seccomp_version`](https://man7.org/linux/man-pages/man3/seccomp_version.3.html).
    ///
    /// # Errors
    ///
    /// If this function encounters an issue while getting the version,
    /// an error will be returned.
    pub fn current() -> Result<Self> {
        if let Some(version) = unsafe { seccomp_version().as_ref() } {
            Ok(Self {
                major: version.major,
                minor: version.minor,
                micro: version.micro,
            })
        } else {
            Err(SeccompError::with_msg("Could not get libseccomp version"))
        }
    }
}

impl From<(u32, u32, u32)> for ScmpVersion {
    /// Creates a `ScmpVersion` from the specified arbitrary version.
    ///
    /// # Arguments
    ///
    /// * `version` - A tuple that represents the version of the libseccomp library.
    ///   The index 0, 1, and 2 represent `major`, `minor`, and `micro` respectively.
    fn from(version: (u32, u32, u32)) -> Self {
        Self {
            major: version.0,
            minor: version.1,
            micro: version.2,
        }
    }
}

impl fmt::Display for ScmpVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.micro)
    }
}

/// Checks that the libseccomp version being used is equal to or greater than
/// the specified version.
///
/// This function returns `Ok(true)` if the libseccomp version is equal to
/// or greater than the specified version, `Ok(false)` otherwise.
///
/// # Arguments
///
/// * `expected` - The libseccomp version you want to check
///
/// # Errors
///
/// If an issue is encountered getting the current version, an error will be returned.
///
/// # Examples
///
/// ```
/// # use libseccomp::*;
/// check_version(ScmpVersion::from((2, 4, 0)))?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn check_version(expected: ScmpVersion) -> Result<bool> {
    let current = ScmpVersion::current()?;

    if current.major > expected.major
        || (current.major == expected.major && current.minor > expected.minor)
        || (current.major == expected.major
            && current.minor == expected.minor
            && current.micro >= expected.micro)
    {
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Ensures that the libseccomp version is equal to or greater than the
/// specified version.
///
/// # Arguments
///
/// * `msg` - An arbitrary non-empty operation description, used as a part
///   of the error message returned.
/// * `expected` - The libseccomp version you want to check
///
/// # Errors
///
/// If the libseccomp version being used is less than the specified version,
/// an error will be returned.
pub(crate) fn ensure_supported_version(msg: &str, expected: ScmpVersion) -> Result<()> {
    if check_version(expected)? {
        Ok(())
    } else {
        let current = ScmpVersion::current()?;
        Err(SeccompError::with_msg(format!(
            "{} requires libseccomp >= {} (current version: {})",
            msg, expected, current,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ScmpVersionTest {
        pub msg: &'static str,
        pub ver: ScmpVersion,
        pub is_ok: bool,
    }
    impl ScmpVersionTest {
        pub fn new(msg: &'static str, ver: ScmpVersion, is_ok: bool) -> Self {
            Self { msg, ver, is_ok }
        }
    }

    #[test]
    fn test_ensure_supported_version() {
        let ver = ScmpVersion::current().unwrap();
        let tests = &[
            ScmpVersionTest::new(
                "VerCurrent",
                ScmpVersion::from((ver.major, ver.minor, ver.micro)),
                true,
            ),
            ScmpVersionTest::new("VerMajor-1", ScmpVersion::from((ver.major - 1, 0, 0)), true),
            ScmpVersionTest::new(
                // To avoid a failure at v*.0.*
                "If VerMinor != 0 then VerMinor-1, otherwise VerMinor=0",
                ScmpVersion::from((ver.major, if ver.minor != 0 { ver.minor - 1 } else { 0 }, 0)),
                true,
            ),
            ScmpVersionTest::new(
                // To avoid a failure at v*.*.0
                "If VerMicro != 0 then VerMicro-1, otherwise VerMicro=0",
                ScmpVersion::from((
                    ver.major,
                    ver.minor,
                    if ver.micro != 0 { ver.micro - 1 } else { 0 },
                )),
                true,
            ),
            ScmpVersionTest::new(
                "VerNew",
                ScmpVersion::from((ver.major + 1, ver.minor + 1, ver.micro + 1)),
                false,
            ),
            ScmpVersionTest::new(
                "VerMajor+1",
                ScmpVersion::from((ver.major + 1, 0, 0)),
                false,
            ),
            ScmpVersionTest::new(
                "VerMinor+1",
                ScmpVersion::from((ver.major, ver.minor + 1, 0)),
                false,
            ),
            ScmpVersionTest::new(
                "VerMicro+1",
                ScmpVersion::from((ver.major, ver.minor, ver.micro + 1)),
                false,
            ),
        ];

        for test in tests {
            assert_eq!(
                ensure_supported_version(test.msg, test.ver).is_ok(),
                test.is_ok
            );
        }
    }
}
