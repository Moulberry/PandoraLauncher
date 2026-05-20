// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR ISC

use crate::cc_builder::CcBuilder;
use crate::OutputLib::{Crypto, Ssl};
use crate::{
    allow_prebuilt_nasm, cargo_env, disable_jitter_entropy, effective_target, emit_warning,
    execute_command, get_crate_cflags, is_crt_static, is_no_asm, is_no_pregenerated_src,
    optional_env, optional_env_optional_crate_target, set_env, set_env_for_target, target_arch,
    target_env, target_os, test_clang_cl_command, test_nasm_command, use_prebuilt_nasm,
    OutputLibType,
};
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub(crate) struct CmakeBuilder {
    manifest_dir: PathBuf,
    out_dir: PathBuf,
    build_prefix: Option<String>,
    output_lib_type: OutputLibType,
}

fn test_prebuilt_nasm_script(script_path: &Path) -> bool {
    // Call with no args - both scripts will exit with error, but we only care if they can execute
    execute_command(script_path.as_os_str(), &[]).executed
}

fn find_cmake_command() -> Option<OsString> {
    if let Some(cmake) = optional_env_optional_crate_target("CMAKE") {
        emit_warning(format!("CMAKE environment variable set: {}", cmake.clone()));
        if execute_command(cmake.as_ref(), &["--version".as_ref()]).status {
            Some(cmake.into())
        } else {
            None
        }
    } else if execute_command("cmake3".as_ref(), &["--version".as_ref()]).status {
        Some("cmake3".into())
    } else if execute_command("cmake".as_ref(), &["--version".as_ref()]).status {
        Some("cmake".into())
    } else {
        None
    }
}

impl CmakeBuilder {
    pub(crate) fn new(
        manifest_dir: PathBuf,
        out_dir: PathBuf,
        build_prefix: Option<String>,
        output_lib_type: OutputLibType,
    ) -> Self {
        Self {
            manifest_dir,
            out_dir,
            build_prefix,
            output_lib_type,
        }
    }

    fn artifact_output_dir(&self) -> PathBuf {
        self.out_dir.join("build").join("artifacts")
    }

    fn get_cmake_config(&self) -> cmake::Config {
        cmake::Config::new(&self.manifest_dir)
    }

    fn apply_universal_build_options<'a>(
        &self,
        cmake_cfg: &'a mut cmake::Config,
    ) -> &'a cmake::Config {
        // Use the compiler options identified by CcBuilder
        let cc_builder = CcBuilder::new(
            self.manifest_dir.clone(),
            self.out_dir.clone(),
            self.build_prefix.clone(),
            self.output_lib_type,
        );
        let cc_build = cc::Build::new();
        let (is_like_msvc, build_options) =
            cc_builder.collect_universal_build_options(&cc_build, true);
        for option in &build_options {
            option.apply_cmake(cmake_cfg, is_like_msvc);
        }
        cmake_cfg
    }

    #[allow(clippy::too_many_lines)]
    fn prepare_cmake_build(&self) -> cmake::Config {
        let mut cmake_cfg = self.get_cmake_config();
        if let Some(generator) = optional_env_optional_crate_target("CMAKE_GENERATOR") {
            set_env("CMAKE_GENERATOR", generator);
        }

        if OutputLibType::default() == OutputLibType::Dynamic {
            cmake_cfg.define("BUILD_SHARED_LIBS", "1");
        } else {
            cmake_cfg.define("BUILD_SHARED_LIBS", "0");
        }

        if let Some(prefix) = &self.build_prefix {
            cmake_cfg.define("BORINGSSL_PREFIX", format!("{prefix}_"));
            let include_path = self.manifest_dir.join("generated-include");
            cmake_cfg.define(
                "BORINGSSL_PREFIX_HEADERS",
                include_path.display().to_string(),
            );
        }

        // Build flags that minimize our crate size.
        cmake_cfg.define("BUILD_TESTING", "OFF");
        cmake_cfg.define("BUILD_TOOL", "OFF");
        cmake_cfg.define("ENABLE_SOURCE_MODIFICATION", "OFF");
        if cfg!(feature = "ssl") {
            cmake_cfg.define("BUILD_LIBSSL", "ON");
        } else {
            cmake_cfg.define("BUILD_LIBSSL", "OFF");
        }
        if is_no_pregenerated_src() {
            // Go and Perl will be required.
            cmake_cfg.define("DISABLE_PERL", "OFF");
            cmake_cfg.define("DISABLE_GO", "OFF");
        } else {
            // Build flags that minimize our dependencies.
            cmake_cfg.define("DISABLE_PERL", "ON");
            cmake_cfg.define("DISABLE_GO", "ON");
        }
        if Some(true) == disable_jitter_entropy() {
            cmake_cfg.define("DISABLE_CPU_JITTER_ENTROPY", "ON");
        }

        if is_no_asm() {
            let opt_level = cargo_env("OPT_LEVEL");
            if opt_level == "0" {
                cmake_cfg.define("OPENSSL_NO_ASM", "1");
            } else {
                panic!("AWS_LC_SYS_NO_ASM only allowed for debug builds!")
            }
        }

        if cfg!(feature = "asan") {
            set_env_for_target("CC", "clang");
            set_env_for_target("CXX", "clang++");

            cmake_cfg.define("ASAN", "1");
        }

        if let Some(cflags) = get_crate_cflags() {
            set_env_for_target("CFLAGS", cflags);
        }

        if target_env() == "ohos" {
            Self::configure_open_harmony(&mut cmake_cfg);
            return cmake_cfg;
        }

        // cmake-rs has logic that strips Optimization/Debug options that are passed via CFLAGS:
        // https://github.com/rust-lang/cmake-rs/issues/240
        // This breaks build configurations that generate warnings when optimizations
        // are disabled.
        Self::preserve_cflag_optimization_flags(&mut cmake_cfg);

        if target_os() == "windows" {
            if use_prebuilt_nasm() {
                self.configure_prebuilt_nasm(&mut cmake_cfg);
            }
            if target_env().as_str() == "msvc" {
                let mut msvcrt = String::from_str("MultiThreaded").unwrap();
                if is_crt_static() {
                    cmake_cfg.static_crt(true);
                    // When using static CRT on Windows MSVC, ignore missing PDB file warnings
                    // The static CRT libraries reference PDB files from Microsoft's build servers
                    // which are not available.
                    println!("cargo:rustc-link-arg=/ignore:4099");
                } else {
                    msvcrt.push_str("DLL");
                }
                cmake_cfg.define("CMAKE_MSVC_RUNTIME_LIBRARY", msvcrt.as_str());
            }
        }

        // Allow environment to specify CMake toolchain.
        if let Some(toolchain) = optional_env_optional_crate_target("CMAKE_TOOLCHAIN_FILE") {
            set_env_for_target("CMAKE_TOOLCHAIN_FILE", toolchain);

            return cmake_cfg;
        }
        // We only consider compiler CFLAGS when no cmake toolchain is set
        self.apply_universal_build_options(&mut cmake_cfg);

        // See issue: https://github.com/aws/aws-lc-rs/issues/453
        if target_os() == "windows" {
            self.configure_windows(&mut cmake_cfg);
        }

        // If the build environment vendor is Apple
        #[cfg(target_vendor = "apple")]
        {
            if target_arch() == "aarch64" {
                cmake_cfg.define("CMAKE_OSX_ARCHITECTURES", "arm64");
                cmake_cfg.define("CMAKE_SYSTEM_PROCESSOR", "arm64");
            }
            if target_arch() == "x86_64" {
                cmake_cfg.define("CMAKE_OSX_ARCHITECTURES", "x86_64");
                cmake_cfg.define("CMAKE_SYSTEM_PROCESSOR", "x86_64");
            }
            if target_os().trim() == "ios" {
                cmake_cfg.define("CMAKE_SYSTEM_NAME", "iOS");
                if effective_target().ends_with("-ios-sim") || target_arch() == "x86_64" {
                    cmake_cfg.define("CMAKE_OSX_SYSROOT", "iphonesimulator");
                } else {
                    cmake_cfg.define("CMAKE_OSX_SYSROOT", "iphoneos");
                }
                cmake_cfg.define("CMAKE_THREAD_LIBS_INIT", "-lpthread");
            }
            if target_os().trim() == "macos" {
                cmake_cfg.define("CMAKE_SYSTEM_NAME", "Darwin");
                cmake_cfg.define("CMAKE_OSX_SYSROOT", "macosx");
            }
            if target_os().trim() == "tvos" {
                cmake_cfg.define("CMAKE_SYSTEM_NAME", "tvOS");
                if effective_target().ends_with("-tvos-sim") || target_arch() == "x86_64" {
                    cmake_cfg.define("CMAKE_OSX_SYSROOT", "appletvsimulator");
                } else {
                    cmake_cfg.define("CMAKE_OSX_SYSROOT", "appletvos");
                }
            }
        }

        if target_os() == "android" {
            self.configure_android(&mut cmake_cfg);
        }

        cmake_cfg
    }

    fn preserve_cflag_optimization_flags(cmake_cfg: &mut cmake::Config) {
        if let Some(cflags) = get_crate_cflags() {
            let split = cflags.split_whitespace();
            for arg in split {
                if arg.starts_with("-O") || arg.starts_with("/O") {
                    emit_warning(format!("Preserving optimization flag: {arg}"));
                    cmake_cfg.cflag(arg);
                }
            }
        }
    }

    #[allow(clippy::unused_self)]
    fn select_prebuilt_nasm_script(&self) -> PathBuf {
        let sh_script = self.manifest_dir.join("builder").join("prebuilt-nasm.sh");
        let bat_script = self.manifest_dir.join("builder").join("prebuilt-nasm.bat");

        // Test .sh first (more universal - works in MSYS2, WSL, native Unix)
        if test_prebuilt_nasm_script(&sh_script) {
            emit_warning("Selected prebuilt-nasm.sh (shell script can execute)");
            sh_script
        } else if test_prebuilt_nasm_script(&bat_script) {
            emit_warning(
                "Selected prebuilt-nasm.bat (batch script can execute, shell script cannot)",
            );
            bat_script
        } else {
            // Fallback to current logic if neither can execute
            let fallback_script = if cfg!(target_os = "windows") {
                bat_script
            } else {
                sh_script
            };
            emit_warning(
                format!(
                    "Neither script could be tested for execution, falling back to target-based selection: {}",
                    fallback_script.file_name().unwrap().to_str().unwrap()));
            fallback_script
        }
    }

    #[allow(clippy::unused_self)]
    fn configure_android(&self, _cmake_cfg: &mut cmake::Config) {
        // If we leave CMAKE_SYSTEM_PROCESSOR unset, then cmake-rs should handle properly setting
        // CMAKE_SYSTEM_NAME and CMAKE_SYSTEM_PROCESSOR:
        // https://github.com/rust-lang/cmake-rs/blob/b689783b5448966e810d515c798465f2e0ab56fd/src/lib.rs#L450-L499

        // Log relevant environment variables.
        if let Some(value) = optional_env_optional_crate_target("ANDROID_NDK_ROOT") {
            set_env("ANDROID_NDK_ROOT", value);
        } else {
            emit_warning("ANDROID_NDK_ROOT not set.");
        }
        if let Some(value) = optional_env_optional_crate_target("ANDROID_NDK") {
            set_env("ANDROID_NDK", value);
        } else {
            emit_warning("ANDROID_NDK not set.");
        }
        if let Some(value) = optional_env_optional_crate_target("ANDROID_STANDALONE_TOOLCHAIN") {
            set_env("ANDROID_STANDALONE_TOOLCHAIN", value);
        } else {
            emit_warning("ANDROID_STANDALONE_TOOLCHAIN not set.");
        }
    }

    #[allow(clippy::unused_self)]
    fn configure_windows(&self, cmake_cfg: &mut cmake::Config) {
        match (target_env().as_str(), target_arch().as_str()) {
            ("msvc", "aarch64") => {
                // If CMAKE_GENERATOR is either not set or not set to "Ninja"
                let cmake_generator = optional_env("CMAKE_GENERATOR");
                if cmake_generator.is_none() || cmake_generator.unwrap().to_lowercase() != "ninja" {
                    // The following is not supported by the Ninja generator
                    cmake_cfg.generator_toolset(format!(
                        "ClangCL{}",
                        if cfg!(target_arch = "x86_64") {
                            ",host=x64"
                        } else {
                            ""
                        }
                    ));
                    cmake_cfg.define("CMAKE_GENERATOR_PLATFORM", "ARM64");
                }
                cmake_cfg.define("CMAKE_SYSTEM_NAME", "Windows");
                cmake_cfg.define("CMAKE_SYSTEM_PROCESSOR", "ARM64");
            }
            ("msvc", _) => {
                // No-op
            }
            (_, arch) => {
                cmake_cfg.define("CMAKE_SYSTEM_NAME", "Windows");
                cmake_cfg.define("CMAKE_SYSTEM_PROCESSOR", arch);
            }
        }
    }

    fn configure_prebuilt_nasm(&self, cmake_cfg: &mut cmake::Config) {
        emit_warning("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
        emit_warning("!!!   Using pre-built NASM binaries   !!!");
        emit_warning("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");

        let script_path = self.select_prebuilt_nasm_script();
        let script_path = script_path.display().to_string();
        let script_path = script_path.replace('\\', "/");

        cmake_cfg.define("CMAKE_ASM_NASM_COMPILER", script_path.as_str());
        // Without the following definition, the build fails with a message similar to the one
        // reported here: https://gitlab.kitware.com/cmake/cmake/-/issues/19453
        // The variables below were found in the associated fix:
        // https://gitlab.kitware.com/cmake/cmake/-/merge_requests/4257/diffs
        cmake_cfg.define(
            "CMAKE_ASM_NASM_COMPILE_OPTIONS_MSVC_RUNTIME_LIBRARY_MultiThreaded",
            "",
        );
        cmake_cfg.define(
            "CMAKE_ASM_NASM_COMPILE_OPTIONS_MSVC_RUNTIME_LIBRARY_MultiThreadedDLL",
            "",
        );
        cmake_cfg.define(
            "CMAKE_ASM_NASM_COMPILE_OPTIONS_MSVC_RUNTIME_LIBRARY_MultiThreadedDebug",
            "",
        );
        cmake_cfg.define(
            "CMAKE_ASM_NASM_COMPILE_OPTIONS_MSVC_RUNTIME_LIBRARY_MultiThreadedDebugDLL",
            "",
        );
        cmake_cfg.define(
            "CMAKE_ASM_NASM_COMPILE_OPTIONS_MSVC_DEBUG_INFORMATION_FORMAT_ProgramDatabase",
            "",
        );
    }

    fn configure_open_harmony(cmake_cfg: &mut cmake::Config) {
        let mut cflags = vec!["-Wno-unused-command-line-argument"];
        let mut asmflags = vec![];

        // If a toolchain is not specified by the environment
        if optional_env_optional_crate_target("CMAKE_TOOLCHAIN_FILE").is_none() {
            if let Ok(ndk) = env::var("OHOS_NDK_HOME") {
                set_env_for_target(
                    "CMAKE_TOOLCHAIN_FILE",
                    format!("{ndk}/native/build/cmake/ohos.toolchain.cmake"),
                );
            } else if let Ok(sdk) = env::var("OHOS_SDK_NATIVE") {
                set_env_for_target(
                    "CMAKE_TOOLCHAIN_FILE",
                    format!("{sdk}/build/cmake/ohos.toolchain.cmake"),
                );
            } else {
                emit_warning(
                    "Neither OHOS_NDK_HOME nor OHOS_SDK_NATIVE are set! No toolchain found.",
                );
            }
        }

        match effective_target().as_str() {
            "aarch64-unknown-linux-ohos" => {
                cmake_cfg.define("OHOS_ARCH", "arm64-v8a");
            }
            "armv7-unknown-linux-ohos" => {
                const ARM7_FLAGS: [&str; 6] = [
                    "-march=armv7-a",
                    "-mfloat-abi=softfp",
                    "-mtune=generic-armv7-a",
                    "-mthumb",
                    "-mfpu=neon",
                    "-DHAVE_NEON",
                ];
                cflags.extend(ARM7_FLAGS);
                asmflags.extend(ARM7_FLAGS);
                cmake_cfg.define("OHOS_ARCH", "armeabi-v7a");
            }
            "x86_64-unknown-linux-ohos" => {
                const X86_64_FLAGS: [&str; 3] = ["-msse4.1", "-DHAVE_NEON_X86", "-DHAVE_NEON"];
                cflags.extend(X86_64_FLAGS);
                asmflags.extend(X86_64_FLAGS);
                cmake_cfg.define("OHOS_ARCH", "x86_64");
            }
            ohos_target => {
                emit_warning(format!("Target: {ohos_target} is not support yet!").as_str());
            }
        }
        cmake_cfg
            .cflag(cflags.join(" ").as_str())
            .cxxflag(cflags.join(" ").as_str())
            .asmflag(asmflags.join(" ").as_str());
    }

    fn build_library(&self) -> PathBuf {
        self.prepare_cmake_build()
            .configure_arg("--no-warn-unused-cli")
            .build()
    }
}

impl crate::Builder for CmakeBuilder {
    fn check_dependencies(&self) -> Result<(), String> {
        let mut missing_dependency = false;
        if target_os() == "windows" && target_arch() == "x86_64" {
            if is_no_asm() && Some(true) == allow_prebuilt_nasm() {
                eprintln!(
                    "Build environment has both `AWS_LC_SYS_PREBUILT_NASM` and `AWS_LC_SYS_NO_ASM` set.\
                Please remove one of these environment variables.
                See User Guide: https://aws.github.io/aws-lc-rs/index.html"
                );
            }
            if !is_no_asm() && !test_nasm_command() && !use_prebuilt_nasm() {
                eprintln!(
                    "Consider installing NASM or setting `AWS_LC_SYS_PREBUILT_NASM` in the build environment.\
                See User Guide: https://aws.github.io/aws-lc-rs/index.html"
                );
                eprintln!("Missing dependency: nasm");
                missing_dependency = true;
            }
            if target_arch() == "aarch64" && target_env() == "msvc" && !test_clang_cl_command() {
                eprintln!("Missing dependency: clang-cl");
                missing_dependency = true;
            }
        }
        if let Some(cmake_cmd) = find_cmake_command() {
            unsafe {
                env::set_var("CMAKE", cmake_cmd);
            }
        } else {
            eprintln!("Missing dependency: cmake");
            missing_dependency = true;
        }

        if missing_dependency {
            return Err("Required build dependency is missing. Halting build.".to_owned());
        }

        Ok(())
    }
    fn build(&self) -> Result<(), String> {
        self.build_library();

        println!(
            "cargo:rustc-link-search=native={}",
            self.artifact_output_dir().display()
        );

        println!(
            "cargo:rustc-link-lib={}={}",
            self.output_lib_type.rust_lib_type(),
            Crypto.libname(&self.build_prefix)
        );

        if cfg!(feature = "ssl") {
            println!(
                "cargo:rustc-link-lib={}={}",
                self.output_lib_type.rust_lib_type(),
                Ssl.libname(&self.build_prefix)
            );
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "CMake"
    }
}
