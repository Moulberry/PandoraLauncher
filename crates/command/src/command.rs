use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
    io::{Error, ErrorKind, PipeReader, PipeWriter},
    path::{Path, PathBuf},
    sync::Arc,
};

#[cfg(unix)]
use std::os::fd::OwnedFd;

use rustc_hash::FxHashMap;

use crate::process::PandoraProcess;

pub struct PandoraCommand {
    pub(crate) executable: PandoraArg,
    pub(crate) args: Vec<PandoraArg>,
    pub(crate) inherit_env: Option<fn(&OsStr) -> bool>,
    pub(crate) env: FxHashMap<PandoraArg, PandoraArg>,
    pub(crate) stdin: PandoraStdioWriteMode,
    pub(crate) stdout: PandoraStdioReadMode,
    pub(crate) stderr: PandoraStdioReadMode,
    #[cfg(unix)]
    pub(crate) pass_fds: Vec<OwnedFd>,
}

impl PandoraCommand {
    pub fn new(executable: impl Into<PandoraArg>) -> Self {
        Self {
            executable: executable.into(),
            args: Vec::new(),
            inherit_env: None,
            env: FxHashMap::default(),
            stdin: Default::default(),
            stdout: Default::default(),
            stderr: Default::default(),
            #[cfg(unix)]
            pass_fds: Default::default(),
        }
    }

    pub fn arg(&mut self, arg: impl Into<PandoraArg>) {
        self.args.push(arg.into());
    }

    pub fn spawn(self) -> std::io::Result<PandoraChild> {
        #[cfg(unix)]
        return crate::unix::unix_spawn::spawn(self);

        #[cfg(not(unix))]
        Err(Error::new(ErrorKind::Unsupported, "process spawning not supported on this platform"))
    }

    pub fn spawn_elevated(self) -> std::io::Result<PandoraProcess> {
        #[cfg(target_os = "linux")]
        return crate::unix::linux::pkexec::spawn(self);

        #[cfg(not(target_os = "linux"))]
        Err(Error::new(ErrorKind::Unsupported, "elevated spawning not supported on this platform"))
    }

    pub fn spawn_sandboxed(self, sandbox: PandoraSandbox) -> std::io::Result<PandoraChild> {
        #[cfg(target_os = "linux")]
        return crate::unix::linux::bwrap::spawn(self, sandbox);

        #[cfg(not(target_os = "linux"))]
        {
            let _ = sandbox;
            Err(Error::new(ErrorKind::Unsupported, "sandboxed spawning not supported on this platform"))
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PandoraStdioReadMode {
    Null,
    #[default]
    Inherit,
    Pipe,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PandoraStdioWriteMode {
    #[default]
    Null,
    Inherit,
    Pipe,
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct PandoraArg(pub(crate) Cow<'static, OsStr>);

impl From<&'static str> for PandoraArg {
    fn from(value: &'static str) -> Self {
        PandoraArg(Cow::Borrowed(OsStr::new(value)))
    }
}

impl From<&'static OsStr> for PandoraArg {
    fn from(value: &'static OsStr) -> Self {
        PandoraArg(Cow::Borrowed(value))
    }
}

impl From<OsString> for PandoraArg {
    fn from(value: OsString) -> Self {
        PandoraArg(Cow::Owned(value))
    }
}

impl From<PathBuf> for PandoraArg {
    fn from(value: PathBuf) -> Self {
        PandoraArg(Cow::Owned(value.into_os_string()))
    }
}

pub struct PandoraSandbox {
    pub allow_read: Vec<Arc<Path>>,
    pub allow_write: Vec<Arc<Path>>,
    pub sandbox_dir: Arc<Path>,
    pub is_jvm: bool,
}

pub struct PandoraChild {
    pub process: PandoraProcess,
    pub stdin: Option<PipeWriter>,
    pub stdout: Option<PipeReader>,
    pub stderr: Option<PipeReader>,
}
