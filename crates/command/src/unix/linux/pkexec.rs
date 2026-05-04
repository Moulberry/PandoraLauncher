use std::{ffi::OsStr, io::Error};

use crate::{PandoraChild, PandoraCommand, spawner::SpawnContext};

pub fn spawn(mut cmd: PandoraCommand, context: &mut SpawnContext) -> std::io::Result<PandoraChild> {
    let Some(pkexec) = crate::path_cache::get_command_path(OsStr::new("pkexec")) else {
        return Err(Error::new(std::io::ErrorKind::NotFound, "cannot find 'pkexec'"));
    };

    let mut executable = std::mem::replace(&mut cmd.executable, pkexec.as_os_str().to_os_string().into());

    // Replace with absolute path since pkexec won't inherit PATH
    if !executable.0.as_encoded_bytes().contains(&b'/') {
        let Some(path) = crate::path_cache::get_command_path(&executable.0) else {
            return Err(Error::new(std::io::ErrorKind::NotFound, format!("cannot find '{}'", executable.0.to_string_lossy())));
        };
        executable = path.as_os_str().to_os_string().into();
    }

    cmd.args.insert(0, "--disable-internal-agent".into());
    cmd.args.insert(1, "--keep-cwd".into());
    cmd.args.insert(2, executable);
    crate::unix::unix_spawn::spawn(cmd, context)
}
