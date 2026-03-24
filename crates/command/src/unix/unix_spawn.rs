use std::{ffi::CString, io::{Error, ErrorKind}, os::{fd::{AsRawFd, IntoRawFd, OwnedFd, RawFd}, unix::ffi::OsStringExt}, path::Path};

use libc::c_char;

use crate::{PandoraArg, PandoraChild, PandoraCommand, PandoraStdioReadMode, PandoraStdioWriteMode, process::PandoraProcess, unix::unix_helpers::{cvt, cvt_r, environ}};

pub fn spawn(mut command: PandoraCommand) -> std::io::Result<PandoraChild> {
    if let Some(inherit_env) = command.inherit_env {
        for (k, v) in std::env::vars_os() {
            let k: PandoraArg = k.into();
            if command.env.contains_key(&k) {
                continue;
            }
            if !(inherit_env)(&k.0) {
                continue;
            }
            command.env.insert(k, v.into());
        }
    } else {
        for (k, v) in std::env::vars_os() {
            let k: PandoraArg = k.into();
            if command.env.contains_key(&k) {
                continue;
            }
            command.env.insert(k, v.into());
        }
    }

    let mut env: Vec<*const c_char> = Vec::with_capacity(command.env.len() + 1);
    for (k, v) in command.env {
        let mut k = k.0.into_owned();
        k.reserve_exact(v.0.len() + 2);
        k.push("=");
        k.push(&v.0);

        if let Ok(item) = CString::new(k.into_vec()) {
            env.push(item.into_raw());
        } else {
            return Err(Error::new(ErrorKind::InvalidData, "environment variable contained null byte"))
        }
    }
    env.push(std::ptr::null_mut());

    let resolved = if command.executable.0.as_encoded_bytes().contains(&b'/') {
        let path = Path::new(&command.executable.0);
        let Ok(path) = path.canonicalize() else {
            return Err(Error::new(ErrorKind::NotFound, "executable file doesn't exist"));
        };
        path
    } else if let Some(path) = crate::path_cache::get_command_path(&command.executable.0) {
        path.to_path_buf()
    } else {
        return Err(Error::new(ErrorKind::NotFound, "unable to resolve executable"));
    };

    debug_assert!(resolved.is_absolute());

    let Ok(program) = CString::new(resolved.clone().into_os_string().into_vec()) else {
        return Err(Error::new(ErrorKind::InvalidData, "program contained null byte"))
    };

    let pass_fds = std::mem::take(&mut command.pass_fds);

    let mut stdin_write = None;
    let mut stdout_read = None;
    let mut stderr_read = None;

    let mut stdin_read = None;
    let mut stdout_write = None;
    let mut stderr_write = None;

    let mut null_fd = None;

    match command.stdin {
        PandoraStdioWriteMode::Null => {
            if null_fd.is_none() {
                let dev_null = unsafe { cvt(libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY))? };
                null_fd = Some(dev_null);
            }
            stdin_read = null_fd.clone();
        },
        PandoraStdioWriteMode::Inherit => {},
        PandoraStdioWriteMode::Pipe => {
            let (read, write) = std::io::pipe()?;
            stdin_write = Some(write);
            stdin_read = Some(read.into_raw_fd());
        }
    }
    match command.stdout {
        PandoraStdioReadMode::Pipe => {
            let (read, write) = std::io::pipe()?;
            stdout_write = Some(write.into_raw_fd());
            stdout_read = Some(read);
        },
        PandoraStdioReadMode::Null => {
            if null_fd.is_none() {
                let dev_null = unsafe { cvt(libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY))? };
                null_fd = Some(dev_null);
            }
            stdout_write = null_fd.clone();
        },
        PandoraStdioReadMode::Inherit => {},
    }
    match command.stderr {
        PandoraStdioReadMode::Pipe => {
            let (read, write) = std::io::pipe()?;
            stderr_write = Some(write.into_raw_fd());
            stderr_read = Some(read);
        },
        PandoraStdioReadMode::Null => {
            if null_fd.is_none() {
                let dev_null = unsafe { cvt(libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY))? };
                null_fd = Some(dev_null);
            }
            stderr_write = null_fd.clone();
        },
        PandoraStdioReadMode::Inherit => {},
    }

    let mut argv: Vec<*const c_char> = Vec::with_capacity(command.args.len() + 1);
    argv.push(program.as_ptr()); // arg0 is program name
    for arg in command.args {
        if let Ok(item) = CString::new(arg.0.into_owned().into_vec()) {
            argv.push(item.into_raw());
        } else {
            return Err(Error::new(ErrorKind::InvalidData, "arg contained null byte"))
        }
    }
    argv.push(std::ptr::null_mut());

    let pid = unsafe { cvt(libc::fork())? };

    if pid == 0 {
        _ = exec(
            env.into_raw_parts().0,
            stdin_read,
            stdout_write,
            stderr_write,
            program.as_ptr(),
            argv.as_ptr(),
            &pass_fds,
        );
        unsafe { libc::_exit(1) }
    }

    // Close unneeded handles
    if let Some(fd) = stdin_read {
        unsafe { libc::close(fd) };
    }
    if let Some(fd) = stdout_write {
        unsafe { libc::close(fd) };
    }
    if let Some(fd) = stderr_write {
        unsafe { libc::close(fd) };
    }
    drop(pass_fds);

    // Deallocate
    for ptr in env {
        if !ptr.is_null() {
            drop(unsafe { CString::from_raw(ptr.cast_mut()) });
        }
    }
    for ptr in argv {
        if !ptr.is_null() {
            drop(unsafe { CString::from_raw(ptr.cast_mut()) });
        }
    }
    std::mem::forget(program); // program is also part of argv, avoid double free

    Ok(PandoraChild {
        process: PandoraProcess {
            pid,
        },
        stdin: stdin_write,
        stdout: stdout_read,
        stderr: stderr_read
    })
}

fn exec(
    env: *const *const c_char,
    stdin: Option<RawFd>,
    stdout: Option<RawFd>,
    stderr: Option<RawFd>,
    program: *const c_char,
    argv: *const *const c_char,
    pass_fds: &[OwnedFd],
) -> std::io::Result<()> {
    unsafe {
        *environ() = env;

        if let Some(mut fd) = stdin {
            if fd > 0 && fd <= libc::STDERR_FILENO {
                fd = cvt_r(|| libc::dup(fd))?;
            }
            cvt_r(|| libc::dup2(fd, libc::STDIN_FILENO))?;
        }
        if let Some(mut fd) = stdout {
            if fd > 0 && fd <= libc::STDERR_FILENO {
                fd = cvt_r(|| libc::dup(fd))?;
            }
            cvt_r(|| libc::dup2(fd, libc::STDOUT_FILENO))?;
        }
        if let Some(mut fd) = stderr {
            if fd > 0 && fd <= libc::STDERR_FILENO {
                fd = cvt_r(|| libc::dup(fd))?;
            }
            cvt_r(|| libc::dup2(fd, libc::STDERR_FILENO))?;
        }

        for fd in pass_fds {
            cvt_r(|| libc::ioctl(fd.as_raw_fd(), libc::FIONCLEX))?;
        }

        cvt_r(|| libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0))?;

        cvt(libc::execvp(program, argv))?;
        Ok(())
    }
}
