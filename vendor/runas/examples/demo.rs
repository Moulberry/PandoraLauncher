use runas;

#[cfg(windows)]
fn main() {
    println!("Running whoami /priv");
    println!(
        "Status: {}",
        runas::Command::new("cmd")
            .arg("/k")
            .arg("whoami")
            .arg("/priv")
            .gui(true)
            .force_prompt(false)
            .status()
            .expect("failed to execute")
    );
}

#[cfg(unix)]
fn main() {
    println!("Running id");
    println!(
        "Status: {}",
        runas::Command::new("id")
            .gui(false)
            .force_prompt(false)
            .status()
            .expect("failed to execute")
    );
}
