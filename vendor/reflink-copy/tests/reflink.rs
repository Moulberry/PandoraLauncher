use std::fs::{self, File};
use std::io;
use std::path::Path;
use tempfile::tempdir;

use reflink_copy::{reflink, reflink_or_copy};

#[test]
fn reflink_file_does_not_exist() {
    let from = Path::new("test/nonexistent-bogus-path");
    let to = Path::new("test/other-bogus-path");

    reflink(from, to).unwrap_err();
    assert!(!from.exists());
    assert!(!to.exists());
}

#[test]
fn reflink_src_does_not_exist() {
    let tmpdir = tempdir().unwrap();
    let from = Path::new("test/nonexistent-bogus-path");
    let to = tmpdir.path().join("out.txt");

    fs::write(&to, b"hello").unwrap();
    assert!(reflink(from, &to).is_err());

    assert!(!from.exists());
    assert_eq!(fs::read(&to).unwrap(), b"hello");
}

#[test]
fn reflink_dest_is_dir() {
    let dir = tempdir().unwrap();
    let src_file_path = dir.path().join("src.txt");
    let _src_file = File::create(&src_file_path).unwrap();
    let err = reflink(&src_file_path, dir.path()).unwrap_err();
    println!("{:?}", err);
    if cfg!(windows) {
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    } else {
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    }
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos"
)))]
#[test]
fn reflink_src_is_dir() {
    let dir = tempdir().unwrap();
    let dest_file_path = dir.path().join("dest.txt");

    let err = reflink(dir.path(), dest_file_path).unwrap_err();
    println!("{:?}", err);
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn reflink_existing_dest_dir_results_in_error() {
    let dir = tempdir().unwrap();
    let src_file_path = dir.path().join("src");
    let dest_file_path = dir.path().join("dest");

    fs::create_dir(&src_file_path).unwrap();
    fs::create_dir(&dest_file_path).unwrap();

    let err = reflink(&src_file_path, &dest_file_path).unwrap_err();
    println!("{:?}", err);
    if cfg!(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos"
    )) {
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    } else {
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }
}

#[test]
fn reflink_existing_dest_results_in_error() {
    let dir = tempdir().unwrap();
    let src_file_path = dir.path().join("src.txt");
    let dest_file_path = dir.path().join("dest.txt");

    let _src_file = File::create(&src_file_path).unwrap();
    let _dest_file = File::create(&dest_file_path).unwrap();

    let err = reflink(&src_file_path, &dest_file_path).unwrap_err();
    println!("{:?}", err);
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists)
}

#[test]
fn reflink_ok() {
    let dir = tempdir().unwrap();
    let src_file_path = dir.path().join("src.txt");
    let dest_file_path = dir.path().join("dest.txt");

    fs::write(&src_file_path, b"this is a test").unwrap();

    let err = reflink(&src_file_path, dest_file_path);
    println!("{:?}", err);
    // do not panic for now, CI envs are old and will probably error out
    // assert_eq!(fs::read(&dest_file_path).unwrap(), b"this is a test");
}

#[test]
fn reflink_ok_permissions() {
    let dir = tempdir().unwrap();
    let src_file_path = dir.path().join("src.txt");
    let dest_file_path = dir.path().join("dest.txt");

    fs::write(&src_file_path, b"this is a test").unwrap();

    let mut permissions = fs::metadata(&src_file_path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&src_file_path, permissions).unwrap();

    let res = reflink(&src_file_path, &dest_file_path);
    println!("{:?}", res);
    // do not panic for now, CI envs are old and will probably error out
    // assert_eq!(fs::read(&dest_file_path).unwrap(), b"this is a test");
    if res.is_ok() {
        let metadata = fs::metadata(dest_file_path).unwrap();
        assert!(metadata.permissions().readonly());
    }
}

#[test]
fn reflink_or_copy_ok() {
    let tmpdir = tempdir().unwrap();
    let input = tmpdir.path().join("in.txt");
    let out = tmpdir.path().join("out.txt");

    fs::write(&input, b"hello").unwrap();

    reflink_or_copy(&input, &out).unwrap();

    assert_eq!(fs::read(&out).unwrap(), b"hello");

    assert_eq!(
        input.metadata().unwrap().permissions(),
        out.metadata().unwrap().permissions()
    );
}
