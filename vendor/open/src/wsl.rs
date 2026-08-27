use std::{
    env,
    ffi::{OsStr, OsString},
    process::Command,
};

pub fn command(path: &OsStr) -> Command {
    command_with_distro(path, env::var("WSL_DISTRO_NAME").ok().as_deref())
}

fn command_with_distro(path: &OsStr, distro: Option<&str>) -> Command {
    let path = interop_path(path, distro);
    let mut cmd = Command::new("powershell.exe");
    cmd.arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-Command")
        .arg("Start-Process -FilePath $env:OPEN_RS_TARGET")
        .env("WSLENV", wslenv_with_target(env::var_os("WSLENV")))
        .env("OPEN_RS_TARGET", path);
    cmd
}

/// Adds `OPEN_RS_TARGET` to `WSLENV` so WSL forwards it to Windows processes,
/// while preserving any existing entries. Without it, PowerShell would not see
/// `OPEN_RS_TARGET`, even though it is set on the command.
fn wslenv_with_target(existing: Option<OsString>) -> OsString {
    let mut wslenv = OsString::from("OPEN_RS_TARGET");
    if let Some(existing) = existing.filter(|value| !value.is_empty()) {
        wslenv.push(":");
        wslenv.push(existing);
    }
    wslenv
}

/// Converts WSL paths into paths Windows can open through interop.
///
/// Paths that are already Windows-compatible, relative paths, paths without a
/// known distro, and non-UTF-8 paths are returned unchanged.
///
/// Examples:
///
/// * `https://example.com` -> `https://example.com`
/// * `/mnt/c/Users/alice/doc.pdf` -> `C:\Users\alice\doc.pdf`
/// * `/home/alice/doc.pdf` with distro `Ubuntu` -> `\\wsl$\Ubuntu\home\alice\doc.pdf`
fn interop_path(path: &OsStr, distro: Option<&str>) -> OsString {
    let path = match path.to_str() {
        Some(path) => path,
        None => return path.to_owned(),
    };

    if !path.starts_with('/') {
        return OsString::from(path);
    }

    if let Some(path) = drvfs_path(path) {
        return OsString::from(path);
    }

    match distro {
        Some(distro) if !distro.is_empty() => {
            OsString::from(format!(r"\\wsl$\{distro}{}", path.replace('/', r"\")))
        }
        _ => OsString::from(path),
    }
}

/// Converts WSL drvfs mount paths into Windows drive paths.
///
/// Returns `None` when the path is not a `/mnt/<drive>` path, when the drive is
/// not an ASCII letter, or when extra path components are missing the separator.
///
/// Examples:
///
/// * `/mnt/c` -> `C:\`
/// * `/mnt/c/Users/alice/doc.pdf` -> `C:\Users\alice\doc.pdf`
/// * `/home/alice/doc.pdf` -> `None`
fn drvfs_path(path: &str) -> Option<String> {
    let rest = path.strip_prefix("/mnt/")?;
    let mut chars = rest.chars();
    let drive = chars.next()?;

    if !drive.is_ascii_alphabetic() {
        return None;
    }

    let rest = chars.as_str();
    if rest.is_empty() {
        return Some(format!("{}:\\", drive.to_ascii_uppercase()));
    }

    let rest = rest.strip_prefix('/')?;
    Some(format!(
        "{}:\\{}",
        drive.to_ascii_uppercase(),
        rest.replace('/', r"\")
    ))
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsStr, process::Command};

    use super::{command, command_with_distro, interop_path};

    fn command_args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn command_starts_with_powershell() {
        let command = command(OsStr::new("https://example.com"));

        assert_eq!(
            command.get_program(),
            "powershell.exe",
            "avoid wslview as it's unmaintained"
        );
        assert_eq!(
            command_args(&command),
            [
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Process -FilePath $env:OPEN_RS_TARGET"
            ]
        );
        assert_eq!(
            command
                .get_envs()
                .find(|(name, _)| *name == OsStr::new("OPEN_RS_TARGET"))
                .and_then(|(_, value)| value),
            Some(OsStr::new("https://example.com"))
        );
        assert!(
            command
                .get_envs()
                .find(|(name, _)| *name == OsStr::new("WSLENV"))
                .and_then(|(_, value)| value)
                .map_or(false, |value| {
                    value.to_string_lossy().split(':').next() == Some("OPEN_RS_TARGET")
                }),
            "WSL must be instructed to forward the target to PowerShell"
        );
    }

    #[test]
    fn command_does_not_embed_powershell_metacharacters() {
        let command = command_with_distro(OsStr::new("https://example.com/a'b;c"), Some("Ubuntu"));

        assert_eq!(
            command_args(&command),
            [
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Process -FilePath $env:OPEN_RS_TARGET"
            ]
        );
        assert_eq!(
            command
                .get_envs()
                .find(|(name, _)| *name == OsStr::new("OPEN_RS_TARGET"))
                .and_then(|(_, value)| value),
            Some(OsStr::new("https://example.com/a'b;c"))
        );
    }

    #[test]
    fn interop_path_converts_absolute_linux_paths_to_unc_paths() {
        assert_eq!(
            interop_path(OsStr::new("/home/alice/doc.pdf"), Some("Ubuntu")),
            r"\\wsl$\Ubuntu\home\alice\doc.pdf"
        );
    }

    #[test]
    fn interop_path_converts_drvfs_paths_to_windows_paths() {
        assert_eq!(
            interop_path(OsStr::new("/mnt/c/Users/alice/doc.pdf"), Some("Ubuntu")),
            r"C:\Users\alice\doc.pdf"
        );
        assert_eq!(interop_path(OsStr::new("/mnt/d"), Some("Ubuntu")), r"D:\");
    }

    #[test]
    fn target_is_added_to_existing_wslenv() {
        assert_eq!(
            super::wslenv_with_target(Some("RUST_LOG/u:PATH/l".into())),
            "OPEN_RS_TARGET:RUST_LOG/u:PATH/l"
        );
    }
}
