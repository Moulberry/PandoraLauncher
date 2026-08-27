use std::{
    ffi::{OsStr, OsString},
    process::Command,
};

use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Detects if the current process is running under Wine by checking environment
/// variables that Wine injects into guest processes.
fn is_running_under_wine() -> bool {
    std::env::var_os("WINEPREFIX").is_some()
        || std::env::var_os("WINELOADER").is_some()
        || std::env::var_os("WINEDEBUG").is_some()
}

/// Constructs a Command to invoke Wine's winebrowser utility, which forwards
/// file/URL open requests to the host OS's default handler.
fn winebrowser_command<T: AsRef<OsStr>>(path: T) -> Command {
    let mut cmd = Command::new("winebrowser");
    cmd.arg(option_safe_path(path.as_ref(), false));
    // Match Windows behavior: suppress console window flash
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

fn option_safe_path(path: &OsStr, protect_explorer_options: bool) -> OsString {
    use std::os::windows::ffi::OsStrExt;
    let starts_with_dash = path.encode_wide().next() == Some(b'-' as u16);
    let explorer_option = protect_explorer_options && is_explorer_option(path);
    if starts_with_dash || explorer_option {
        let mut safe = OsString::from(".\\");
        safe.push(path);
        safe
    } else {
        path.to_os_string()
    }
}

/// Return whether `path` would be interpreted as an `explorer.exe` option.
///
/// This distinction is important because Explorer is used as a fallback launcher
/// for potentially untrusted paths. Passing an option-shaped path unchanged would
/// let the input control Explorer's command-line behavior instead of identifying
/// the item to open. [`option_safe_path()`] uses this check to prefix such input
/// with `.\`, forcing Explorer to treat it as a relative path.
///
/// Explorer accepts these case-insensitive options:
///
/// - `/e` opens the folder using Explorer's navigation-pane view.
/// - `/n` opens a new window, even if the location is already open.
/// - `/separate` opens the folder in a separate Explorer process.
/// - `/root,<path>` makes `<path>` the root of the displayed folder tree.
/// - `/select,<path>` opens the parent folder and selects `<path>`.
///
/// The parameterized options are matched by prefix because Explorer separates
/// the option from its path with a comma.
fn is_explorer_option(path: &OsStr) -> bool {
    let path = path.to_string_lossy().to_ascii_lowercase();
    matches!(path.as_str(), "/e" | "/n" | "/separate")
        || path.starts_with("/root,")
        || path.starts_with("/select,")
}

pub fn commands<T: AsRef<OsStr>>(path: T) -> Vec<Command> {
    let mut cmds = Vec::new();
    let path = path.as_ref();

    // When under Wine, try winebrowser first for seamless host integration.
    if is_running_under_wine() {
        cmds.push(winebrowser_command(path));
    }

    let mut cmd = Command::new("powershell.exe");
    cmd.arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-Command")
        .arg("Start-Process -FilePath $env:OPEN_RS_TARGET")
        .env("OPEN_RS_TARGET", path)
        .creation_flags(CREATE_NO_WINDOW);
    cmds.push(cmd);

    let mut cmd = Command::new("explorer.exe");
    cmd.arg(option_safe_path(path, true))
        .creation_flags(CREATE_NO_WINDOW);
    cmds.push(cmd);

    #[cfg(feature = "insecure")]
    cmds.push(legacy_cmd_command(path, None));

    cmds
}

pub fn with_command<T: AsRef<OsStr>>(path: T, app: impl Into<String>) -> Command {
    #[cfg(feature = "insecure")]
    return legacy_cmd_command(path.as_ref(), Some(app.into()));

    #[cfg(not(feature = "insecure"))]
    let mut cmd = Command::new(resolve_app_path(app.into()));
    #[cfg(not(feature = "insecure"))]
    cmd.arg(path.as_ref()).creation_flags(CREATE_NO_WINDOW);
    #[cfg(not(feature = "insecure"))]
    cmd
}

#[cfg(not(feature = "insecure"))]
fn resolve_app_path(app: String) -> OsString {
    if app.contains(['\\', '/']) {
        return app.into();
    }

    app_path_from_registry(&app).unwrap_or_else(|| app.into())
}

#[cfg(not(feature = "insecure"))]
fn app_path_from_registry(app: &str) -> Option<OsString> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    const HKEY_CURRENT_USER: isize = 0x80000001u32 as usize as isize;
    const HKEY_LOCAL_MACHINE: isize = 0x80000002u32 as usize as isize;
    const RRF_RT_REG_SZ: u32 = 0x00000002;

    let key: Vec<_> = OsString::from(format!(
        r"Software\Microsoft\Windows\CurrentVersion\App Paths\{app}"
    ))
    .encode_wide()
    .chain(std::iter::once(0))
    .collect();

    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let mut bytes = 0;
        let status = unsafe {
            RegGetValueW(
                root,
                key.as_ptr(),
                std::ptr::null(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut bytes,
            )
        };
        if status != 0 || bytes < 2 {
            continue;
        }

        let mut value = vec![0u16; bytes as usize / 2];
        let status = unsafe {
            RegGetValueW(
                root,
                key.as_ptr(),
                std::ptr::null(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                value.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        if status == 0 {
            if value.last() == Some(&0) {
                value.pop();
            }
            return Some(OsString::from_wide(&value));
        }
    }
    None
}

#[cfg(not(feature = "insecure"))]
#[link(name = "advapi32")]
extern "system" {
    fn RegGetValueW(
        hkey: isize,
        sub_key: *const u16,
        value: *const u16,
        flags: u32,
        value_type: *mut u32,
        data: *mut core::ffi::c_void,
        data_size: *mut u32,
    ) -> i32;
}

#[cfg(feature = "insecure")]
fn legacy_cmd_command(path: &OsStr, app: Option<String>) -> Command {
    let mut cmd = Command::new("cmd");
    cmd.arg("/c").arg("start").raw_arg("\"\"");
    if let Some(app) = app {
        cmd.raw_arg(wrap_in_quotes(app));
    }
    cmd.raw_arg(wrap_in_quotes(path))
        .creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(feature = "insecure")]
fn wrap_in_quotes(value: impl AsRef<OsStr>) -> OsString {
    let mut quoted = OsString::from("\"");
    quoted.push(value);
    quoted.push("\"");
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_open_does_not_embed_the_target_in_shell_code() {
        let launchers = commands(r#"https://example.invalid/\" & calc.exe & rem \""#);
        let command = launchers
            .iter()
            .find(|command| command.get_program() == "powershell.exe")
            .unwrap();
        assert_eq!(command.get_program(), "powershell.exe");
        assert_eq!(
            command.get_args().last(),
            Some(OsStr::new("Start-Process -FilePath $env:OPEN_RS_TARGET")),
            "the PowerShell program must read the target as data instead of embedding it as code"
        );
        assert_eq!(
            command
                .get_envs()
                .find(|(name, _)| *name == OsStr::new("OPEN_RS_TARGET"))
                .and_then(|(_, value)| value),
            Some(OsStr::new(
                r#"https://example.invalid/\" & calc.exe & rem \""#
            )),
            "shell metacharacters must remain unchanged in the environment-variable value"
        );
        assert_eq!(
            launchers.last().unwrap().get_program(),
            OsStr::new("explorer.exe"),
            "Explorer must remain available as the fallback when PowerShell cannot launch"
        );
        assert_eq!(
            commands("/select,attacker-controlled")
                .iter()
                .find(|command| command.get_program() == "explorer.exe")
                .unwrap()
                .get_args()
                .collect::<Vec<_>>(),
            [OsStr::new(r#".\/select,attacker-controlled"#)],
            "an option-shaped path must be rewritten so Explorer treats it as a path"
        );
        assert_eq!(
            commands("/Users/alice/file.pdf")
                .iter()
                .find(|command| command.get_program() == "explorer.exe")
                .unwrap()
                .get_args()
                .collect::<Vec<_>>(),
            [OsStr::new("/Users/alice/file.pdf")],
            "a slash-prefixed path that is not an Explorer option must remain unchanged"
        );
    }

    #[test]
    fn custom_application_receives_the_target_as_one_argument() {
        let command = with_command(r#"C:\path with spaces\a\"b"#, "viewer.exe");
        #[cfg(not(feature = "insecure"))]
        {
            assert_eq!(command.get_program(), "viewer.exe");
            assert_eq!(
                command.get_args().collect::<Vec<_>>(),
                [OsStr::new(r#"C:\path with spaces\a\"b"#)],
                "spaces and quotes in the target must remain within one application argument"
            );
        }
        #[cfg(feature = "insecure")]
        assert_eq!(
            command.get_program(),
            "cmd",
            "the insecure feature restores the legacy shell-based application launcher"
        );
    }

    #[test]
    fn legacy_cmd_matches_the_insecure_feature() {
        let commands = commands("document.pdf");
        assert_eq!(
            commands
                .iter()
                .any(|command| command.get_program() == "cmd"),
            cfg!(feature = "insecure"),
            "cmd must only be offered when the insecure compatibility feature is enabled"
        );
    }
}

#[cfg(feature = "shellexecute-on-windows")]
pub fn that_detached<T: AsRef<OsStr>>(path: T) -> std::io::Result<()> {
    let path = path.as_ref();
    let is_dir = std::fs::metadata(path).map(|f| f.is_dir()).unwrap_or(false);

    if is_dir && shell_open_folder(path).is_ok() {
        return Ok(());
    };

    let path = wide(path);

    let (verb, class) = if is_dir {
        (ffi::EXPLORE, ffi::FOLDER)
    } else {
        (std::ptr::null(), std::ptr::null())
    };

    let mut info = ffi::SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<ffi::SHELLEXECUTEINFOW>() as _,
        nShow: ffi::SW_SHOWNORMAL,
        lpVerb: verb,
        lpClass: class,
        lpFile: path.as_ptr(),
        ..unsafe { std::mem::zeroed() }
    };

    unsafe { ShellExecuteExW(&mut info) }
}

#[cfg(feature = "shellexecute-on-windows")]
fn shell_open_folder(path: &OsStr) -> Result<(), std::io::Error> {
    let path = std::path::absolute(path)?;
    let path = wide(dunce::simplified(&path));
    unsafe { ffi::CoInitialize(std::ptr::null()) };
    let folder = unsafe { ffi::ILCreateFromPathW(path.as_ptr()) };
    if folder.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    let result = unsafe { SHOpenFolderAndSelectItems(folder, Some(&[folder]), 0) };
    unsafe { ffi::ILFree(folder) };
    result
}

#[cfg(feature = "shellexecute-on-windows")]
pub fn with_detached<T: AsRef<OsStr>>(path: T, app: impl Into<String>) -> std::io::Result<()> {
    let app = wide(app.into());
    let path = wide(path);

    let mut info = ffi::SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<ffi::SHELLEXECUTEINFOW>() as _,
        nShow: ffi::SW_SHOWNORMAL,
        lpFile: app.as_ptr(),
        lpParameters: path.as_ptr(),
        ..unsafe { std::mem::zeroed() }
    };

    unsafe { ShellExecuteExW(&mut info) }
}

/// Encodes as wide and adds a null character.
#[cfg(feature = "shellexecute-on-windows")]
#[inline]
fn wide<T: AsRef<OsStr>>(input: T) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    input
        .as_ref()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Performs an operation on a specified file.
///
/// <https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecuteexw>
#[allow(non_snake_case)]
#[cfg(feature = "shellexecute-on-windows")]
pub unsafe fn ShellExecuteExW(info: *mut ffi::SHELLEXECUTEINFOW) -> std::io::Result<()> {
    // ShellExecuteExW returns TRUE (i.e 1) on success
    // https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecuteexw#remarks
    if ffi::ShellExecuteExW(info) == 1 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

// Taken from https://microsoft.github.io/windows-docs-rs/doc/windows/
/// Opens a Windows Explorer window with specified items in a particular folder selected.
///
/// <https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shopenfolderandselectitems>
#[allow(non_snake_case)]
#[cfg(feature = "shellexecute-on-windows")]
pub unsafe fn SHOpenFolderAndSelectItems(
    pidlfolder: *const ffi::ITEMIDLIST,
    apidl: Option<&[*const ffi::ITEMIDLIST]>,
    dwflags: u32,
) -> std::io::Result<()> {
    use std::convert::TryInto;

    match ffi::SHOpenFolderAndSelectItems(
        pidlfolder,
        apidl.map_or(0, |slice| slice.len().try_into().unwrap()),
        apidl.map_or(core::ptr::null(), |slice| slice.as_ptr()),
        dwflags,
    ) {
        0 => Ok(()),
        error_code => Err(std::io::Error::from_raw_os_error(error_code)),
    }
}

#[cfg(feature = "shellexecute-on-windows")]
#[allow(non_snake_case)]
mod ffi {
    /// Activates and displays a window.
    /// If the window is minimized, maximized, or arranged, the system restores it to its original size and position.
    /// An application should specify this flag when displaying the window for the first time.
    ///
    /// <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow>
    pub const SW_SHOWNORMAL: i32 = 1;

    /// Null-terminated UTF-16 encoding of `explore`.
    pub const EXPLORE: *const u16 = [101, 120, 112, 108, 111, 114, 101, 0].as_ptr();

    /// Null-terminated UTF-16 encoding of `folder`.
    pub const FOLDER: *const u16 = [102, 111, 108, 100, 101, 114, 0].as_ptr();

    // Taken from https://docs.rs/windows-sys/latest/windows_sys/
    #[cfg_attr(not(target_arch = "x86"), repr(C))]
    #[cfg_attr(target_arch = "x86", repr(C, packed(1)))]
    #[allow(clippy::upper_case_acronyms)]
    pub struct SHELLEXECUTEINFOW {
        pub cbSize: u32,
        pub fMask: u32,
        pub hwnd: isize,
        pub lpVerb: *const u16,
        pub lpFile: *const u16,
        pub lpParameters: *const u16,
        pub lpDirectory: *const u16,
        pub nShow: i32,
        pub hInstApp: isize,
        pub lpIDList: *mut core::ffi::c_void,
        pub lpClass: *const u16,
        pub hkeyClass: isize,
        pub dwHotKey: u32,
        pub Anonymous: SHELLEXECUTEINFOW_0,
        pub hProcess: isize,
    }

    // Taken from https://docs.rs/windows-sys/latest/windows_sys/
    #[cfg_attr(not(target_arch = "x86"), repr(C))]
    #[cfg_attr(target_arch = "x86", repr(C, packed(1)))]
    pub union SHELLEXECUTEINFOW_0 {
        pub hIcon: isize,
        pub hMonitor: isize,
    }

    // Taken from https://microsoft.github.io/windows-docs-rs/doc/windows/
    #[repr(C, packed(1))]
    #[allow(clippy::upper_case_acronyms)]
    pub struct SHITEMID {
        pub cb: u16,
        pub abID: [u8; 1],
    }

    // Taken from https://microsoft.github.io/windows-docs-rs/doc/windows/
    #[repr(C, packed(1))]
    #[allow(clippy::upper_case_acronyms)]
    pub struct ITEMIDLIST {
        pub mkid: SHITEMID,
    }

    #[link(name = "shell32")]
    extern "system" {
        pub fn ShellExecuteExW(info: *mut SHELLEXECUTEINFOW) -> isize;
        pub fn ILCreateFromPathW(pszpath: *const u16) -> *mut ITEMIDLIST;
        pub fn SHOpenFolderAndSelectItems(
            pidlfolder: *const ITEMIDLIST,
            cidl: u32,
            apidl: *const *const ITEMIDLIST,
            dwflags: u32,
        ) -> i32;
        pub fn ILFree(pidl: *const ITEMIDLIST) -> i32;
    }

    #[link(name = "ole32")]
    extern "system" {
        pub fn CoInitialize(pvreserved: *const core::ffi::c_void) -> i32;
    }
}
