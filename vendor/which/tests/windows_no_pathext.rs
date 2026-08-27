//! A tiny test scaffold to keep the `set_env` call in this test isolated to its own process.
#![allow(clippy::disallowed_methods)]

/// Ensure that if PATHEXT is not populated, and the query is missing an extension, that an
/// appropriate NonFatalError is emitted.
#[test]
#[cfg(all(windows, feature = "real-sys"))]
fn windows_no_pathext_nonfatal_error() {
    use std::ffi::OsString;
    use which::{NonFatalError, WhichConfig};
    // This runs on Windows only and so is guaranteed to be safe. Still this shouldn't be allowed to
    // interfere with the other tests, so it is isolated to its own process.
    #[allow(unused_unsafe)] // Required for Rust 1.70.
    unsafe {
        std::env::set_var("PATHEXT", "");
    }

    let this_executable = std::env::current_exe().unwrap();
    let new_name = this_executable.parent().unwrap().join("test_executable");
    std::fs::copy(&this_executable, &new_name).unwrap();
    let mut nonfatal_errors = Vec::new();
    WhichConfig::new()
        .nonfatal_error_handler(|e| {
            nonfatal_errors.push(e);
        })
        .binary_name(OsString::from("test_executable"))
        .custom_path_list(this_executable.parent().unwrap().as_os_str().to_os_string())
        .first_result()
        .unwrap();
    std::fs::remove_file(new_name).unwrap();
    assert_eq!(nonfatal_errors.len(), 1);
    assert!(matches!(
        nonfatal_errors[0],
        NonFatalError::PathExtNotPopulated
    ));
}
