fn main() {
    // On NetBSD and OpenBSD, inotify is provided by a userspace library
    // (libinotify) rather than the kernel. Link against it when building for
    // those targets.
    //
    // FreeBSD ships native inotify support with libc wrappers in libc.so.7, so
    // the extern declarations resolve against libc directly there.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();

    let needs_libinotify = match target_os.as_str() {
        "netbsd" | "openbsd" => true,
        _ => false,
    };

    if needs_libinotify {
        // Use pkg-config to find the library search path if available,
        // otherwise fall back to /usr/local/lib (the conventional BSD prefix).
        let lib_dir = std::process::Command::new("pkg-config")
            .args(["--variable=libdir", "libinotify"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_owned())
            .unwrap_or_else(|| "/usr/local/lib".to_owned());

        println!("cargo:rustc-link-search=native={lib_dir}");
        println!("cargo:rustc-link-lib=inotify");
    }
}
