use std::{
    ffi::{OsStr, OsString},
    process::Command,
};

/// Prevent launchers from interpreting a path that starts with `-` as an option.
///
/// Prefixing such paths with `./` preserves their meaning as paths relative to the
/// current directory. This is used for launchers that do not support `--` as an
/// end-of-options marker.
fn option_safe_path(path: &OsStr) -> OsString {
    use std::os::unix::ffi::OsStrExt;

    if path.as_bytes().first() == Some(&b'-') {
        let mut safe = OsString::from("./");
        safe.push(path);
        safe
    } else {
        path.to_os_string()
    }
}

pub fn commands<T: AsRef<OsStr>>(path: T) -> Vec<Command> {
    let path = path.as_ref();
    let is_wsl = is_wsl::is_wsl();
    let mut commands = vec![];

    if is_wsl {
        commands.push(crate::wsl::command(path));
    }

    let safe_path = option_safe_path(path);

    let mut xdg = Command::new("xdg-open");
    xdg.arg(&safe_path);
    commands.push(xdg);

    let mut gio = Command::new("gio");
    gio.arg("open").arg(&safe_path);
    commands.push(gio);

    let mut gnome = Command::new("gnome-open");
    gnome.arg(&safe_path);
    commands.push(gnome);

    let mut kde = Command::new("kde-open");
    kde.arg("--").arg(path);
    commands.push(kde);

    commands
}

pub fn with_command<T: AsRef<OsStr>>(path: T, app: impl Into<String>) -> Command {
    let mut cmd = Command::new(app.into());
    cmd.arg(path.as_ref());
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(command: &Command) -> Vec<&OsStr> {
        command.get_args().collect()
    }

    #[test]
    fn protects_dash_leading_paths_from_launcher_option_parsing() {
        let commands = commands("--load-modules=/tmp/evil.so");

        let expected_len = 4;
        let commands = &commands[commands.len() - expected_len..];
        assert_eq!(commands.len(), expected_len);
        assert_eq!(commands[0].get_program(), "xdg-open");
        assert_eq!(
            args(&commands[0]),
            [OsStr::new("./--load-modules=/tmp/evil.so")],
            "xdg-open has no end-of-options marker, so the path must not start with a dash"
        );
        assert_eq!(commands[1].get_program(), "gio");
        assert_eq!(
            args(&commands[1]),
            [
                OsStr::new("open"),
                OsStr::new("./--load-modules=/tmp/evil.so")
            ],
            "gio has no end-of-options marker, so the path must not start with a dash"
        );
        let kde = commands.last().unwrap();
        assert_eq!(commands[2].get_program(), "gnome-open");
        assert_eq!(
            args(&commands[2]),
            [OsStr::new("./--load-modules=/tmp/evil.so")],
            "gnome-open would otherwise interpret the path as a module-loading option"
        );
        assert_eq!(kde.get_program(), "kde-open");
        assert_eq!(
            args(kde),
            [OsStr::new("--"), OsStr::new("--load-modules=/tmp/evil.so")],
            "kde-open supports separating its options from an unchanged path"
        );
    }

    #[test]
    fn leaves_urls_and_regular_paths_unchanged() {
        for path in ["https://example.com", "document.pdf", "/tmp/-document.pdf"] {
            let commands = commands(path);
            let expected_len = 4;
            let commands = &commands[commands.len() - expected_len..];
            assert_eq!(args(&commands[0]), [OsStr::new(path)]);
            assert_eq!(args(&commands[1]), [OsStr::new("open"), OsStr::new(path)]);
            assert_eq!(args(&commands[2]), [OsStr::new(path)]);
            assert_eq!(args(&commands[3]), [OsStr::new("--"), OsStr::new(path)]);
        }
    }
}
