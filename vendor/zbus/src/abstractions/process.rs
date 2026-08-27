#[cfg(feature = "async-io")]
use async_process::{Child, unix::CommandExt};
#[cfg(unix)]
use std::process::Output;
use std::{ffi::OsStr, io::Error, process::Stdio};
#[cfg(all(feature = "tokio", not(feature = "async-io")))]
use tokio::process::Child;

use crate::address::transport::Unixexec;

/// A wrapper around the command API of the underlying async runtime.
///
/// Unlike the socket transports, `unixexec` isn't run-time selected: `async-process` is used
/// whenever it's compiled in (its pipes are `Async<_>`, which any runtime can drive), so with both
/// backends a tokio app connecting over `unixexec:` picks up async-io's reactor.
pub struct Command(
    #[cfg(feature = "async-io")] async_process::Command,
    #[cfg(all(feature = "tokio", not(feature = "async-io")))] tokio::process::Command,
);

impl Command {
    /// Constructs a new `Command` for launching the program at path `program`.
    pub fn new<S>(program: S) -> Self
    where
        S: AsRef<OsStr>,
    {
        #[cfg(feature = "async-io")]
        return Self(async_process::Command::new(program));

        #[cfg(all(feature = "tokio", not(feature = "async-io")))]
        return Self(tokio::process::Command::new(program));
    }

    /// Constructs a new `Command` from a `unixexec` address.
    pub fn for_unixexec(unixexec: &Unixexec) -> Self {
        let mut command = Self::new(unixexec.path());
        command.args(unixexec.args());

        if let Some(arg0) = unixexec.arg0() {
            command.arg0(arg0);
        }

        command
    }

    /// Sets executable argument.
    ///
    /// Set the first process argument, `argv[0]`, to something other than the
    /// default executable path.
    pub fn arg0<S>(&mut self, arg: S) -> &mut Self
    where
        S: AsRef<OsStr>,
    {
        self.0.arg0(arg);
        self
    }

    /// Adds multiple arguments to pass to the program.
    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.0.args(args);
        self
    }

    /// Executes the command as a child process, waiting for it to finish and
    /// collecting all of its output.
    #[cfg(unix)]
    pub async fn output(&mut self) -> Result<Output, Error> {
        self.0.output().await
    }

    /// Sets configuration for the child process's standard input (stdin) handle.
    pub fn stdin<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.0.stdin(cfg);
        self
    }

    /// Sets configuration for the child process's standard output (stdout) handle.
    pub fn stdout<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.0.stdout(cfg);
        self
    }

    /// Sets configuration for the child process's standard error (stderr) handle.
    pub fn stderr<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.0.stderr(cfg);
        self
    }

    /// Executes the command as a child process, returning a handle to it.
    pub fn spawn(&mut self) -> Result<Child, Error> {
        self.0.spawn()
    }
}

/// An asynchronous wrapper around running and getting command output
#[cfg(unix)]
pub async fn run<I, S>(program: S, args: I) -> Result<Output, Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(program).args(args).output().await
}
