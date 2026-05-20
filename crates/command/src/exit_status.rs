use std::fmt::{Debug, Display};

#[cfg(windows)]
#[derive(Clone, Copy, Debug)]
pub struct PandoraExitStatus(pub(crate) u32);

#[cfg(windows)]
impl Display for PandoraExitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 & 0x80000000 != 0 {
            f.write_fmt(format_args!("exitcode=0x{:#x}", self.0))
        } else {
            f.write_fmt(format_args!("exitcode={}", self.0))
        }
    }
}

#[cfg(unix)]
#[derive(Clone, Copy)]
pub struct PandoraExitStatus(pub(crate) libc::c_int);

#[cfg(unix)]
impl Display for PandoraExitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if libc::WIFEXITED(self.0) {
            f.write_fmt(format_args!("exitcode={}", libc::WEXITSTATUS(self.0)))
        } else if libc::WIFSIGNALED(self.0) {
            f.write_fmt(format_args!("signal={}", libc::WTERMSIG(self.0)))
        } else {
            f.write_fmt(format_args!("unknownwait=0x{:#x}", self.0))
        }
    }
}

#[cfg(unix)]
impl Debug for PandoraExitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("PandoraExitStatus");
        debug.field("raw", &self.0);
        if libc::WIFEXITED(self.0) {
            debug.field("exitcode", &libc::WEXITSTATUS(self.0));
        }
        if libc::WIFSIGNALED(self.0) {
            debug.field("signal", &libc::WTERMSIG(self.0));
        }
        debug.finish()
    }
}
