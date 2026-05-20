use std::io;
use std::process;
use std::process::ExitStatus;

use crate::Command;

pub fn runas_impl(cmd: &Command) -> io::Result<ExitStatus> {
    match which::which("sudo") {
        Ok(_) => {
            let mut c = process::Command::new("sudo");
            if cmd.force_prompt {
                c.arg("-k");
            }
            c.arg("--").arg(&cmd.command).args(&cmd.args[..]).status()
        }
        Err(_) => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Command `sudo` not found",
        )),
    }
}
