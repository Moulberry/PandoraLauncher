use std::{ffi::CString, io::{Error, ErrorKind}, os::unix::ffi::OsStringExt};

use security_framework_sys::authorization::{AuthorizationCreate, AuthorizationExecuteWithPrivileges, AuthorizationFree, AuthorizationRef, errAuthorizationSuccess, kAuthorizationFlagDefaults};

use crate::{PandoraChild, PandoraCommand, PandoraProcess, unix::unix_helpers::{RawStringVec, cvt_r}};

pub fn spawn(mut command: PandoraCommand) -> std::io::Result<PandoraChild> {
    // Program
    let resolved = command.resolve_executable_path()?;
    let Ok(program) = CString::new(resolved.clone().into_os_string().into_vec()) else {
        return Err(Error::new(ErrorKind::InvalidData, "program contained null byte"))
    };

    // Arguments
    let mut argv = RawStringVec::with_capacity(command.args.len());
    for arg in std::mem::take(&mut command.args) {
        argv.push_os(arg.0.into_owned())?;
    }

    let mut authorization = OwnedAuthorization(std::ptr::null_mut());
    let status = unsafe {
        AuthorizationCreate(
            std::ptr::null(),
            std::ptr::null(),
            kAuthorizationFlagDefaults,
            &mut authorization.0
        )
    };
    if status != errAuthorizationSuccess {
        return Err(Error::new(ErrorKind::Other, format!("unable to create authorization: {status}")));
    }

    let mut stdout = std::ptr::null_mut();
    let status = unsafe {
        AuthorizationExecuteWithPrivileges(
            authorization.0,
            program.as_ptr(),
            kAuthorizationFlagDefaults,
            argv.as_null_terminated_ptr(),
            &mut stdout
        )
    };
    if status != errAuthorizationSuccess {
        return Err(Error::new(ErrorKind::Other, format!("unable to execute with privileges: {status}")));
    }

    let fileno = unsafe { cvt_r(|| libc::fileno(stdout)) }?;
    let pid = unsafe { libc::fcntl(fileno, libc::F_GETOWN, 0) };

    return Ok(PandoraChild {
        process: PandoraProcess::new(pid),
        stdin: None,
        stdout: None,
        stderr: None
    });
}

struct OwnedAuthorization(AuthorizationRef);

impl Drop for OwnedAuthorization {
    fn drop(&mut self) {
        if self.0.is_null() {
            unsafe { AuthorizationFree(self.0, kAuthorizationFlagDefaults) };
        }
    }
}
