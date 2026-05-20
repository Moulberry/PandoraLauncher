#[test]
fn archive_oob_errors_gracefully() {
    let bad_archive = [
        0x50u8, 0x4b, 0x6, 0x7, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x50, 0x4b, 0x5,
        0x6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let Err(err) = rc_zip_sync::ReadZip::read_zip(&bad_archive.as_slice()) else {
        // NOTE(cosmic): `.unwrap_err()` requires `ArchiveHandle` to impl `Debug`, but it doesn't
        panic!("expected error, but parsed a valid archive");
    };
    assert_eq!(
        err.to_string(),
        "io: archive tried reading beyond zip archive end. 65536 goes beyond 42"
    );
}
