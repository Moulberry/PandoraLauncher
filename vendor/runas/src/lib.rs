//! This library implements basic support for running a command in an elevated context.
//!
//! In particular this runs a command through "sudo" or other platform equivalents.
//!
//! ## Basic Usage
//!
//! The library provides a single struct called `Command` which largely follows the
//! API of `std::process::Command`.  However it does not support capturing output or
//! gives any guarantees for the working directory or environment.  This is because
//! the platform APIs do not have support for that either in some cases.
//!
//! In particular the working directory is always the system32 folder on windows and
//! the environment variables are always the ones of the initial system session on
//! OS X if the GUI mode is used.
//!
//! ```rust,no_run
//! use runas::Command;
//!
//! let status = Command::new("rm")
//!     .arg("/usr/local/my-app")
//!     .status()
//!     .unwrap();
//! ```
//!
//! ## Platform Support
//!
//! The following platforms are supported:
//!
//! * Windows: always GUI mode
//! * OS X: GUI and CLI mode
//! * Linux: CLI mode
use std::ffi::{OsStr, OsString};
use std::io;
use std::process::ExitStatus;

#[cfg(target_os = "macos")]
mod impl_darwin;
#[cfg(unix)]
mod impl_unix;
#[cfg(windows)]
mod impl_windows;

/// A process builder for elevated execution
pub struct Command {
    command: OsString,
    args: Vec<OsString>,
    force_prompt: bool,
    hide: bool,
    gui: bool,
}

/// The `Command` type acts as a process builder for spawning programs that run in
/// an elevated context.
///
/// Example:
///
/// ```rust,no_run
/// use runas::Command;
/// let status = Command::new("cmd").status();
/// ```
impl Command {
    /// Creates a new command type for a given program.
    ///
    /// The default configuration is to spawn without arguments, to be visible and
    /// to not be launched from a GUI context.
    pub fn new<S: AsRef<OsStr>>(program: S) -> Command {
        Command {
            command: program.as_ref().to_os_string(),
            args: vec![],
            hide: false,
            gui: false,
            force_prompt: true,
        }
    }

    /// Add an argument to pass to the program.
    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Command {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    /// Add multiple arguments to pass to the program.

    pub fn args<S: AsRef<OsStr>>(&mut self, args: &[S]) -> &mut Command {
        for arg in args {
            self.arg(arg);
        }
        self
    }

    /// Controls the visibility of the program on supported platforms.  The default is
    /// to launch the program visible.
    pub fn show(&mut self, val: bool) -> &mut Command {
        self.hide = !val;
        self
    }

    /// Controls the GUI context.  The default behavior is to assume that the program is
    /// launched from a command line (not using a GUI).  This primarily controls how the
    /// elevation prompt is rendered.  On some platforms like Windows the elevation prompt
    /// is always a GUI element.
    ///
    /// If the preferred mode is not available it falls back to the other automatically.
    pub fn gui(&mut self, val: bool) -> &mut Command {
        self.gui = val;
        self
    }

    /// Can disable the prompt forcing for supported platforms.  Mostly this allows sudo
    /// on unix platforms to not prompt for a password.
    pub fn force_prompt(&mut self, val: bool) -> &mut Command {
        self.force_prompt = val;
        self
    }

    /// Executes a command as a child process, waiting for it to finish and
    /// collecting its exit status.
    pub fn status(&mut self) -> io::Result<ExitStatus> {
        #[cfg(all(unix, target_os = "macos"))]
        use crate::impl_darwin::runas_impl;
        #[cfg(all(unix, not(target_os = "macos")))]
        use impl_unix::runas_impl;
        #[cfg(windows)]
        use impl_windows::runas_impl;
        runas_impl(self)
    }
}
