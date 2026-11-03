use std::{path::Path, process::Stdio};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub async fn run(command_line: &str, java_path: &Path, game_dir: &Path) -> std::io::Result<()> {
    let mut tokens = split_command_line(command_line)
        .into_iter()
        .map(|token| substitute_variables(&token, java_path, game_dir));
    let Some(program) = tokens.next() else {
        return Ok(());
    };
    let args: Vec<String> = tokens.collect();
    let game_dir = game_dir.to_path_buf();

    let (program, status) = tokio::task::spawn_blocking(move || {
        let mut command = std::process::Command::new(&program);
        command.args(&args).current_dir(&game_dir).stdin(Stdio::null());

        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut command, CREATE_NO_WINDOW);

        match command.status() {
            Ok(status) => Ok((program, status)),
            Err(error) => Err(std::io::Error::new(
                error.kind(),
                format!("failed to start pre-launch command `{program}`: {error}"),
            )),
        }
    })
    .await
    .map_err(std::io::Error::other)??;

    if status.success() {
        return Ok(());
    }
    Err(std::io::Error::other(format!("pre-launch command `{program}` failed ({status})")))
}

fn substitute_variables(token: &str, java_path: &Path, game_dir: &Path) -> String {
    let instance_dir = game_dir.parent().unwrap_or(game_dir);
    token
        .replace("$INST_JAVA", &strip_verbatim_prefix(java_path))
        .replace("$INST_MC_DIR", &strip_verbatim_prefix(game_dir))
        .replace("$INST_DIR", &strip_verbatim_prefix(instance_dir))
}

fn strip_verbatim_prefix(path: &Path) -> String {
    let path = path.to_string_lossy();
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        path.into_owned()
    }
}

fn split_command_line(command_line: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        split_windows_command_line(command_line)
    }
    #[cfg(not(windows))]
    {
        match shell_words::split(command_line) {
            Ok(split) => split,
            Err(_) => command_line.split_whitespace().map(|word| word.to_string()).collect(),
        }
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn split_windows_command_line(command_line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut in_quotes = false;

    for c in command_line.chars() {
        if c == '"' {
            in_quotes = !in_quotes;
            in_token = true;
        } else if c.is_whitespace() && !in_quotes {
            if in_token {
                tokens.push(std::mem::take(&mut current));
                in_token = false;
            }
        } else {
            current.push(c);
            in_token = true;
        }
    }
    if in_token {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_command_line_keeps_backslashes_and_groups_quotes() {
        assert_eq!(
            split_windows_command_line(r#"$INST_DIR\hook.bat "C:\Program Files\tool.exe" "a b""#),
            vec![r"$INST_DIR\hook.bat", r"C:\Program Files\tool.exe", "a b"],
        );
    }

    #[cfg(windows)]
    #[test]
    fn variables_are_substituted_after_splitting_and_keep_spaces() {
        let java = Path::new(r"\\?\C:\My Games\jre\bin\java.exe");
        let game_dir = Path::new(r"\\?\C:\My Games\inst\.minecraft");
        let tokens: Vec<String> = split_windows_command_line(r"$INST_JAVA $INST_MC_DIR\mods $INST_DIR")
            .iter()
            .map(|token| substitute_variables(token, java, game_dir))
            .collect();
        assert_eq!(
            tokens,
            vec![
                r"C:\My Games\jre\bin\java.exe",
                r"C:\My Games\inst\.minecraft\mods",
                r"C:\My Games\inst",
            ]
        );
    }
}
