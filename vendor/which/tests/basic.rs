#![allow(clippy::disallowed_methods)]

extern crate which;

#[cfg(feature = "real-sys")]
mod real_sys {
    #[cfg(all(unix, feature = "regex"))]
    use regex::Regex;
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::{env, vec};
    use tempfile::TempDir;

    #[derive(Debug)]
    struct TestFixture {
        /// Temp directory.
        pub tempdir: TempDir,
        /// $PATH
        pub paths: OsString,
        /// Binaries created in $PATH
        pub bins: Vec<PathBuf>,
    }

    const SUBDIRS: &[&str] = &["a", "b", "c"];
    const BIN_NAME: &str = "bin";

    #[allow(clippy::unnecessary_cast)]
    #[cfg(unix)]
    fn mk_bin(dir: &Path, path: &str, extension: &str) -> io::Result<PathBuf> {
        use std::os::unix::fs::OpenOptionsExt;
        let bin = dir.join(path).with_extension(extension);

        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let mode = (0o666 | libc::S_IXUSR) as u32;
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .mode(mode)
            .open(&bin)
            .and_then(|_f| bin.canonicalize())
    }

    fn touch(dir: &Path, path: &str, extension: &str) -> io::Result<PathBuf> {
        let b = dir.join(path).with_extension(extension);
        fs::File::create(&b).and_then(|_f| b.canonicalize())
    }

    #[cfg(windows)]
    fn mk_bin(dir: &Path, path: &str, extension: &str) -> io::Result<PathBuf> {
        touch(dir, path, extension)
    }

    impl TestFixture {
        // tmp/a/bin
        // tmp/a/bin.exe
        // tmp/a/bin.cmd
        // tmp/b/bin
        // tmp/b/bin.exe
        // tmp/b/bin.cmd
        // tmp/c/bin
        // tmp/c/bin.exe
        // tmp/c/bin.cmd
        pub fn new() -> TestFixture {
            let tempdir = tempfile::tempdir().unwrap();
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            let mut paths = vec![];
            let mut bins = vec![];
            for d in SUBDIRS.iter() {
                let p = tempdir.path().join(d);
                builder.create(&p).unwrap();
                bins.push(mk_bin(&p, BIN_NAME, "").unwrap());
                bins.push(mk_bin(&p, BIN_NAME, "exe").unwrap());
                bins.push(mk_bin(&p, BIN_NAME, "cmd").unwrap());
                paths.push(p);
            }
            let p = tempdir.path().join("win-bin");
            builder.create(&p).unwrap();
            bins.push(mk_bin(&p, "win-bin", "exe").unwrap());
            paths.push(p);
            TestFixture {
                tempdir,
                paths: env::join_paths(paths).unwrap(),
                bins,
            }
        }

        #[cfg(unix)]
        pub fn new_with_tilde_path() -> TestFixture {
            let tempdir = tempfile::tempdir().unwrap();
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            let mut paths = vec![];
            let mut bins = vec![];
            for d in SUBDIRS.iter() {
                let p = PathBuf::from("~").join(d);
                let p_bin = tempdir.path().join(d);
                builder.create(&p_bin).unwrap();
                bins.push(mk_bin(&p_bin, BIN_NAME, "").unwrap());
                bins.push(mk_bin(&p_bin, BIN_NAME, "exe").unwrap());
                bins.push(mk_bin(&p_bin, BIN_NAME, "cmd").unwrap());
                paths.push(p);
            }
            let p = tempdir.path().join("win-bin");
            builder.create(&p).unwrap();
            bins.push(mk_bin(&p, "win-bin", "exe").unwrap());
            paths.push(p);
            TestFixture {
                tempdir,
                paths: env::join_paths(paths).unwrap(),
                bins,
            }
        }

        #[allow(dead_code)]
        pub fn touch(&self, path: &str, extension: &str) -> io::Result<PathBuf> {
            touch(self.tempdir.path(), path, extension)
        }

        pub fn mk_bin(&self, path: &str, extension: &str) -> io::Result<PathBuf> {
            mk_bin(self.tempdir.path(), path, extension)
        }
    }

    fn _which<T: AsRef<OsStr>>(f: &TestFixture, path: T) -> which::Result<which::CanonicalPath> {
        which::CanonicalPath::new_in(path, Some(f.paths.clone()), f.tempdir.path())
    }
    fn _which_uncanonicalized<T: AsRef<OsStr>>(f: &TestFixture, path: T) -> which::Result<PathBuf> {
        which::which_in(path, Some(f.paths.clone()), f.tempdir.path())
    }

    fn _which_all<'a, T: AsRef<OsStr> + 'a>(
        f: &'a TestFixture,
        path: T,
    ) -> which::Result<impl Iterator<Item = which::Result<which::CanonicalPath>> + 'a> {
        which::CanonicalPath::all_in(path, Some(f.paths.clone()), f.tempdir.path())
    }

    #[test]
    #[cfg(unix)]
    fn it_works() {
        use std::process::Command;
        let result = which::Path::new("rustc");
        assert!(result.is_ok());

        let which_result = Command::new("which").arg("rustc").output();

        assert_eq!(
            String::from(result.unwrap().to_str().unwrap()),
            String::from_utf8(which_result.unwrap().stdout)
                .unwrap()
                .trim()
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_which() {
        let f = TestFixture::new();
        assert_eq!(_which(&f, BIN_NAME).unwrap(), f.bins[0])
    }

    #[test]
    #[cfg(windows)]
    fn test_which() {
        let f = TestFixture::new();
        assert_eq!(_which(&f, BIN_NAME).unwrap(), f.bins[1])
    }

    #[test]
    #[cfg(unix)]
    fn test_which_tilde() {
        let old_home = env::var_os("HOME");
        let f = TestFixture::new_with_tilde_path();
        env::set_var("HOME", f.tempdir.path().as_os_str());
        assert_eq!(_which(&f, BIN_NAME).unwrap(), f.bins[0]);
        if let Some(old_home) = old_home {
            env::set_var("HOME", old_home);
        } else {
            env::remove_var("HOME");
        }
    }

    // Windows test_which_tilde intentionally omitted because
    // we don't want to pollute the home directory.
    // It's non-trivial to adjust which directory Windows thinks
    // is the home directory. At this time, tilde expansion has
    // no Windows specific behavior. It works as normal on Windows.

    #[test]
    #[cfg(all(unix, feature = "regex"))]
    fn test_which_re_in_with_matches() {
        let f = TestFixture::new();
        f.mk_bin("a/bin_0", "").unwrap();
        f.mk_bin("b/bin_1", "").unwrap();
        let re = Regex::new(r"bin_\d").unwrap();

        let result: Vec<PathBuf> = which::which_re_in(re, Some(f.paths)).unwrap().collect();

        let temp = f.tempdir;

        assert_eq!(
            result,
            vec![temp.path().join("a/bin_0"), temp.path().join("b/bin_1")]
        )
    }

    #[test]
    #[cfg(all(unix, feature = "regex"))]
    fn test_which_re_in_without_matches() {
        let f = TestFixture::new();
        let re = Regex::new(r"bi[^n]").unwrap();

        let result: Vec<PathBuf> = which::which_re_in(re, Some(f.paths)).unwrap().collect();

        assert_eq!(result, Vec::<PathBuf>::new())
    }

    #[test]
    #[cfg(all(unix, feature = "regex"))]
    fn test_which_re_accepts_owned_and_borrow() {
        which::which_re(Regex::new(r".").unwrap())
            .unwrap()
            .for_each(drop);
        which::which_re(&Regex::new(r".").unwrap())
            .unwrap()
            .for_each(drop);
        which::which_re_in(Regex::new(r".").unwrap(), Some("pth"))
            .unwrap()
            .for_each(drop);
        which::which_re_in(&Regex::new(r".").unwrap(), Some("pth"))
            .unwrap()
            .for_each(drop);
    }

    #[test]
    #[cfg(unix)]
    fn test_which_extension() {
        let f = TestFixture::new();
        let b = Path::new(&BIN_NAME).with_extension("");
        assert_eq!(_which(&f, b).unwrap(), f.bins[0])
    }

    #[test]
    #[cfg(windows)]
    fn test_which_extension() {
        let f = TestFixture::new();
        let b = Path::new(&BIN_NAME).with_extension("cmd");
        assert_eq!(_which(&f, b).unwrap(), f.bins[2])
    }

    #[test]
    #[cfg(windows)]
    fn test_which_no_extension() {
        let f = TestFixture::new();
        let b = Path::new("win-bin");
        let which_result = which::which_in(b, Some(&f.paths), ".").unwrap();
        // Make sure the extension is the correct case.
        assert_eq!(which_result.extension(), f.bins[9].extension());
        assert_eq!(fs::canonicalize(&which_result).unwrap(), f.bins[9])
    }

    #[test]
    fn test_which_not_found() {
        let f = TestFixture::new();
        assert!(_which(&f, "a").is_err());
    }

    #[test]
    fn test_which_second() {
        let f = TestFixture::new();
        let b = f.mk_bin("b/another", env::consts::EXE_EXTENSION).unwrap();
        assert_eq!(_which(&f, "another").unwrap(), b);
    }

    #[test]
    fn test_relative_path_entries_use_provided_cwd() {
        let f = TestFixture::new();
        #[cfg(unix)]
        let expected = &f.bins[0];
        #[cfg(windows)]
        let expected = &f.bins[1];
        let nested_cwd = f.tempdir.path().join("nested");
        fs::create_dir(&nested_cwd).unwrap();

        for (cwd, entry) in [
            (f.tempdir.path(), "a"),
            (f.tempdir.path(), "./a"),
            (nested_cwd.as_path(), "../a"),
        ] {
            let paths = env::join_paths([entry]).unwrap();
            let result = which::which_in(BIN_NAME, Some(paths), cwd).unwrap();

            assert_eq!(
                result.canonicalize().unwrap(),
                *expected,
                "PATH entry: {entry}"
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn test_relative_path_entry_when_cwd_contains_path_separator() {
        let f = TestFixture::new();
        let cwd = f.tempdir.path().join("command:cwd");
        let tools = cwd.join("tools");
        fs::create_dir_all(&tools).unwrap();
        let expected = mk_bin(&tools, BIN_NAME, "").unwrap();
        let paths = env::join_paths(["./tools"]).unwrap();

        let result = which::which_in(BIN_NAME, Some(paths), &cwd).unwrap();

        assert_eq!(result.canonicalize().unwrap(), expected);
    }

    #[test]
    #[cfg(unix)]
    fn test_empty_path_entry_uses_provided_cwd() {
        let f = TestFixture::new();
        let cwd = f.tempdir.path().join("a");
        let paths = env::join_paths([PathBuf::new()]).unwrap();

        let result = which::which_in(BIN_NAME, Some(paths), &cwd).unwrap();

        assert_eq!(result.canonicalize().unwrap(), f.bins[0]);
    }

    #[test]
    fn test_which_all() {
        let f = TestFixture::new();
        let actual = _which_all(&f, BIN_NAME)
            .unwrap()
            .map(|c| c.unwrap())
            .collect::<Vec<_>>();
        let mut expected = f
            .bins
            .iter()
            .map(|p| p.canonicalize().unwrap())
            .collect::<Vec<_>>();
        #[cfg(windows)]
        {
            expected.retain(|p| p.file_stem().unwrap() == BIN_NAME);
            expected
                .retain(|p| p.extension().map(|ext| ext == "exe" || ext == "cmd") == Some(true));
        }
        #[cfg(not(windows))]
        {
            expected.retain(|p| p.file_name().unwrap() == BIN_NAME);
        }
        assert_eq!(actual, expected);
    }

    #[test]
    #[cfg(unix)]
    fn test_which_absolute() {
        let f = TestFixture::new();
        assert_eq!(
            _which(&f, &f.bins[3]).unwrap(),
            f.bins[3].canonicalize().unwrap()
        );
    }

    #[test]
    #[cfg(windows)]
    fn test_which_absolute() {
        let f = TestFixture::new();
        assert_eq!(
            _which(&f, &f.bins[4]).unwrap(),
            f.bins[4].canonicalize().unwrap()
        );
    }

    #[test]
    #[cfg(windows)]
    fn test_which_global_absolute_without_path_list() {
        let f = TestFixture::new();

        let result = which::which_in_global(&f.bins[4], Option::<&OsStr>::None)
            .unwrap()
            .next()
            .unwrap();

        assert_eq!(result, f.bins[4]);
    }

    #[test]
    #[cfg(windows)]
    fn test_which_absolute_path_case() {
        // Test that an absolute path with an uppercase extension
        // is accepted.
        let f = TestFixture::new();
        let p = &f.bins[4];
        assert_eq!(_which(&f, p).unwrap(), f.bins[4].canonicalize().unwrap());
    }

    #[test]
    #[cfg(unix)]
    fn test_which_absolute_extension() {
        let f = TestFixture::new();
        // Don't append EXE_EXTENSION here.
        let b = f.bins[3].parent().unwrap().join(BIN_NAME);
        assert_eq!(_which(&f, b).unwrap(), f.bins[3].canonicalize().unwrap());
    }

    #[test]
    #[cfg(windows)]
    fn test_which_absolute_extension() {
        let f = TestFixture::new();
        // Don't append EXE_EXTENSION here.
        let b = f.bins[4].parent().unwrap().join(BIN_NAME);
        assert_eq!(_which(&f, b).unwrap(), f.bins[4].canonicalize().unwrap());
    }

    #[test]
    #[cfg(unix)]
    fn test_which_relative() {
        let f = TestFixture::new();
        assert_eq!(
            _which(&f, "b/bin").unwrap(),
            f.bins[3].canonicalize().unwrap()
        );
    }

    #[test]
    #[cfg(windows)]
    fn test_which_relative() {
        let f = TestFixture::new();
        assert_eq!(
            _which(&f, "b/bin").unwrap(),
            f.bins[4].canonicalize().unwrap()
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_which_relative_extension() {
        // test_which_relative tests a relative path without an extension,
        // so test a relative path with an extension here.
        let f = TestFixture::new();
        let b = Path::new("b/bin").with_extension(env::consts::EXE_EXTENSION);
        assert_eq!(_which(&f, b).unwrap(), f.bins[3].canonicalize().unwrap());
    }

    #[test]
    #[cfg(windows)]
    fn test_which_relative_extension() {
        // test_which_relative tests a relative path without an extension,
        // so test a relative path with an extension here.
        let f = TestFixture::new();
        let b = Path::new("b/bin").with_extension("cmd");
        assert_eq!(_which(&f, b).unwrap(), f.bins[5].canonicalize().unwrap());
    }

    #[test]
    #[cfg(windows)]
    fn test_which_relative_extension_case() {
        // Test that a relative path with an uppercase extension
        // is accepted.
        let f = TestFixture::new();
        let b = Path::new("b/bin").with_extension("EXE");
        assert_eq!(_which(&f, b).unwrap(), f.bins[4].canonicalize().unwrap());
    }

    #[test]
    #[cfg(unix)]
    fn test_which_relative_leading_dot() {
        let f = TestFixture::new();
        assert_eq!(
            _which(&f, "./b/bin").unwrap(),
            f.bins[3].canonicalize().unwrap()
        );
    }

    #[test]
    #[cfg(windows)]
    fn test_which_relative_leading_dot() {
        let f = TestFixture::new();
        assert_eq!(
            _which(&f, "./b/bin").unwrap(),
            f.bins[4].canonicalize().unwrap()
        );
    }

    #[test]
    #[cfg(all(unix, not(target_os = "macos")))]
    fn test_which_relative_leading_dot_uncanonicalized() {
        let f = TestFixture::new();

        let actual = _which_uncanonicalized(&f, "./b/bin").unwrap();
        assert_eq!(actual, f.bins[3].canonicalize().unwrap());

        assert!(
            !actual.display().to_string().contains("/./"),
            "'{}' should not contain a CurDir component",
            actual.display()
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_which_non_executable() {
        // Shouldn't return non-executable files.
        let f = TestFixture::new();
        f.touch("b/another", "").unwrap();
        assert!(_which(&f, "another").is_err());
    }

    #[test]
    #[cfg(unix)]
    fn test_which_absolute_non_executable() {
        // Shouldn't return non-executable files, even if given an absolute path.
        let f = TestFixture::new();
        let b = f.touch("b/another", "").unwrap();
        assert!(_which(&f, b).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn test_which_relative_non_executable() {
        // Shouldn't return non-executable files.
        let f = TestFixture::new();
        f.touch("b/another", "").unwrap();
        assert!(_which(&f, "b/another").is_err());
    }

    #[test]
    fn test_failure() {
        let f = TestFixture::new();

        let run = || -> which::Result<PathBuf> {
            let p = _which(&f, "./b/bin")?;
            Ok(p.into_path_buf())
        };

        let _ = run();
    }

    #[test]
    #[cfg(windows)]
    fn test_path_empty_entry() {
        let f = TestFixture::new();
        std::env::set_current_dir(f.tempdir.path().join("a")).unwrap();
        let found = which::which_in_global(&BIN_NAME, Some("C:\\foo\\bar/;;C:\\bar\\foo"))
            .unwrap()
            .collect::<Vec<_>>();
        assert!(found.is_empty(), "results contained {found:?}")
    }

    #[test]
    #[cfg(unix)]
    fn test_path_empty_entry() {
        let f = TestFixture::new();
        std::env::set_current_dir(f.tempdir.path().join("a")).unwrap();
        let found = which::which_in_global(&BIN_NAME, Some("/foo/bar/::/bar/foo"))
            .unwrap()
            .collect::<Vec<_>>();
        assert!(found.len() == 1, "results contained {found:?}")
    }

    #[test]
    #[cfg(windows)]
    fn windows_no_extension_but_executable() {
        let this_executable = std::env::current_exe().unwrap();
        let new_name = this_executable.parent().unwrap().join("test_executable");
        std::fs::copy(&this_executable, &new_name).unwrap();
        let found_executable = which::which_in_global(
            new_name.file_name().unwrap(),
            Some(this_executable.parent().unwrap()),
        )
        .unwrap()
        .next()
        .unwrap();
        assert_eq!(found_executable, new_name);
        std::fs::remove_file(new_name).unwrap();
    }
}

mod in_memory {
    use std::collections::BTreeMap;
    use std::collections::HashMap;
    use std::collections::HashSet;
    use std::ffi::OsStr;
    use std::ffi::OsString;
    use std::io;
    use std::io::Error;
    use std::io::ErrorKind;
    use std::path::Component;
    use std::path::Path;
    use std::path::PathBuf;

    struct Metadata {
        is_symlink: bool,
        is_file: bool,
    }

    impl which::sys::SysMetadata for Metadata {
        fn is_symlink(&self) -> bool {
            self.is_symlink
        }

        fn is_file(&self) -> bool {
            self.is_file
        }
    }

    struct ReadDirEntry {
        path: PathBuf,
    }

    impl which::sys::SysReadDirEntry for ReadDirEntry {
        fn file_name(&self) -> OsString {
            self.path.file_name().unwrap().to_os_string()
        }

        fn path(&self) -> PathBuf {
            self.path.clone()
        }
    }

    #[derive(Debug, Clone)]
    enum DirectoryEntry {
        Directory(Directory),
        File(File),
        Symlink(Symlink),
    }

    impl DirectoryEntry {
        pub fn unwrap_directory(&self) -> &Directory {
            match self {
                DirectoryEntry::Directory(d) => d,
                _ => unreachable!(),
            }
        }

        pub fn unwrap_directory_mut(&mut self) -> &mut Directory {
            match self {
                DirectoryEntry::Directory(d) => d,
                _ => unreachable!(),
            }
        }

        pub fn as_metadata(&self) -> Metadata {
            Metadata {
                is_symlink: matches!(self, DirectoryEntry::Symlink(_)),
                is_file: matches!(self, DirectoryEntry::File(_)),
            }
        }
    }

    #[derive(Debug, Default, Clone)]
    struct Directory {
        entries: BTreeMap<OsString, DirectoryEntry>,
    }

    #[derive(Debug, Clone)]
    struct File {
        is_valid_executable: bool,
    }

    #[derive(Debug, Clone)]
    struct Symlink {
        to: PathBuf,
    }

    #[derive(Debug, Clone)]
    struct InMemorySys {
        is_windows: bool,
        cwd: PathBuf,
        home_dir: Option<PathBuf>,
        env_vars: HashMap<OsString, OsString>,
        root_dir: DirectoryEntry,
    }

    impl InMemorySys {
        pub fn new() -> Self {
            Self {
                is_windows: false,
                cwd: PathBuf::from("/project"),
                home_dir: None,
                env_vars: Default::default(),
                root_dir: DirectoryEntry::Directory(Directory::default()),
            }
        }

        pub fn set_home_dir(&mut self, path: impl AsRef<Path>) {
            self.home_dir = Some(path.as_ref().to_path_buf());
        }

        pub fn set_env_var(&mut self, name: impl AsRef<OsStr>, value: impl AsRef<OsStr>) {
            self.env_vars
                .insert(name.as_ref().to_os_string(), value.as_ref().to_os_string());
        }

        pub fn create_symlink(&mut self, from: impl AsRef<Path>, to: impl AsRef<Path>) {
            self.insert_dir_entry(
                from,
                DirectoryEntry::Symlink(Symlink {
                    to: to.as_ref().to_path_buf(),
                }),
            );
        }

        pub fn write_executable(&mut self, path: impl AsRef<Path>) {
            self.insert_dir_entry(
                path,
                DirectoryEntry::File(File {
                    is_valid_executable: true,
                }),
            );
        }

        pub fn write_non_executable(&mut self, path: impl AsRef<Path>) {
            self.insert_dir_entry(
                path,
                DirectoryEntry::File(File {
                    is_valid_executable: false,
                }),
            );
        }

        fn insert_dir_entry(&mut self, path: impl AsRef<Path>, entry: DirectoryEntry) {
            // not super efficient, but good enough for testing
            let dir_path = path.as_ref().parent().unwrap();
            self.create_directory(dir_path);
            let dir_entry = self.with_entry_mut(dir_path).unwrap();
            let dir = dir_entry.unwrap_directory_mut();
            let name = path.as_ref().file_name().unwrap();
            dir.entries.insert(name.to_os_string(), entry);
        }

        pub fn create_directory(&mut self, path: impl AsRef<Path>) {
            // lazy implementation
            let mut ancestors = path
                .as_ref()
                .ancestors()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .peekable();
            while let Some(ancestor) = ancestors.next() {
                let entry = self.with_entry_mut(ancestor).unwrap();
                match entry {
                    DirectoryEntry::Directory(d) => {
                        if let Some(next_name) = ancestors.peek().and_then(|a| a.file_name()) {
                            if !d.entries.contains_key(next_name) {
                                d.entries.insert(
                                    next_name.to_os_string(),
                                    DirectoryEntry::Directory(Default::default()),
                                );
                            }
                        }
                    }
                    _ => unreachable!("Not a directory."),
                }
            }
        }

        fn with_entry_mut(&mut self, path: impl AsRef<Path>) -> Option<&mut DirectoryEntry> {
            let mut current_entry = &mut self.root_dir;
            let mut components = path.as_ref().components().peekable();

            while let Some(component) = components.next() {
                match component {
                    Component::RootDir => {
                        let is_last = components.peek().is_none();
                        if is_last {
                            return Some(current_entry);
                        }
                    }
                    Component::Normal(os_str) => {
                        let is_last = components.peek().is_none();
                        let entry = current_entry
                            .unwrap_directory_mut()
                            .entries
                            .get_mut(os_str)?;

                        if is_last {
                            return Some(entry);
                        }

                        current_entry = entry;
                    }
                    Component::CurDir | Component::ParentDir | Component::Prefix(_) => todo!(),
                }
            }
            None
        }

        fn get_entry(&self, path: &Path) -> Option<&DirectoryEntry> {
            let mut current_entry = &self.root_dir;
            let mut components = path.components().peekable();

            while let Some(component) = components.next() {
                match component {
                    Component::RootDir => continue,
                    Component::Normal(os_str) => {
                        let entry = current_entry.unwrap_directory().entries.get(os_str)?;
                        if components.peek().is_none() {
                            return Some(entry);
                        } else {
                            current_entry = entry;
                        }
                    }
                    Component::CurDir | Component::ParentDir | Component::Prefix(_) => todo!(),
                }
            }

            unreachable!()
        }

        fn get_entry_follow_symlink(&self, path: &Path) -> Option<&DirectoryEntry> {
            let mut current_path = path.to_path_buf();
            let mut seen = HashSet::new();

            loop {
                let entry = self.get_entry(&current_path)?;
                if let DirectoryEntry::Symlink(symlink) = &entry {
                    if !seen.insert(current_path.clone()) {
                        return None; // symlink loop
                    }
                    current_path = symlink.to.clone();
                    continue;
                }
                return Some(entry);
            }
        }
    }

    impl which::sys::Sys for InMemorySys {
        type ReadDirEntry = ReadDirEntry;

        type Metadata = Metadata;

        fn is_windows(&self) -> bool {
            self.is_windows
        }

        fn current_dir(&self) -> io::Result<PathBuf> {
            Ok(self.cwd.clone())
        }

        fn home_dir(&self) -> Option<PathBuf> {
            self.home_dir.clone()
        }

        fn env_split_paths(&self, paths: &OsStr) -> Vec<PathBuf> {
            paths
                .to_string_lossy()
                .split(if self.is_windows { ";" } else { ":" })
                .map(PathBuf::from)
                .collect()
        }

        fn env_path(&self) -> Option<OsString> {
            self.env_vars.get(OsStr::new("PATH")).cloned()
        }

        fn env_path_ext(&self) -> Option<OsString> {
            self.env_vars.get(OsStr::new("PATHEXT")).cloned()
        }

        fn metadata(&self, path: &Path) -> io::Result<Self::Metadata> {
            let entry = self
                .get_entry_follow_symlink(path)
                .ok_or_else(|| Error::new(ErrorKind::NotFound, "metadata: entry not found"))?;

            Ok(entry.as_metadata())
        }

        fn symlink_metadata(&self, path: &Path) -> io::Result<Self::Metadata> {
            let entry = self
                .get_entry(path)
                .ok_or_else(|| Error::new(ErrorKind::NotFound, "metadata: entry not found"))?;

            Ok(entry.as_metadata())
        }

        fn read_dir(
            &self,
            path: &Path,
        ) -> io::Result<Box<dyn Iterator<Item = io::Result<Self::ReadDirEntry>>>> {
            let entry = self
                .get_entry_follow_symlink(path)
                .ok_or_else(|| Error::new(ErrorKind::NotFound, "metadata: entry not found"))?;

            match &entry {
                DirectoryEntry::Directory(dir) => {
                    let entries = dir
                        .entries
                        .keys()
                        .map(|name| {
                            Ok(ReadDirEntry {
                                path: path.join(name),
                            })
                        })
                        .collect::<Vec<_>>();
                    Ok(Box::new(entries.into_iter()))
                }
                // should use ErrorKind::NotADirectory once upgrading rust version
                _ => Err(Error::new(ErrorKind::Other, "Not a directory")),
            }
        }

        fn is_valid_executable(&self, path: &Path) -> io::Result<bool> {
            let entry = self.get_entry_follow_symlink(path).ok_or_else(|| {
                Error::new(ErrorKind::NotFound, "is_valid_executable: entry not found")
            })?;

            match &entry {
                DirectoryEntry::File(file) => Ok(file.is_valid_executable),
                _ => Ok(false),
            }
        }
    }

    #[test]
    fn basic() {
        let mut sys = InMemorySys::new();
        sys.set_env_var("PATH", "/sub/dir1/:/sub/dir2/");
        sys.write_non_executable("/sub/dir1/exec1");
        sys.write_executable("/sub/dir2/exec1"); // will get this one
        sys.write_executable("/sub/dir2/exec2");
        let config = which::WhichConfig::new_with_sys(sys).binary_name(OsString::from("exec1"));
        let result = config.first_result().unwrap();
        assert_eq!(result, PathBuf::from("/sub/dir2/exec1"));
    }

    #[test]
    #[cfg(not(windows))]
    fn absolute_path_works_without_cwd_or_path_list() {
        let mut sys = InMemorySys::new();
        sys.write_executable("/sub/dir/exec");
        let config = which::WhichConfig::new_with_sys(sys)
            .system_cwd(false)
            .binary_name(OsString::from("/sub/dir/exec"));

        let result = config.first_result().unwrap();

        assert_eq!(result, PathBuf::from("/sub/dir/exec"));
    }

    #[test]
    fn symlink() {
        let mut sys = InMemorySys::new();
        sys.set_env_var("PATH", "/sub/dir1/");
        sys.create_symlink("/sub/dir1/exec", "/sub/dir2/exec");
        sys.write_executable("/sub/dir2/exec");
        let config = which::WhichConfig::new_with_sys(sys).binary_name(OsString::from("exec"));
        let result = config.first_result().unwrap();
        assert_eq!(result, PathBuf::from("/sub/dir1/exec"));
    }

    #[test]
    fn tilde_path() {
        let mut sys = InMemorySys::new();
        sys.set_home_dir("/home/user/");
        sys.set_env_var("PATH", "/dir/:~/sub/");
        sys.write_executable("/home/user/sub/exec");
        let config = which::WhichConfig::new_with_sys(sys).binary_name(OsString::from("exec"));
        let result = config.first_result().unwrap();
        assert_eq!(result, PathBuf::from("/home/user/sub/exec"));
    }
}
