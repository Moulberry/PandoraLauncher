use std::{borrow::Cow, collections::BTreeMap, ffi::{OsStr, OsString}, io::{Error, ErrorKind, PipeReader, PipeWriter}, path::{Path, PathBuf}, sync::Arc};

#[cfg(target_os = "macos")]
use crate::unix::unix_helpers::RawStringVec;
use crate::{process::PandoraProcess, spawner::SpawnType};

#[derive(Debug)]
pub struct PandoraCommand {
    pub(crate) executable: PandoraArg,
    pub(crate) args: Vec<PandoraArg>,
    pub(crate) inherit_env: Option<fn(&OsStr) -> bool>,
    pub(crate) env: BTreeMap<PandoraArg, PandoraArg>,
    pub(crate) current_dir: Option<PathBuf>,
    pub(crate) stdin: PandoraStdioWriteMode,
    pub(crate) stdout: PandoraStdioReadMode,
    pub(crate) stderr: PandoraStdioReadMode,
    #[cfg(windows)]
    pub(crate) force_feedback: bool,
    #[cfg(unix)]
    pub(crate) pass_fds: Vec<std::os::fd::OwnedFd>,
    #[cfg(target_os = "macos")]
    pub(crate) sandbox_profile: Option<std::ffi::CString>,
    #[cfg(target_os = "macos")]
    pub(crate) sandbox_params: Option<RawStringVec>,
}

impl PandoraCommand {
    pub fn new(executable: impl Into<PandoraArg>) -> Self {
        let executable = executable.into();
        assert!(!executable.0.is_empty());
        Self {
            executable,
            args: Vec::new(),
            inherit_env: None,
            env: BTreeMap::default(),
            current_dir: None,
            stdin: Default::default(),
            stdout: Default::default(),
            stderr: Default::default(),
            #[cfg(windows)]
            force_feedback: false,
            #[cfg(unix)]
            pass_fds: Default::default(),
            #[cfg(target_os = "macos")]
            sandbox_profile: None,
            #[cfg(target_os = "macos")]
            sandbox_params: None,
        }
    }

    pub fn arg(&mut self, arg: impl Into<PandoraArg>) {
        self.args.push(arg.into());
    }

    pub fn env(&mut self, k: impl Into<PandoraArg>, v: impl Into<PandoraArg>) {
        self.env.insert(k.into(), v.into());
    }

    pub fn current_dir(&mut self, current_dir: &Path) {
        self.current_dir = Some(current_dir.to_path_buf());
    }

    pub fn stdin(&mut self, stdin: PandoraStdioWriteMode) {
        self.stdin = stdin;
    }

    pub fn stdout(&mut self, stdout: PandoraStdioReadMode) {
        self.stdout = stdout;
    }

    pub fn stderr(&mut self, stderr: PandoraStdioReadMode) {
        self.stderr = stderr;
    }

    #[cfg(windows)]
    pub fn force_feedback(&mut self, force_feedback: bool) {
        self.force_feedback = force_feedback;
    }

    pub async fn spawn(self) -> std::io::Result<PandoraChild> {
        crate::spawner::spawn(self, SpawnType::Normal)
            .await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "spawning thread has shutdown"))
            .flatten()
    }

    pub async fn spawn_elevated(self) -> std::io::Result<PandoraProcess> {
        crate::spawner::spawn(self, SpawnType::Elevated)
            .await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "spawning thread has shutdown"))
            .flatten()
            .map(|child| child.process)
    }

    pub async fn spawn_sandboxed(self, sandbox: PandoraSandbox) -> std::io::Result<PandoraChild> {
        crate::spawner::spawn(self, SpawnType::Sandboxed(sandbox))
            .await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "spawning thread has shutdown"))
            .flatten()
    }

    pub(crate) fn resolve_executable_path(&self) -> std::io::Result<PathBuf> {
        let path = Path::new(&self.executable.0);
        let path = if path.components().count() > 1 {
            let Ok(path) = path.canonicalize() else {
                return Err(Error::new(ErrorKind::NotFound, "executable file doesn't exist"));
            };
            path
        } else if let Some(path) = crate::path_cache::get_command_path(&self.executable.0) {
            path.to_path_buf()
        } else {
            return Err(Error::new(ErrorKind::NotFound, "unable to resolve executable"));
        };

        debug_assert!(path.is_absolute());

        #[cfg(windows)]
        {
            // Try to remove the \\?\ verbatim path prefix since it can break some applications
            let encoded_bytes = path.as_os_str().as_encoded_bytes();
            if let Some(rest) = encoded_bytes.strip_prefix(b"\\\\?\\") {
                return Ok(PathBuf::from(unsafe { OsStr::from_encoded_bytes_unchecked(&rest) }));
            }
        }

        Ok(path)
    }

    pub(crate) fn take_final_env(&mut self) -> BTreeMap<PandoraArg, PandoraArg> {
        if let Some(inherit_env) = self.inherit_env {
            for (k, v) in std::env::vars_os() {
                let k: PandoraArg = k.into();
                if self.env.contains_key(&k) {
                    continue;
                }
                if !(inherit_env)(&k.0) {
                    continue;
                }
                self.env.insert(k, v.into());
            }
        } else {
            for (k, v) in std::env::vars_os() {
                let k: PandoraArg = k.into();
                if self.env.contains_key(&k) {
                    continue;
                }
                self.env.insert(k, v.into());
            }
        }
        std::mem::take(&mut self.env)
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

impl From<String> for PandoraArg {
    fn from(value: String) -> Self {
        PandoraArg(Cow::Owned(value.into()))
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
    pub is_jvm: bool,

    pub grant_network_access: bool,

    #[cfg(target_os = "linux")]
    pub sandbox_dir: Arc<Path>,
    #[cfg(windows)]
    pub name: Arc<OsStr>,
    #[cfg(windows)]
    pub description: Arc<OsStr>,
    #[cfg(windows)]
    pub self_elevate_for_acl_arg: Option<PandoraArg>,
    #[cfg(windows)]
    pub grant_winsta_writeattributes: bool,
}

#[derive(Debug)]
pub struct PandoraChild {
    pub process: PandoraProcess,
    pub stdin: Option<PipeWriter>,
    pub stdout: Option<PipeReader>,
    pub stderr: Option<PipeReader>,
}
