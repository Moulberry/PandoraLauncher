use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::windows::ffi::OsStringExt;
use std::os::windows::fs::symlink_file;
use std::os::windows::io::AsRawHandle;
#[cfg(miri)]
use std::path::Path;
use std::path::PathBuf;
use std::slice;

#[cfg(not(miri))]
use tempfile::TempDir;

#[cfg(miri)]
struct TempDir {
    path: PathBuf,
}

#[cfg(miri)]
impl TempDir {
    fn path(&self) -> &Path {
        self.path.as_path()
    }
}

// https://docs.microsoft.com/en-us/windows/desktop/debug/system-error-codes
const ERROR_NOT_A_REPARSE_POINT: i32 = 0x1126;
const ERROR_ALREADY_EXISTS: i32 = 0xb7;

#[cfg(not(miri))]
fn create_tempdir() -> TempDir {
    tempfile::Builder::new()
        .prefix("junction-test-")
        .tempdir_in("target/debug")
        .unwrap()
}

#[cfg(miri)]
fn create_tempdir() -> TempDir {
    TempDir {
        path: PathBuf::from("target/debug/junction-test"),
    }
}

#[test]
fn create_dir_all_with_junctions() {
    let tmpdir = create_tempdir();
    let target = tmpdir.path().join("target");

    let junction = tmpdir.path().join("junction");
    let b = junction.join("a/b");

    fs::create_dir_all(&target).unwrap();

    super::create(&target, &junction).unwrap();
    fs::create_dir_all(&b).unwrap();
    // the junction itself is not a directory, but `is_dir()` on a Path
    // follows links
    assert!(junction.is_dir());
    assert!(b.exists());
}

#[test]
fn create_recursive_rmdir() {
    let tmpdir = create_tempdir();
    let d1 = tmpdir.path().join("d1"); // "d1"
    let dt = d1.join("t"); // "d1/t"
    let dtt = dt.join("t"); // "d1/t/t"
    let d2 = tmpdir.path().join("d2"); // "d2"
    let canary = d2.join("do_not_delete"); // "d2/do_not_delete"

    fs::create_dir_all(dtt).unwrap();
    fs::create_dir_all(&d2).unwrap();
    File::create(&canary).unwrap().write_all(b"foo").unwrap();

    super::create(d2, dt.join("d2")).unwrap(); // "d1/t/d2" -> "d2"

    let _ = symlink_file(&canary, d1.join("canary")); // d1/canary -> d2/do_not_delete
    fs::remove_dir_all(&d1).unwrap();

    assert!(!d1.is_dir());
    assert!(canary.exists());
}

#[test]
fn create_recursive_rmdir_of_symlink() {
    // test we do not recursively delete a symlink but only dirs.
    let tmpdir = create_tempdir();
    let link = tmpdir.path().join("link");
    let dir = tmpdir.path().join("dir");
    let canary = dir.join("do_not_delete");
    fs::create_dir_all(&dir).unwrap();
    File::create(&canary).unwrap().write_all(b"foo").unwrap();
    super::create(&dir, &link).unwrap();
    fs::remove_dir_all(&link).unwrap();

    assert!(!link.is_dir());
    assert!(canary.exists());
}

#[test]
fn create_directory_exist_before() {
    let tmpdir = create_tempdir();

    let target = tmpdir.path().join("target");
    let junction = tmpdir.path().join("junction");

    fs::create_dir_all(&junction).unwrap();

    match super::create(target, &junction) {
        Err(ref e) if e.raw_os_error() == Some(ERROR_ALREADY_EXISTS) => {}
        other => panic!("directory exists before creating: {:?}", other),
    }
}

#[test]
fn create_target_no_exist() {
    let tmpdir = create_tempdir();

    let target = tmpdir.path().join("target");
    let junction = tmpdir.path().join("junction");

    match super::create(target, junction) {
        Ok(()) => {}
        other => panic!("junction should point to non exist target path: {:?}", other),
    }
}

#[test]
fn delete_junctions() {
    let tmpdir = create_tempdir();

    let non_existence_dir = tmpdir.path().join("non_existence_dir");
    match super::delete(non_existence_dir) {
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {}
        e => panic!("target path does not exist or is not a directory: {:?}", e),
    }

    let dir_not_junction = tmpdir.path().join("dir_not_junction");
    fs::create_dir_all(&dir_not_junction).unwrap();
    match super::delete(dir_not_junction) {
        Err(ref e) if e.raw_os_error() == Some(ERROR_NOT_A_REPARSE_POINT) => {}
        e => panic!("target path is not a junction point: {:?}", e),
    }

    let file = tmpdir.path().join("foo-file");
    File::create(&file).unwrap().write_all(b"foo").unwrap();
    match super::delete(&file) {
        Err(ref e) if e.raw_os_error() == Some(ERROR_NOT_A_REPARSE_POINT) => {}
        e => panic!("target path is not a junction point: {:?}", e),
    }
}

#[test]
fn exists_verify() {
    let tmpdir = create_tempdir();

    // Check no such directory or file
    let no_such_dir = tmpdir.path().join("no_such_dir");
    assert!(!super::exists(no_such_dir).unwrap());

    // Target exists but not a junction
    let no_such_file = tmpdir.path().join("file");
    File::create(&no_such_file).unwrap().write_all(b"foo").unwrap();
    match super::exists(&no_such_file) {
        Err(ref e) if e.raw_os_error() == Some(ERROR_NOT_A_REPARSE_POINT) => {}
        other => panic!("target exists but not a junction: {:?}", other),
    }

    let target = tmpdir.path().join("target");
    let junction = tmpdir.path().join("junction");
    let file = target.join("file");
    let junction_file = junction.join("file");

    fs::create_dir_all(&target).unwrap();
    File::create(file).unwrap().write_all(b"foo").unwrap();

    assert!(
        !junction_file.exists(),
        "file should not be located until junction created"
    );
    assert!(!super::exists(&junction).unwrap(), "junction not created yet");

    super::create(&target, &junction).unwrap();
    assert!(super::exists(&junction).unwrap(), "junction should exist now");
    assert_eq!(&super::get_target(&junction).unwrap(), &target);
    assert!(junction_file.exists(), "file should be accessible via the junction");

    super::delete(&junction).unwrap();
    match super::exists(&junction) {
        Err(ref e) if e.raw_os_error() == Some(ERROR_NOT_A_REPARSE_POINT) => {}
        other => panic!("junction had been deleted: {:?}", other),
    }
    assert!(
        !junction_file.exists(),
        "file should not be located after junction deleted"
    );
    assert!(junction.exists(), "directory should not be deleted");
}

#[test]
fn get_target_user_dirs() {
    use std::env;
    if cfg!(feature = "unstable_admin") {
        // These special system directory junction are from
        // <https://en.wikipedia.org/wiki/NTFS_links#Built-in_uses>
        assert_eq!(
            super::get_target(r"C:\Users\Default User").unwrap().to_str(),
            Some(r"C:\Users\Default"),
        );
        assert_eq!(
            super::get_target(r"C:\Documents and Settings\").unwrap().to_str(),
            Some(r"C:\Users"),
        );
        let user_profile = env::var("USERPROFILE").unwrap();
        assert_eq!(
            super::get_target(format!("{user}\\Application Data", user = user_profile))
                .unwrap()
                .to_str(),
            Some(format!("{user}\\AppData\\Roaming", user = user_profile).as_str()),
        );
        assert_eq!(
            super::get_target(format!("{user}\\My Documents\\My Pictures", user = user_profile))
                .unwrap()
                .to_str(),
            Some(format!("{user}\\Pictures", user = user_profile).as_str()),
        );
    }

    let tmpdir = create_tempdir();

    let non_existence_dir = tmpdir.path().join("non_existence_dir");
    match super::get_target(non_existence_dir) {
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {}
        other => panic!("target path does not exist or is not a directory: {:?}", other),
    }

    let dir_not_junction = tmpdir.path().join("dir_not_junction");
    fs::create_dir_all(&dir_not_junction).unwrap();
    match super::get_target(dir_not_junction) {
        Err(ref e) if e.raw_os_error() == Some(ERROR_NOT_A_REPARSE_POINT) => {}
        other => panic!("target path is not a junction point: {:?}", other),
    }

    let file = tmpdir.path().join("foo-file");
    File::create(&file).unwrap().write_all(b"foo").unwrap();
    match super::get_target(file) {
        Err(ref e) if e.raw_os_error() == Some(ERROR_NOT_A_REPARSE_POINT) => {}
        other => panic!("target path is not a junction point: {:?}", other),
    }
}

#[test]
fn create_with_verbatim_prefix_paths() {
    // Regression test for https://github.com/tesuji/junction/issues/30
    let tmpdir = create_tempdir();

    let target = tmpdir.path().join("target");
    let link_parent = tmpdir.path().join("links");
    fs::create_dir_all(&target).unwrap();
    fs::create_dir_all(&link_parent).unwrap();

    // std::fs::canonicalize() returns paths with \\?\ verbatim prefix on Windows
    let target_canonical = fs::canonicalize(&target).unwrap();
    let junction = fs::canonicalize(&link_parent).unwrap().join("junction");

    super::create(&target_canonical, &junction).unwrap();
    assert!(super::exists(&junction).unwrap(), "junction should exist");
    // get_target returns path without verbatim prefix
    assert_eq!(super::get_target(&junction).unwrap(), target);
}

#[test]
fn create_populates_print_name() {
    // Regression test: the junction reparse point must have a non-empty PrintName
    // so that Windows Container layer snapshots correctly preserve the junction target.
    use super::internals::{c, cast, helpers, WCHAR_SIZE};

    let tmpdir = tempfile::tempdir().unwrap();
    let target = tmpdir.path().join("target");
    let junction = tmpdir.path().join("junction");
    fs::create_dir_all(&target).unwrap();

    super::create(&target, &junction).unwrap();

    // Read back the raw reparse data
    let mut data = cast::BytesAsReparseDataBuffer::new();
    {
        let file = helpers::open_reparse_point(&junction, false).unwrap();
        helpers::get_reparse_data_point(file.as_raw_handle(), data.as_mut_ptr()).unwrap();
    }
    let rdb = unsafe { data.assume_init() };

    assert_eq!(rdb.ReparseTag, c::IO_REPARSE_TAG_MOUNT_POINT);

    // Read PrintName
    let print_offset = (rdb.ReparseBuffer.PrintNameOffset / WCHAR_SIZE) as usize;
    let print_len = (rdb.ReparseBuffer.PrintNameLength / WCHAR_SIZE) as usize;
    let print_name = unsafe {
        let buf = rdb.ReparseBuffer.PathBuffer.as_ptr().add(print_offset);
        slice::from_raw_parts(buf, print_len)
    };

    // PrintName must not be empty
    assert!(print_len > 0, "PrintName must not be empty");

    // PrintName should match what get_target returns (the Win32 path without \??\ prefix)
    let print_path = PathBuf::from(OsString::from_wide(print_name));
    let target_path = super::get_target(&junction).unwrap();
    assert_eq!(print_path, target_path, "PrintName should match the target path");
}
