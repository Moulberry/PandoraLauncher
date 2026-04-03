use crate::unix::unix_helpers::{cvt, cvt_r};

pub struct PandoraProcess {
    pub(crate) pid: libc::pid_t,
}

impl PandoraProcess {
    pub fn stop(&mut self) -> std::io::Result<()> {
        unsafe { cvt(libc::kill(self.pid, libc::SIGTERM))? };
        Ok(())
    }

    pub fn kill(self) -> std::io::Result<()> {
        unsafe { cvt(libc::kill(self.pid, libc::SIGKILL))? };
        Ok(())
    }

    pub fn wait(self) -> std::io::Result<Option<i32>> {
        let mut status = 0 as libc::c_int;
        cvt_r(|| unsafe { libc::waitpid(self.pid, &mut status, 0) })?;
        if libc::WIFEXITED(status) {
            return Ok(Some(libc::WEXITSTATUS(status)));
        } else {
            return Ok(None);
        }
    }
}
