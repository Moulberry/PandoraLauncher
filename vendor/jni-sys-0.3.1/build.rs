fn main() {
    println!("cargo::rustc-check-cfg=cfg(jvm_linked)");

    if option_env!("_JNI_SYS_TEST").is_some() {
        let java_home = std::env::var("JAVA_HOME").unwrap();
        println!("cargo:rustc-link-search=native={}/lib/server", java_home);
        println!("cargo:rustc-link-search=native={}/lib", java_home);

        println!("cargo:rustc-cfg=jvm_linked");

        println!("cargo:rustc-link-lib=dylib=jvm");

        // NB: cargo won't update the system search path for dynamic libraries outside of
        // the target directory, so we need to set the environment variables for the test
        // process to find libjvm

        // set LD_LIBRARY_PATH (linux), DYLD_FALLBACK_LIBRARY_PATH (macOS) + PATH (windows) for tests
        println!("cargo:rustc-env=LD_LIBRARY_PATH={java_home}/lib/server");
        println!("cargo:rustc-env=DYLD_FALLBACK_LIBRARY_PATH={java_home}/lib/server");
        let path = std::env::var("PATH").unwrap_or_default();
        let java_home = std::path::PathBuf::from(java_home);
        if cfg!(windows) {
            let windows_jvm_path = java_home.join("bin").join("server");
            println!(
                "cargo:rustc-env=PATH={};{}",
                windows_jvm_path.display(),
                path
            );
        }
    }

    // Re-run the build script if JAVA_HOME or _JNI_SYS_TEST changes
    println!("cargo:rerun-if-env-changed=JAVA_HOME");
    println!("cargo:rerun-if-env-changed=_JNI_SYS_TEST");
}
