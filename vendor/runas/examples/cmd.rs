use runas;

fn shell() -> String {
    #[cfg(windows)]
    {
        "cmd".to_string()
    }
    #[cfg(unix)]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "bash".into())
    }
}

fn main() {
    println!("Starting a root shell:");
    println!(
        "Status: {}",
        runas::Command::new(shell())
            .status()
            .expect("failed to execute")
    );
}
