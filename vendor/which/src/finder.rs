use crate::checker::is_valid;
use crate::helper::has_executable_extension;
use crate::sys::Sys;
use crate::sys::SysReadDirEntry;
use crate::{error::*, NonFatalErrorHandler};
#[cfg(feature = "regex")]
use regex::Regex;
#[cfg(feature = "regex")]
use std::borrow::Borrow;
use std::borrow::Cow;
use std::ffi::OsStr;
#[cfg(feature = "regex")]
use std::io;
use std::path::{Component, Path, PathBuf};
use std::vec;

trait PathExt {
    fn has_separator(&self) -> bool;

    fn to_absolute<P>(self, cwd: P) -> PathBuf
    where
        P: AsRef<Path>;
}

impl PathExt for PathBuf {
    fn has_separator(&self) -> bool {
        self.components().count() > 1
    }

    fn to_absolute<P>(self, cwd: P) -> PathBuf
    where
        P: AsRef<Path>,
    {
        if self.is_absolute() {
            self
        } else {
            let mut new_path = PathBuf::from(cwd.as_ref());
            new_path.extend(
                self.components()
                    .skip_while(|c| matches!(c, Component::CurDir)),
            );
            new_path
        }
    }
}

pub struct Finder<TSys: Sys> {
    sys: TSys,
}

impl<TSys: Sys> Finder<TSys> {
    pub fn new(sys: TSys) -> Self {
        Finder { sys }
    }

    pub fn find<'a, T, U, V, F: NonFatalErrorHandler + 'a>(
        self,
        binary_name: T,
        paths: Option<U>,
        cwd: Option<V>,
        nonfatal_error_handler: F,
    ) -> Result<impl Iterator<Item = PathBuf> + 'a>
    where
        TSys: 'a,
        T: AsRef<OsStr>,
        U: AsRef<OsStr>,
        V: AsRef<Path> + 'a,
    {
        let path = PathBuf::from(&binary_name);

        #[cfg(feature = "tracing")]
        tracing::debug!(
            "query binary_name = {:?}, paths = {:?}, cwd = {:?}",
            binary_name.as_ref().to_string_lossy(),
            paths.as_ref().map(|p| p.as_ref().to_string_lossy()),
            cwd.as_ref().map(|p| p.as_ref().display())
        );

        let cwd = cwd.as_ref().map(AsRef::as_ref);
        let ret = match cwd {
            _ if path.is_absolute() => {
                WhichFindIterator::new_path(path, self.sys, nonfatal_error_handler)
            }
            Some(cwd) if path.has_separator() => {
                WhichFindIterator::new_cwd(path, cwd, self.sys, nonfatal_error_handler)
            }
            cwd => {
                #[cfg(feature = "tracing")]
                tracing::trace!(
                    "{} has no path seperators, so only paths in PATH environment variable will be searched.",
                    path.display()
                );
                // Search binary in PATHs(defined in environment variable).
                let paths = paths.ok_or(Error::CannotGetCurrentDirAndPathListEmpty)?;
                let paths = self.sys.env_split_paths(paths.as_ref());
                if paths.is_empty() {
                    return Err(Error::CannotGetCurrentDirAndPathListEmpty);
                }
                WhichFindIterator::new_paths(path, paths, cwd, self.sys, nonfatal_error_handler)
            }
        };
        #[cfg(feature = "tracing")]
        let ret = ret.inspect(|p| {
            tracing::debug!("found path {}", p.display());
        });
        Ok(ret)
    }

    #[cfg(feature = "regex")]
    pub fn find_re<T, F: NonFatalErrorHandler>(
        self,
        binary_regex: impl std::borrow::Borrow<Regex>,
        paths: Option<T>,
        nonfatal_error_handler: F,
    ) -> Result<impl Iterator<Item = PathBuf>>
    where
        T: AsRef<OsStr>,
    {
        WhichFindRegexIter::new(self.sys, paths, binary_regex, nonfatal_error_handler)
    }
}

struct WhichFindIterator<TSys: Sys, F: NonFatalErrorHandler> {
    sys: TSys,
    paths: PathsIter<vec::IntoIter<PathBuf>>,
    nonfatal_error_handler: F,
}

impl<TSys: Sys, F: NonFatalErrorHandler> WhichFindIterator<TSys, F> {
    pub fn new_path(binary_name: PathBuf, sys: TSys, mut nonfatal_error_handler: F) -> Self {
        let path_extensions = if sys.is_windows() {
            sys.env_windows_path_ext()
        } else {
            Cow::Borrowed(Default::default())
        };
        if sys.is_windows() && path_extensions.is_empty() && binary_name.extension().is_none() {
            nonfatal_error_handler.handle(NonFatalError::PathExtNotPopulated);
        }
        Self {
            sys,
            paths: PathsIter {
                paths: vec![binary_name].into_iter(),
                current_path_with_index: None,
                path_extensions,
            },
            nonfatal_error_handler,
        }
    }

    pub fn new_cwd(binary_name: PathBuf, cwd: &Path, sys: TSys, nonfatal_error_handler: F) -> Self {
        Self::new_path(binary_name.to_absolute(cwd), sys, nonfatal_error_handler)
    }

    pub fn new_paths(
        binary_name: PathBuf,
        paths: Vec<PathBuf>,
        cwd: Option<&Path>,
        sys: TSys,
        mut nonfatal_error_handler: F,
    ) -> Self {
        let path_extensions = if sys.is_windows() {
            sys.env_windows_path_ext()
        } else {
            Cow::Borrowed(Default::default())
        };
        if sys.is_windows() && path_extensions.is_empty() && binary_name.extension().is_none() {
            nonfatal_error_handler.handle(NonFatalError::PathExtNotPopulated);
        }

        // PowerShell Get-Command omits empty entries in PATH string, unix `which` command does not.
        // Emulate OS specific behavior here.
        let is_windows = sys.is_windows();
        let paths = paths
            .into_iter()
            .filter(|p| !is_windows || !p.as_os_str().is_empty())
            .map(|p| {
                let p = tilde_expansion(&sys, &p);
                match cwd {
                    Some(cwd) if is_ordinary_relative_path(&p, is_windows) => {
                        p.into_owned().to_absolute(cwd)
                    }
                    _ => p.into_owned(),
                }
            })
            .map(|p| p.join(&binary_name))
            .collect::<Vec<_>>();

        Self {
            sys,
            paths: PathsIter {
                paths: paths.into_iter(),
                current_path_with_index: None,
                path_extensions,
            },
            nonfatal_error_handler,
        }
    }
}

fn is_ordinary_relative_path(path: &Path, is_windows: bool) -> bool {
    if path.has_root() || path.starts_with("~") {
        return false;
    }

    if is_windows {
        // `Sys::is_windows` can model Windows even when this crate was compiled for another
        // platform, so do not rely only on the host platform's `Path::components` parsing here.
        // Drive-relative (`C:foo`) and root-relative (`\foo`) paths retain their native semantics.
        let path = path.as_os_str().to_string_lossy();
        let bytes = path.as_bytes();
        if path.starts_with('\\') || bytes.get(1) == Some(&b':') {
            return false;
        }
    }

    true
}

impl<TSys: Sys, F: NonFatalErrorHandler> Iterator for WhichFindIterator<TSys, F> {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        for path in &mut self.paths {
            if is_valid(&self.sys, &path, &mut self.nonfatal_error_handler) {
                return Some(correct_casing(
                    &self.sys,
                    path,
                    &mut self.nonfatal_error_handler,
                ));
            }
        }
        None
    }
}

struct PathsIter<P>
where
    P: Iterator<Item = PathBuf>,
{
    paths: P,
    current_path_with_index: Option<(PathBuf, usize)>,
    path_extensions: Cow<'static, [String]>,
}

impl<P> Iterator for PathsIter<P>
where
    P: Iterator<Item = PathBuf>,
{
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        if self.path_extensions.is_empty() {
            self.paths.next()
        } else if let Some((p, index)) = self.current_path_with_index.take() {
            let next_index = index + 1;
            if next_index < self.path_extensions.len() {
                self.current_path_with_index = Some((p.clone(), next_index));
            }
            // Append the extension.
            let mut p = p.into_os_string();
            p.push(&self.path_extensions[index]);
            let ret = PathBuf::from(p);
            #[cfg(feature = "tracing")]
            tracing::trace!("possible extension: {}", ret.display());
            Some(ret)
        } else {
            let p = self.paths.next()?;
            if has_executable_extension(&p, &self.path_extensions) {
                #[cfg(feature = "tracing")]
                tracing::trace!(
                    "{} already has an executable extension, not modifying it further",
                    p.display()
                );
            } else {
                #[cfg(feature = "tracing")]
                tracing::trace!(
                    "{} has no extension, using PATHEXT environment variable to infer one",
                    p.display()
                );
                // Appended paths with windows executable extensions.
                // e.g. path `c:/windows/bin[.ext]` will expand to:
                // [c:/windows/bin.ext]
                // c:/windows/bin[.ext].COM
                // c:/windows/bin[.ext].EXE
                // c:/windows/bin[.ext].CMD
                // ...
                self.current_path_with_index = Some((p.clone(), 0));
            }
            Some(p)
        }
    }
}

fn tilde_expansion<TSys: Sys>(sys: TSys, p: &Path) -> Cow<'_, Path> {
    let mut component_iter = p.components();
    if let Some(Component::Normal(o)) = component_iter.next() {
        if o == "~" {
            let new_path = sys.home_dir();
            if let Some(mut new_path) = new_path {
                new_path.extend(component_iter);
                #[cfg(feature = "tracing")]
                tracing::trace!(
                    "found tilde, substituting in user's home directory to get {}",
                    new_path.display()
                );
                return Cow::Owned(new_path);
            } else {
                #[cfg(feature = "tracing")]
                tracing::trace!("found tilde in path, but user's home directory couldn't be found");
            }
        }
    }
    Cow::Borrowed(p)
}

fn correct_casing<TSys: Sys, F: NonFatalErrorHandler>(
    sys: TSys,
    mut p: PathBuf,
    nonfatal_error_handler: &mut F,
) -> PathBuf {
    if sys.is_windows() {
        if let (Some(parent), Some(file_name)) = (p.parent(), p.file_name()) {
            if let Ok(iter) = sys.read_dir(parent) {
                for e in iter {
                    match e {
                        Ok(e) => {
                            if e.file_name().eq_ignore_ascii_case(file_name) {
                                p.pop();
                                p.push(e.file_name());
                                break;
                            }
                        }
                        Err(e) => {
                            nonfatal_error_handler.handle(NonFatalError::Io(e));
                        }
                    }
                }
            }
        }
    }
    p
}

#[cfg(feature = "regex")]
struct WhichFindRegexIter<TSys: Sys, B: Borrow<Regex>, F: NonFatalErrorHandler> {
    sys: TSys,
    re: B,
    paths: vec::IntoIter<PathBuf>,
    nonfatal_error_handler: F,
    current_read_dir_iter: Option<Box<dyn Iterator<Item = io::Result<TSys::ReadDirEntry>>>>,
}

#[cfg(feature = "regex")]
impl<TSys: Sys, B: Borrow<Regex>, F: NonFatalErrorHandler> WhichFindRegexIter<TSys, B, F> {
    pub fn new<T: AsRef<OsStr>>(
        sys: TSys,
        paths: Option<T>,
        re: B,
        nonfatal_error_handler: F,
    ) -> Result<Self> {
        let p = paths.ok_or(Error::CannotGetCurrentDirAndPathListEmpty)?;
        let paths = sys.env_split_paths(p.as_ref());
        Ok(WhichFindRegexIter {
            sys,
            re,
            paths: paths.into_iter(),
            nonfatal_error_handler,
            current_read_dir_iter: None,
        })
    }
}

#[cfg(feature = "regex")]
impl<TSys: Sys, B: Borrow<Regex>, F: NonFatalErrorHandler> Iterator
    for WhichFindRegexIter<TSys, B, F>
{
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(iter) = &mut self.current_read_dir_iter {
                match iter.next() {
                    Some(Ok(path)) => {
                        if let Some(unicode_file_name) = path.file_name().to_str() {
                            if self.re.borrow().is_match(unicode_file_name) {
                                return Some(path.path());
                            } else {
                                #[cfg(feature = "tracing")]
                                tracing::debug!("regex filtered out {}", unicode_file_name);
                            }
                        } else {
                            #[cfg(feature = "tracing")]
                            tracing::debug!(
                                "regex unable to evaluate filename as it's not valid unicode. Lossy filename conversion: {}",
                                path.file_name().to_string_lossy()
                            );
                        }
                    }
                    Some(Err(e)) => {
                        self.nonfatal_error_handler.handle(NonFatalError::Io(e));
                    }
                    None => {
                        self.current_read_dir_iter = None;
                    }
                }
            } else {
                let path = self.paths.next()?;
                match self.sys.read_dir(&path) {
                    Ok(new_read_dir_iter) => {
                        self.current_read_dir_iter = Some(new_read_dir_iter);
                    }
                    Err(e) => {
                        self.nonfatal_error_handler.handle(NonFatalError::Io(e));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_ordinary_relative_path;
    use std::path::Path;

    #[test]
    fn ordinary_relative_paths_are_resolved_against_cwd() {
        for path in ["tools", "./tools", "../tools"] {
            assert!(is_ordinary_relative_path(Path::new(path), false));
            assert!(is_ordinary_relative_path(Path::new(path), true));
        }
    }

    #[test]
    fn tilde_and_windows_special_paths_keep_native_semantics() {
        assert!(!is_ordinary_relative_path(Path::new("~/tools"), false));

        for path in [r"C:tools", r"C:\tools", r"\tools", r"\\server\share\tools"] {
            assert!(!is_ordinary_relative_path(Path::new(path), true));
        }
    }
}
