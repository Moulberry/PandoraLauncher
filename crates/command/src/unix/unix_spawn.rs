use std::{ffi::CString, io::{Error, ErrorKind}, os::{fd::{AsRawFd, OwnedFd, RawFd}, unix::ffi::OsStringExt}};

use libc::c_char;

use crate::{PandoraChild, PandoraCommand, PandoraStdioReadMode, PandoraStdioWriteMode, process::PandoraProcess, spawner::SpawnContext, unix::unix_helpers::{RawStringVec, cvt, cvt_r, environ}};

pub fn spawn(mut command: PandoraCommand, context: &mut SpawnContext) -> std::io::Result<PandoraChild> {
    // Program
    let resolved = command.resolve_executable_path()?;
    let Ok(program) = CString::new(resolved.clone().into_os_string().into_vec()) else {
        return Err(Error::new(ErrorKind::InvalidData, "program contained null byte"))
    };

    // Arguments
    let mut argv = RawStringVec::with_capacity(command.args.len() + 1);
    argv.push_c(program.clone()); // arg0 is program name
    for arg in std::mem::take(&mut command.args) {
        argv.push_os(arg.0.into_owned())?;
    }

    // Environment
    let final_env = command.take_final_env();

    let mut env = RawStringVec::with_capacity(final_env.len());
    for (k, v) in final_env {
        let mut k = k.0.into_owned();
        k.reserve_exact(v.0.len() + 2);
        k.push("=");
        k.push(&v.0);

        env.push_os(k)?;
    }

    // Stdio
    let pass_fds = std::mem::take(&mut command.pass_fds);

    let mut stdin_write = None;
    let mut stdout_read = None;
    let mut stderr_read = None;

    let mut fds_to_drop: Vec<OwnedFd> = Vec::new();
    let mut stdin_read = None;
    let mut stdout_write = None;
    let mut stderr_write = None;

    match command.stdin {
        PandoraStdioWriteMode::Null => {
            if context.dev_null_fd.is_none() {
                let dev_null = unsafe { cvt(libc::open(c"/dev/null".as_ptr(), libc::O_RDWR))? };
                context.dev_null_fd = Some(dev_null);
            }
            stdin_read = context.dev_null_fd.clone();
        },
        PandoraStdioWriteMode::Inherit => {},
        PandoraStdioWriteMode::Pipe => {
            let (read, write) = std::io::pipe()?;
            stdin_write = Some(write);
            stdin_read = Some(read.as_raw_fd());
            fds_to_drop.push(read.into());
        }
    }
    match command.stdout {
        PandoraStdioReadMode::Pipe => {
            let (read, write) = std::io::pipe()?;
            stdout_read = Some(read);
            stdout_write = Some(write.as_raw_fd());
            fds_to_drop.push(write.into());
        },
        PandoraStdioReadMode::Null => {
            if context.dev_null_fd.is_none() {
                let dev_null = unsafe { cvt(libc::open(c"/dev/null".as_ptr(), libc::O_RDWR))? };
                context.dev_null_fd = Some(dev_null);
            }
            stdout_write = context.dev_null_fd.clone();
        },
        PandoraStdioReadMode::Inherit => {},
    }
    match command.stderr {
        PandoraStdioReadMode::Pipe => {
            let (read, write) = std::io::pipe()?;
            stderr_read = Some(read);
            stderr_write = Some(write.as_raw_fd());
            fds_to_drop.push(write.into());
        },
        PandoraStdioReadMode::Null => {
            if context.dev_null_fd.is_none() {
                let dev_null = unsafe { cvt(libc::open(c"/dev/null".as_ptr(), libc::O_RDWR))? };
                context.dev_null_fd = Some(dev_null);
            }
            stderr_write = context.dev_null_fd.clone();
        },
        PandoraStdioReadMode::Inherit => {},
    }

    let workdir = if let Some(current_dir) = command.current_dir {
        let Ok(workdir) = CString::new(current_dir.into_os_string().into_vec()) else {
            return Err(Error::new(ErrorKind::InvalidData, "program contained null byte"))
        };
        Some(workdir)
    } else {
        None
    };

    let pid = unsafe { cvt(libc::fork())? };

    argv.ensure_null_terminated();
    env.ensure_null_terminated();
    #[cfg(target_os = "macos")]
    if let Some(sandbox_params) = &mut command.sandbox_params {
        sandbox_params.ensure_null_terminated();
    }

    if pid == 0 {
        _ = exec(
            program.as_ptr(),
            argv.into_null_terminated_ptr() as *const *const c_char,
            env.into_null_terminated_ptr() as *const *const c_char,
            stdin_read,
            stdout_write,
            stderr_write,
            workdir.as_ref().map(|dir| dir.as_ptr()),
            &pass_fds,
            #[cfg(target_os = "macos")]
            command.sandbox_profile,
            #[cfg(target_os = "macos")]
            command.sandbox_params.map(|v| v.into_null_terminated_ptr().cast())
        );
        unsafe { libc::_exit(1) }
    }

    Ok(PandoraChild {
        process: PandoraProcess::new(pid),
        stdin: stdin_write,
        stdout: stdout_read,
        stderr: stderr_read
    })
}

fn exec(
    program: *const c_char,
    argv: *const *const c_char,
    env: *const *const c_char,
    stdin: Option<RawFd>,
    stdout: Option<RawFd>,
    stderr: Option<RawFd>,
    workdir: Option<*const c_char>,
    pass_fds: &[OwnedFd],
    #[cfg(target_os = "macos")]
    sandbox_profile: Option<CString>,
    #[cfg(target_os = "macos")]
    sandbox_params: Option<*const *const c_char>,
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

        if let Some(workdir) = workdir {
            cvt_r(|| libc::chdir(workdir))?;
        }

        for fd in pass_fds {
            cvt_r(|| libc::ioctl(fd.as_raw_fd(), libc::FIONCLEX))?;
        }

        #[cfg(target_os = "linux")]
        cvt_r(|| libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0))?;

        #[cfg(target_os = "macos")]
        if let Some(sandbox_profile) = sandbox_profile {
            let mut errorbuf = std::ptr::null_mut();
            let res = if let Some(sandbox_params) = sandbox_params {
                sandbox_init_with_parameters(sandbox_profile.as_ptr(), 0,
                    sandbox_params, &mut errorbuf)
            } else {
                sandbox_init(sandbox_profile.as_ptr(), 0, &mut errorbuf)
            };

            if !errorbuf.is_null() {
                eprintln!("An error occurred while trying to set up the sandbox");
                eprintln!("{:?}", std::ffi::CStr::from_ptr(errorbuf));
                sandbox_free_error(errorbuf);
            }

            if res != 0 || !errorbuf.is_null() {
                return Err(Error::new(ErrorKind::InvalidInput, "provided sandbox profile was invalid"));
            }
        }

        cvt(libc::execvp(program, argv))?;
        Ok(())
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn sandbox_init_with_parameters(profile: *const libc::c_char, flags: u64, parameters: *const *const libc::c_char, errorbuf: *mut *mut libc::c_char) -> libc::c_int;
    fn sandbox_init(profile: *const libc::c_char, flags: u64, errorbuf: *mut *mut libc::c_char) -> libc::c_int;
    fn sandbox_free_error(errorbuf: *mut libc::c_char);
}
