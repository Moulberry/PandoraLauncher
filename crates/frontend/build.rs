use std::process::Command;

fn main() {
    if let Ok(output) = Command::new("git").args(["rev-parse", "HEAD"]).output() {
        let mut git_hash = String::from_utf8(output.stdout).unwrap();
        git_hash.truncate(8);
        println!("cargo:rustc-env=GIT_REVISION={}", git_hash);
    }
}
