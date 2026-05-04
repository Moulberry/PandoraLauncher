use std::ffi::OsStr;
use std::io;
use std::mem;
use std::os::raw::c_ushort;
use std::os::windows::ffi::OsStrExt;
use std::process::ExitStatus;
use std::ptr;

use windows_sys::Win32::System::Com::{
    CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows_sys::Win32::System::Threading::GetExitCodeProcess;
use windows_sys::Win32::System::Threading::WaitForSingleObject;
use windows_sys::Win32::System::Threading::INFINITE;
use windows_sys::Win32::UI::Shell::SEE_MASK_NOASYNC;
use windows_sys::Win32::UI::Shell::SEE_MASK_NOCLOSEPROCESS;
use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SHELLEXECUTEINFOW};
use windows_sys::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_NORMAL};

use crate::Command;

unsafe fn win_runas(cmd: *const c_ushort, args: *const c_ushort, show: bool) -> u32 {
    let mut code = 0;
    let mut sei: SHELLEXECUTEINFOW = mem::zeroed();
    let verb = "runas\0".encode_utf16().collect::<Vec<u16>>();
    CoInitializeEx(
        ptr::null(),
        COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE,
    );

    sei.fMask = SEE_MASK_NOASYNC | SEE_MASK_NOCLOSEPROCESS;
    sei.cbSize = mem::size_of::<SHELLEXECUTEINFOW>() as _;
    sei.lpVerb = verb.as_ptr();
    sei.lpFile = cmd;
    sei.lpParameters = args;
    sei.nShow = if show { SW_NORMAL } else { SW_HIDE } as _;

    if ShellExecuteExW(&mut sei) == 0 || sei.hProcess == 0 {
        return !0;
    }

    WaitForSingleObject(sei.hProcess, INFINITE);

    if GetExitCodeProcess(sei.hProcess, &mut code) == 0 {
        !0
    } else {
        code
    }
}

pub fn runas_impl(cmd: &Command) -> io::Result<ExitStatus> {
    let mut params = String::new();
    for arg in cmd.args.iter() {
        let arg = arg.to_string_lossy();
        params.push(' ');
        if arg.len() == 0 {
            params.push_str("\"\"");
        } else if arg.find(&[' ', '\t', '"'][..]).is_none() {
            params.push_str(&arg);
        } else {
            params.push('"');
            for c in arg.chars() {
                match c {
                    '\\' => params.push_str("\\\\"),
                    '"' => params.push_str("\\\""),
                    c => params.push(c),
                }
            }
            params.push('"');
        }
    }

    let file = OsStr::new(&cmd.command)
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let params = OsStr::new(&params)
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();

    unsafe {
        Ok(mem::transmute(win_runas(
            file.as_ptr(),
            params.as_ptr(),
            !cmd.hide,
        )))
    }
}
