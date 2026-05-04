use std::{
    fs::{self, File},
    io::{self, Read},
    path::Path,
};

use rc_zip::{
    error::{Error, FormatError},
    parse::EntryKind,
};
use rc_zip_sync::{ArchiveHandle, ReadZip};

/// The executable side of a self-extracting zip file
///
/// A program starts from the front and will still behave the same if you tack anything on the end.
/// The same is true for trailing zip files, but from the opposite direction. This means that you
/// can create a self-extracting zip file by throwing a zip file on the end of a program that
/// extracts itself as a zip file. It is both a valid executable, and a valid zip file at the same
/// time
///
/// 1. build the executable
///   - `$ cargo build --release --example=self_extracting`
/// 2. combine it with a zip file to make a `self_extracting.zip` file
///   - `$ cat target/release/examples/self_extracting path/to/some.zip > self_extracting.zip`
/// 3. make it executable as well
///   - `$ chmod +x self_extracting.zip`
/// 4. now the zip file can extract itself!
///   - `$ zipinfo self_extracting.zip # still detects the zip file`
///   - `$ ./self_extracting.zip`
fn main() -> Result<(), Error> {
    let zip_path = std::env::args_os().next().unwrap();
    let zip_file = File::open(&zip_path)?;
    let archive = zip_file.read_zip().inspect_err(|err| {
        if let Error::Format(FormatError::DirectoryEndSignatureNotFound) = err {
            eprintln!("hint: did you forget to append a zip file?");
        }
    })?;

    extract(&archive)?;

    Ok(())
}

fn extract(archive: &ArchiveHandle<'_, File>) -> Result<(), Error> {
    for entry in archive.entries() {
        println!("extracting {}", entry.name);
        let Some(entry_name) = entry.sanitized_name() else {
            eprintln!("ignoring potentially malicious entry");
            continue;
        };

        let path = Path::new(entry_name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        match entry.kind() {
            EntryKind::Directory => fs::create_dir_all(path)?,
            EntryKind::File => {
                let mut entry_writer = File::create(path)?;
                let mut entry_reader = entry.reader();
                io::copy(&mut entry_reader, &mut entry_writer)?;
            }
            EntryKind::Symlink => {
                #[cfg(windows)]
                {
                    // creating a symlink on windows is a privileged action, so instead we create a
                    // regular file
                    let mut entry_writer = File::create(path)?;
                    let mut entry_reader = entry.reader();
                    io::copy(&mut entry_reader, &mut entry_writer)?;
                }
                #[cfg(unix)]
                {
                    use std::ffi::OsString;
                    use std::os::unix::ffi::OsStringExt;

                    if let Ok(metadata) = fs::symlink_metadata(&path) {
                        if metadata.is_file() {
                            fs::remove_file(&path)?;
                        }
                    }

                    let mut src = Vec::new();
                    entry.reader().read_to_end(&mut src)?;
                    let src = OsString::from_vec(src);

                    std::os::unix::fs::symlink(&src, &path)?;
                }
                #[cfg(not(any(windows, unix)))]
                {
                    eprintln!("ignoring symlink on unsupported platform");
                }
            }
        }
    }

    Ok(())
}
