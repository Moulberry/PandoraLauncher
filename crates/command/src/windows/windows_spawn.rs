use std::{ffi::c_void, io::{Error, ErrorKind}, os::windows::{ffi::OsStrExt, io::{AsRawHandle, OwnedHandle}}};

use windows::{Win32::{Foundation::{HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation}, System::{Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE}, JobObjects::AssignProcessToJobObject, Threading::{CREATE_UNICODE_ENVIRONMENT, CreateProcessW, EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, STARTF_FORCEONFEEDBACK, STARTF_USESTDHANDLES, STARTUPINFOEXW, STARTUPINFOW}}}, core::Free};

use crate::{PandoraChild, PandoraCommand, PandoraStdioReadMode, PandoraStdioWriteMode, process::PandoraProcess, spawner::SpawnContext, windows::windows_helpers};

pub fn spawn(command: PandoraCommand, context: &mut SpawnContext) -> std::io::Result<PandoraChild> {
    spawn_with_attributes(command, context, None)
}

pub(crate) fn spawn_with_attributes(mut command: PandoraCommand, context: &mut SpawnContext, attributes: Option<LPPROC_THREAD_ATTRIBUTE_LIST>) -> std::io::Result<PandoraChild> {
    if context.job_handle.is_none() {
        context.job_handle = Some(windows_helpers::to_owned_handle(windows_helpers::create_job_object()?)?);
    }

    let env_map = command.take_final_env();
    let application_path = command.resolve_executable_path()?;
    let application_name = application_path.as_os_str().encode_wide()
        .chain([0])
        .collect::<Vec<_>>();
    let mut command_line = windows_helpers::join_windows_shell_arg(command.args.as_slice()).encode_wide()
        .chain([0])
        .collect::<Vec<_>>();
    let current_directory = command.current_dir.map(|dir| dir.as_os_str().encode_wide()
        .chain([0])
        .collect::<Vec<_>>());

    let mut env = Vec::new();
    if env_map.is_empty() {
        env.push(0);
    }
    for (k, v) in env_map {
        if k.0.as_encoded_bytes().contains(&b'\0') || v.0.as_encoded_bytes().contains(&b'\0') {
            return Err(Error::new(ErrorKind::InvalidData, "environment variable contained null byte"));
        }
        env.extend(k.0.encode_wide());
        env.push('=' as u16);
        env.extend(v.0.encode_wide());
        env.push(0);
    }
    env.push(0);

    let mut stdin_write = None;
    let mut stdout_read = None;
    let mut stderr_read = None;

    let mut handles_to_close = Vec::new();

    let mut stdin_read = None;
    let mut stdout_write = None;
    let mut stderr_write = None;

    match command.stdin {
        PandoraStdioWriteMode::Null => {
            if context.null_device.is_none() {
                context.null_device = Some(windows_helpers::open_null_device()?);
            }
            stdin_read = context.null_device.as_ref().map(|d| HANDLE(d.as_raw_handle()));
        },
        PandoraStdioWriteMode::Inherit => {
            let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) }?;
            if !handle.is_invalid() {
                let handle = windows_helpers::duplicate_handle_as_inheritable(handle)?;
                handles_to_close.push(windows_helpers::to_owned_handle(handle)?);
                stdin_read = Some(handle);
            }
        },
        PandoraStdioWriteMode::Pipe => {
            let (read, write) = std::io::pipe()?;
            let owned: OwnedHandle = read.into();

            stdin_write = Some(write);
            unsafe { SetHandleInformation(HANDLE(owned.as_raw_handle()), HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT)? };
            stdin_read = Some(HANDLE(owned.as_raw_handle()));
            handles_to_close.push(owned);
        }
    }
    match command.stdout {
        PandoraStdioReadMode::Pipe => {
            let (read, write) = std::io::pipe()?;
            let owned: OwnedHandle = write.into();

            stdout_read = Some(read);
            unsafe { SetHandleInformation(HANDLE(owned.as_raw_handle()), HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT)? };
            stdout_write = Some(HANDLE(owned.as_raw_handle()));
            handles_to_close.push(owned);
        },
        PandoraStdioReadMode::Null => {
            if context.null_device.is_none() {
                context.null_device = Some(windows_helpers::open_null_device()?);
            }
            stdout_write = context.null_device.as_ref().map(|d| HANDLE(d.as_raw_handle()));
        },
        PandoraStdioReadMode::Inherit => {
            let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) }?;
            if !handle.is_invalid() {
                let handle = windows_helpers::duplicate_handle_as_inheritable(handle)?;
                handles_to_close.push(windows_helpers::to_owned_handle(handle)?);
                stdout_write = Some(handle);
            }
        },
    }
    match command.stderr {
        PandoraStdioReadMode::Pipe => {
            let (read, write) = std::io::pipe()?;
            let owned: OwnedHandle = write.into();

            stderr_read = Some(read);
            unsafe { SetHandleInformation(HANDLE(owned.as_raw_handle()), HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT)? };
            stderr_write = Some(HANDLE(owned.as_raw_handle()));
            handles_to_close.push(owned);
        },
        PandoraStdioReadMode::Null => {
            if context.null_device.is_none() {
                context.null_device = Some(windows_helpers::open_null_device()?);
            }
            stderr_write = context.null_device.as_ref().map(|d| HANDLE(d.as_raw_handle()));
        },
        PandoraStdioReadMode::Inherit => {
            let handle = unsafe { GetStdHandle(STD_ERROR_HANDLE) }?;
            if !handle.is_invalid() {
                let handle = windows_helpers::duplicate_handle_as_inheritable(handle)?;
                handles_to_close.push(windows_helpers::to_owned_handle(handle)?);
                stderr_write = Some(handle);
            }
        },
    }

    let mut process_creation_flags = PROCESS_CREATION_FLAGS::default();
    process_creation_flags |= CREATE_UNICODE_ENVIRONMENT;

    let mut si: STARTUPINFOW = Default::default();
    si.cb = size_of::<STARTUPINFOW>() as u32;

    if let Some(stdin_read) = stdin_read {
        si.dwFlags |= STARTF_USESTDHANDLES;
        si.hStdInput = stdin_read;
    }
    if let Some(stdout_write) = stdout_write {
        si.dwFlags |= STARTF_USESTDHANDLES;
        si.hStdOutput = stdout_write;
    }
    if let Some(stderr_write) = stderr_write {
        si.dwFlags |= STARTF_USESTDHANDLES;
        si.hStdError = stderr_write;
    }

    if command.force_feedback {
        si.dwFlags |= STARTF_FORCEONFEEDBACK;
    }

    let mut sip = &si as *const STARTUPINFOW;
    let si_ex;
    if let Some(attributes) = attributes {
        process_creation_flags |= EXTENDED_STARTUPINFO_PRESENT;
        si.cb = size_of::<STARTUPINFOEXW>() as u32;
        si_ex = STARTUPINFOEXW {
            StartupInfo: si,
            lpAttributeList: attributes,
        };
        sip = &si_ex as *const _ as *const STARTUPINFOW;
    }

    let mut pi: PROCESS_INFORMATION = Default::default();
    unsafe {
        CreateProcessW(
            windows::core::PCWSTR(application_name.as_ptr()),
            Some(windows::core::PWSTR(command_line.as_mut_ptr())),
            None,
            None,
            stdin_read.is_some() || stdout_write.is_some() || stderr_write.is_some(),
            process_creation_flags,
            Some(env.as_ptr() as *mut c_void),
            current_directory.as_ref().map(|dir| windows::core::PCWSTR(dir.as_ptr())).unwrap_or_default(),
            sip,
            &mut pi
        )?
    }

    unsafe { pi.hThread.free(); };

    if pi.hProcess.is_invalid() {
        return Err(Error::new(ErrorKind::Other, "CreateProcessW returned invalid process handle"));
    }

    drop(handles_to_close);

    unsafe {
        let job_handle = context.job_handle
            .as_ref()
            .map(|h| HANDLE(h.as_raw_handle()))
            .unwrap();
        _ = AssignProcessToJobObject(job_handle, pi.hProcess);
    }

    return Ok(PandoraChild {
        process: PandoraProcess::new(pi.hProcess),
        stdin: stdin_write,
        stdout: stdout_read,
        stderr: stderr_read
    });
}
