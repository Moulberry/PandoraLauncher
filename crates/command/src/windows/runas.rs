use std::{io::{Error, ErrorKind}, os::windows::io::AsRawHandle, path::Path};

use windows::Win32::{Foundation::HANDLE, System::JobObjects::AssignProcessToJobObject, UI::Shell::SHELLEXECUTEINFOW};

use crate::{PandoraChild, PandoraCommand, process::PandoraProcess, spawner::SpawnContext, windows::windows_helpers};

pub fn spawn(command: PandoraCommand, context: &mut SpawnContext) -> std::io::Result<PandoraChild> {
    if command.inherit_env.is_some() || !command.env.is_empty() {
        return Err(Error::new(ErrorKind::InvalidInput, "cannot set custom environment for elevated process"));
    }

    if context.job_handle.is_none() {
        context.job_handle = Some(windows_helpers::to_owned_handle(windows_helpers::create_job_object()?)?);
    }

    let path = Path::new(&command.executable.0);
    let resolved = if path.components().count() > 1 {
        let Ok(path) = path.canonicalize() else {
            return Err(Error::new(ErrorKind::NotFound, "executable file doesn't exist"));
        };
        path
    } else if let Some(path) = crate::path_cache::get_command_path(&command.executable.0) {
        path.to_path_buf()
    } else {
        return Err(Error::new(ErrorKind::NotFound, "unable to resolve executable"));
    };

    use std::os::windows::ffi::OsStrExt;
    let application_name = resolved.into_os_string().encode_wide()
        .chain([0])
        .collect::<Vec<_>>();
    let command_line = windows_helpers::join_windows_shell_arg(command.args.as_slice()).encode_wide()
        .chain([0])
        .collect::<Vec<_>>();

    let mut sei: SHELLEXECUTEINFOW = SHELLEXECUTEINFOW::default();
    sei.fMask = windows::Win32::UI::Shell::SEE_MASK_NOASYNC | windows::Win32::UI::Shell::SEE_MASK_NOCLOSEPROCESS;
    sei.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as _;
    sei.lpVerb = windows::core::w!("runas");
    sei.lpFile = windows::core::PCWSTR(application_name.as_ptr());
    sei.lpParameters = windows::core::PCWSTR::from_raw(command_line.as_ptr());
    sei.nShow = windows::Win32::UI::WindowsAndMessaging::SW_HIDE.0;

    unsafe {
        windows::Win32::UI::Shell::ShellExecuteExW(&mut sei)?;
    }

    if sei.hProcess.is_invalid() {
        return Err(Error::new(ErrorKind::Other, "ShellExecuteExW returned invalid process handle. Operation completed via DDE?"));
    }

    unsafe {
        let job_handle = context.job_handle
            .as_ref()
            .map(|h| HANDLE(h.as_raw_handle()))
            .unwrap();
        _ = AssignProcessToJobObject(job_handle, sei.hProcess);
    }

    Ok(PandoraChild {
        process: PandoraProcess::new(sei.hProcess),
        stdin: None,
        stdout: None,
        stderr: None
    })
}
