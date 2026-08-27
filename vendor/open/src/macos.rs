use std::{ffi::OsStr, process::Command};

pub fn commands<T: AsRef<OsStr>>(path: T) -> Vec<Command> {
    let mut cmd = Command::new("/usr/bin/open");
    cmd.arg("--").arg(path.as_ref());
    vec![cmd]
}

pub fn with_command<T: AsRef<OsStr>>(path: T, app: impl Into<String>) -> Command {
    let mut cmd = Command::new("/usr/bin/open");
    cmd.arg("-a").arg(app.into()).arg("--").arg(path.as_ref());
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as SysCommand;
    use std::thread;
    use std::time::Duration;

    fn zombie_children_count() -> usize {
        let ppid = std::process::id().to_string();
        let output = SysCommand::new("ps")
            .args(["-axo", "ppid,stat"])
            .output()
            .expect("ps");
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .skip(1)
            .filter_map(|line| {
                let mut it = line.split_whitespace();
                let (child_ppid, stat) = (it.next()?, it.next()?);
                Some((child_ppid, stat))
            })
            .filter(|(child_ppid, stat)| *child_ppid == ppid.as_str() && stat.starts_with('Z'))
            .count()
    }

    #[test]
    fn that_detached_does_not_leave_zombie_children() {
        // Use a missing local path so `/usr/bin/open` still runs (the zombie
        // comes from spawn_detached, not from the target) without launching a
        // browser or other GUI app. `that()` returning Err is expected.
        let missing = "/tmp/open-rs-no-such-file-zombie-test";
        let before = zombie_children_count();
        for _ in 0..5 {
            let _ = crate::that_detached(missing);
        }
        thread::sleep(Duration::from_millis(300));
        let after = zombie_children_count();
        assert_eq!(
            before, after,
            "RACY: that_detached must not leave zombie children (before={before}, after={after})"
        );
    }

    #[test]
    fn separates_paths_from_open_options() {
        assert_eq!(
            commands("-aCalculator")[0].get_args().collect::<Vec<_>>(),
            [OsStr::new("--"), OsStr::new("-aCalculator")]
        );
        assert_eq!(
            with_command("-aCalculator", "Preview")
                .get_args()
                .collect::<Vec<_>>(),
            [
                OsStr::new("-a"),
                OsStr::new("Preview"),
                OsStr::new("--"),
                OsStr::new("-aCalculator")
            ]
        );
    }
}
