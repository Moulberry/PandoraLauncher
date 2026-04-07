use std::{ffi::OsString, io::{Error, ErrorKind}, os::windows::io::{HandleOrInvalid, OwnedHandle}};

use crate::PandoraArg;

use windows::Win32::{Foundation::{DUPLICATE_SAME_ACCESS, DuplicateHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, TRUE}, Security::SECURITY_ATTRIBUTES, Storage::FileSystem::{CreateFileW, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING}, System::{JobObjects::{CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject}, Pipes::CreatePipe, Threading::GetCurrentProcess}};

pub fn join_windows_shell_arg(args: &[PandoraArg]) -> OsString {
    let mut string = Vec::new();

    let mut first = true;
    for arg in args {
        let arg = &arg.0;
        let mut backslashes = 0;

        if first {
            first = false;
        } else {
            string.push(b' ');
        }

        if arg.is_empty() {
            string.extend(b"\"\"");
            continue;
        }

        let arg_raw = arg.as_encoded_bytes();
        let quoted = arg_raw.contains(&b' ') || arg_raw.contains(&b'\t');
        if quoted {
            string.push(b'"');
        }

        for byte in arg_raw {
            if *byte == b'\\' {
                backslashes += 1;
            } else if *byte == b'"' {
                for _ in 0..backslashes*2 {
                    string.push(b'\\');
                }
                string.push(b'\\');
                string.push(b'"');
                backslashes = 0;
            } else {
                for _ in 0..backslashes {
                    string.push(b'\\');
                }
                backslashes = 0;
                string.push(*byte);
            }
        }

        if quoted {
            for _ in 0..backslashes*2 {
                string.push(b'\\');
            }
        } else {
            for _ in 0..backslashes {
                string.push(b'\\');
            }
        }

        if quoted {
            string.push(b'"');
        }
    }

    unsafe {
        OsString::from_encoded_bytes_unchecked(string)
    }
}

pub fn create_job_object() -> std::io::Result<HANDLE> {
    let job_handle = unsafe {
        CreateJobObjectW(
            None,
            windows::core::PCWSTR::default()
        )?
    };
    if job_handle.is_invalid() {
        return Err(Error::new(ErrorKind::Other, "CreateJobObjectW returned invalid handle"));
    }
    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    unsafe {
        SetInformationJobObject(
            job_handle,
            JobObjectExtendedLimitInformation,
            &info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION as _,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32
        )?
    }
    Ok(job_handle)
}

pub fn create_inheritable_pipe() -> std::io::Result<(HANDLE, HANDLE)> {
    let mut read = HANDLE::default();
    let mut write = HANDLE::default();

    let sa = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: TRUE,
    };

    unsafe {
        CreatePipe(
            &mut read,
            &mut write,
            Some(&sa),
            0
        )?;
    }

    if read.is_invalid() || write.is_invalid() {
        return Err(Error::new(ErrorKind::Other, "CreatePipe returned invalid handles"));
    }

    Ok((read, write))
}

pub fn open_null_device() -> std::io::Result<OwnedHandle> {
    let sa = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: TRUE,
    };
    let handle = unsafe {
        CreateFileW(
            windows::core::w!(r"\\.\NUL"),
            (GENERIC_READ | GENERIC_WRITE).0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            Some(&sa),
            OPEN_EXISTING,
            Default::default(),
            None,
        )?
    };
    if handle.is_invalid() {
        return Err(Error::new(ErrorKind::Other, "CreateFileW returned invalid handle"));
    }
    to_owned_handle(handle)
}

pub fn duplicate_handle_as_inheritable(handle: HANDLE) -> std::io::Result<HANDLE> {
    let mut duplicated = HANDLE::default();
    unsafe {
        let cur_proc = GetCurrentProcess();
        DuplicateHandle(
           cur_proc,
            handle,
            cur_proc,
            &mut duplicated,
            0,
            true,
            DUPLICATE_SAME_ACCESS,
        )?;
    }
    if duplicated.is_invalid() {
        return Err(Error::new(ErrorKind::Other, "DuplicateHandle returned invalid handle"));
    }
    Ok(duplicated)
}

pub fn to_owned_handle(handle: HANDLE) -> std::io::Result<OwnedHandle> {
    let handle = unsafe { HandleOrInvalid::from_raw_handle(handle.0) };
    if let Ok(handle) = OwnedHandle::try_from(handle) {
        Ok(handle)
    } else {
        Err(Error::new(ErrorKind::InvalidInput, "cannot convert invalid handle to owned handle"))
    }
}
