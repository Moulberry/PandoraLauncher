use std::{
    env, io,
    process::{Command, Stdio},
};

fn run(mut commands: Vec<Command>) -> io::Result<Command> {
    let mut last_err = None;

    for mut command in commands.drain(..) {
        let result = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        match result {
            Ok(status) if status.success() => return Ok(command),
            Ok(status) => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Launcher {command:?} failed with {status:?}"),
                ));
            }
            Err(err) => {
                eprintln!("Failed to execute {command:?}: {err}");
                last_err = Some(err);
            }
        }
    }

    Err(last_err.expect("no launcher worked, at least one error"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();
    let path_or_url = match args.nth(1) {
        Some(arg) => arg,
        None => return Err("usage: open <path-or-url> [--with|-w program]".into()),
    };

    let command = match args.next() {
        Some(arg) if arg == "--with" || arg == "-w" => {
            let program = args
                .next()
                .ok_or("--with must be followed by the program to use for opening")?;
            run(vec![open::with_command(&path_or_url, program)])
        }
        Some(arg) => return Err(format!("Argument '{arg}' is invalid").into()),
        None => run(open::commands(&path_or_url)),
    }?;

    println!("Opened '{}' successfully with {command:?}.", path_or_url);
    Ok(())
}
