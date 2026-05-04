// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR ISC

// NOTE: This module is intended to produce an equivalent "libcrypto" static library to the one
// produced by the CMake. Changes to CMake relating to compiler checks and/or special build flags
// may require modifications to the logic in this module.

mod apple_aarch64;
mod apple_x86_64;
mod linux_aarch64;
mod linux_arm;
mod linux_ppc64le;
mod linux_x86;
mod linux_x86_64;
mod universal;
mod win_aarch64;
mod win_x86;
mod win_x86_64;

use crate::nasm_builder::NasmBuilder;
use crate::{
    cargo_env, disable_jitter_entropy, emit_warning, env_var_to_bool, execute_command,
    get_crate_cc, get_crate_cflags, get_crate_cxx, is_no_asm, out_dir, requested_c_std,
    set_env_for_target, target, target_arch, target_env, target_os, target_vendor,
    test_clang_cl_command, CStdRequested, OutputLibType,
};
use std::cell::Cell;
use std::collections::HashMap;
use std::path::PathBuf;

#[non_exhaustive]
#[derive(PartialEq, Eq)]
pub(crate) enum CompilerFeature {
    NeonSha3,
}

pub(crate) struct CcBuilder {
    manifest_dir: PathBuf,
    out_dir: PathBuf,
    build_prefix: Option<String>,
    output_lib_type: OutputLibType,
    compiler_features: Cell<Vec<CompilerFeature>>,
}

use std::fs;

fn identify_sources() -> Vec<&'static str> {
    let mut source_files: Vec<&'static str> = vec![];
    source_files.append(&mut Vec::from(universal::CRYPTO_LIBRARY));
    let mut target_specific_source_found = true;
    if target_os() == "windows" {
        if target_arch() == "x86_64" {
            source_files.append(&mut Vec::from(win_x86_64::CRYPTO_LIBRARY));
        } else if target_arch() == "aarch64" {
            source_files.append(&mut Vec::from(win_aarch64::CRYPTO_LIBRARY));
        } else if target_arch() == "x86" {
            source_files.append(&mut Vec::from(win_x86::CRYPTO_LIBRARY));
        } else {
            target_specific_source_found = false;
        }
    } else if target_vendor() == "apple" {
        if target_arch() == "x86_64" {
            source_files.append(&mut Vec::from(apple_x86_64::CRYPTO_LIBRARY));
        } else if target_arch() == "aarch64" {
            source_files.append(&mut Vec::from(apple_aarch64::CRYPTO_LIBRARY));
        } else {
            target_specific_source_found = false;
        }
    } else if target_arch() == "x86_64" {
        source_files.append(&mut Vec::from(linux_x86_64::CRYPTO_LIBRARY));
    } else if target_arch() == "aarch64" {
        source_files.append(&mut Vec::from(linux_aarch64::CRYPTO_LIBRARY));
    } else if target_arch() == "arm" {
        source_files.append(&mut Vec::from(linux_arm::CRYPTO_LIBRARY));
    } else if target_arch() == "x86" {
        source_files.append(&mut Vec::from(linux_x86::CRYPTO_LIBRARY));
    } else if target_arch() == "powerpc64" {
        source_files.append(&mut Vec::from(linux_ppc64le::CRYPTO_LIBRARY));
    } else {
        target_specific_source_found = false;
    }
    if !target_specific_source_found {
        emit_warning(format!(
            "No target-specific source found: {}-{}",
            target_os(),
            target_arch()
        ));
    }

    source_files
}

#[allow(clippy::upper_case_acronyms)]
pub(crate) enum BuildOption {
    STD(String),
    FLAG(String),
    DEFINE(String, String),
    INCLUDE(PathBuf),
}
impl BuildOption {
    fn std<T: ToString + ?Sized>(val: &T) -> Self {
        Self::STD(val.to_string())
    }
    fn flag<T: ToString + ?Sized>(val: &T) -> Self {
        Self::FLAG(val.to_string())
    }
    fn flag_if_supported<T: ToString + ?Sized>(cc_build: &cc::Build, flag: &T) -> Option<Self> {
        if let Ok(true) = cc_build.is_flag_supported(flag.to_string()) {
            Some(Self::FLAG(flag.to_string()))
        } else {
            None
        }
    }

    fn define<K: ToString + ?Sized, V: ToString + ?Sized>(key: &K, val: &V) -> Self {
        Self::DEFINE(key.to_string(), val.to_string())
    }

    fn include<P: Into<PathBuf>>(path: P) -> Self {
        Self::INCLUDE(path.into())
    }

    fn apply_cc<'a>(&self, cc_build: &'a mut cc::Build) -> &'a mut cc::Build {
        match self {
            BuildOption::STD(val) => cc_build.std(val),
            BuildOption::FLAG(val) => cc_build.flag(val),
            BuildOption::DEFINE(key, val) => cc_build.define(key, Some(val.as_str())),
            BuildOption::INCLUDE(path) => cc_build.include(path.as_path()),
        }
    }

    pub(crate) fn apply_cmake<'a>(
        &self,
        cmake_cfg: &'a mut cmake::Config,
        is_like_msvc: bool,
    ) -> &'a mut cmake::Config {
        if is_like_msvc {
            match self {
                BuildOption::STD(val) => cmake_cfg.define(
                    "CMAKE_C_STANDARD",
                    val.to_ascii_lowercase().strip_prefix('c').unwrap_or("11"),
                ),
                BuildOption::FLAG(val) => cmake_cfg.cflag(val),
                BuildOption::DEFINE(key, val) => cmake_cfg.cflag(format!("/D{key}={val}")),
                BuildOption::INCLUDE(path) => cmake_cfg.cflag(format!("/I{}", path.display())),
            }
        } else {
            match self {
                BuildOption::STD(val) => cmake_cfg.define(
                    "CMAKE_C_STANDARD",
                    val.to_ascii_lowercase().strip_prefix('c').unwrap_or("11"),
                ),
                BuildOption::FLAG(val) => cmake_cfg.cflag(val),
                BuildOption::DEFINE(key, val) => cmake_cfg.cflag(format!("-D{key}={val}")),
                BuildOption::INCLUDE(path) => cmake_cfg.cflag(format!("-I{}", path.display())),
            }
        }
    }

    pub(crate) fn apply_nasm<'a>(&self, nasm_builder: &'a mut NasmBuilder) -> &'a mut NasmBuilder {
        match self {
            BuildOption::FLAG(val) => nasm_builder.flag(val),
            BuildOption::DEFINE(key, val) => nasm_builder.define(key, Some(val.as_str())),
            BuildOption::INCLUDE(path) => nasm_builder.include(path.as_path()),
            BuildOption::STD(_) => nasm_builder, // STD ignored for NASM
        }
    }
}

impl CcBuilder {
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
            compiler_features: Cell::new(vec![]),
        }
    }

    pub(crate) fn collect_universal_build_options(
        &self,
        cc_build: &cc::Build,
        do_quote_paths: bool,
    ) -> (bool, Vec<BuildOption>) {
        let mut build_options: Vec<BuildOption> = Vec::new();

        let compiler_is_msvc = {
            let compiler = cc_build.get_compiler();
            !compiler.is_like_gnu() && !compiler.is_like_clang()
        };

        match requested_c_std() {
            CStdRequested::C99 => {
                build_options.push(BuildOption::std("c99"));
            }
            CStdRequested::C11 => {
                build_options.push(BuildOption::std("c11"));
            }
            CStdRequested::None => {
                if self.compiler_check("c11", Vec::<String>::new()) {
                    build_options.push(BuildOption::std("c11"));
                } else {
                    build_options.push(BuildOption::std("c99"));
                }
            }
        }

        if let Some(cc) = get_crate_cc() {
            set_env_for_target("CC", &cc);
        }
        if let Some(cxx) = get_crate_cxx() {
            set_env_for_target("CXX", &cxx);
        }

        if target_arch() == "x86" && !compiler_is_msvc {
            if let Some(option) = BuildOption::flag_if_supported(cc_build, "-msse2") {
                build_options.push(option);
            }
        }

        if target_os() == "macos" || target_os() == "darwin" {
            // Certain MacOS system headers are guarded by _POSIX_C_SOURCE and _DARWIN_C_SOURCE
            build_options.push(BuildOption::define("_DARWIN_C_SOURCE", "1"));
        }

        let opt_level = cargo_env("OPT_LEVEL");
        match opt_level.as_str() {
            "0" | "1" | "2" => {
                if is_no_asm() {
                    emit_warning("AWS_LC_SYS_NO_ASM found. Disabling assembly code usage.");
                    build_options.push(BuildOption::define("OPENSSL_NO_ASM", "1"));
                }
            }
            _ => {
                assert!(
                    !is_no_asm(),
                    "AWS_LC_SYS_NO_ASM only allowed for debug builds!"
                );
                if !compiler_is_msvc {
                    let path_str = if do_quote_paths {
                        format!("\"{}\"", self.manifest_dir.display())
                    } else {
                        format!("{}", self.manifest_dir.display())
                    };

                    let flag = format!("-ffile-prefix-map={path_str}=");
                    if let Ok(true) = cc_build.is_flag_supported(&flag) {
                        emit_warning(format!("Using flag: {}", &flag));
                        build_options.push(BuildOption::flag(&flag));
                    } else {
                        emit_warning("NOTICE: Build environment source paths might be visible in release binary.");
                        let flag = format!("-fdebug-prefix-map={path_str}=");
                        if let Ok(true) = cc_build.is_flag_supported(&flag) {
                            emit_warning(format!("Using flag: {}", &flag));
                            build_options.push(BuildOption::flag(&flag));
                        }
                    }
                }
            }
        }

        if target_os() == "macos" {
            // This compiler error has only been seen on MacOS x86_64:
            // ```
            // clang: error: overriding '-mmacosx-version-min=13.7' option with '--target=x86_64-apple-macosx14.2' [-Werror,-Woverriding-t-option]
            // ```
            if let Some(option) =
                BuildOption::flag_if_supported(cc_build, "-Wno-overriding-t-option")
            {
                build_options.push(option);
            }
            if let Some(option) = BuildOption::flag_if_supported(cc_build, "-Wno-overriding-option")
            {
                build_options.push(option);
            }
        }
        (compiler_is_msvc, build_options)
    }

    pub fn collect_cc_only_build_options(&self, cc_build: &cc::Build) -> Vec<BuildOption> {
        let mut build_options: Vec<BuildOption> = Vec::new();
        let is_like_msvc = {
            let compiler = cc_build.get_compiler();
            !compiler.is_like_gnu() && !compiler.is_like_clang()
        };
        if !is_like_msvc {
            build_options.push(BuildOption::flag("-Wno-unused-parameter"));
            build_options.push(BuildOption::flag("-pthread"));
            if target_os() == "linux" {
                build_options.push(BuildOption::define("_XOPEN_SOURCE", "700"));
            } else if target_vendor() != "apple" {
                // Needed by illumos
                build_options.push(BuildOption::define("__EXTENSIONS__", "1"));
            }
        }
        if Some(true) == disable_jitter_entropy() {
            build_options.push(BuildOption::define("DISABLE_CPU_JITTER_ENTROPY", "1"));
        }
        self.add_includes(&mut build_options);
        self.add_defines(&mut build_options, is_like_msvc);

        build_options
    }

    fn add_includes(&self, build_options: &mut Vec<BuildOption>) {
        // The order of includes matters
        if let Some(prefix) = &self.build_prefix {
            build_options.push(BuildOption::define("BORINGSSL_IMPLEMENTATION", "1"));
            build_options.push(BuildOption::define("BORINGSSL_PREFIX", prefix.as_str()));
            build_options.push(BuildOption::include(
                self.manifest_dir.join("generated-include"),
            ));
        }
        build_options.push(BuildOption::include(self.manifest_dir.join("include")));
        build_options.push(BuildOption::include(
            self.manifest_dir.join("aws-lc").join("include"),
        ));
        build_options.push(BuildOption::include(
            self.manifest_dir
                .join("aws-lc")
                .join("third_party")
                .join("s2n-bignum")
                .join("include"),
        ));
        build_options.push(BuildOption::include(
            self.manifest_dir
                .join("aws-lc")
                .join("third_party")
                .join("s2n-bignum")
                .join("s2n-bignum-imported")
                .join("include"),
        ));

        if Some(true) != disable_jitter_entropy() {
            build_options.push(BuildOption::include(
                self.manifest_dir
                    .join("aws-lc")
                    .join("third_party")
                    .join("jitterentropy")
                    .join("jitterentropy-library"),
            ));
        }
    }

    pub fn create_builder(&self) -> cc::Build {
        let mut cc_build = cc::Build::new();
        let build_options = self.collect_cc_only_build_options(&cc_build);
        for option in build_options {
            option.apply_cc(&mut cc_build);
        }
        cc_build
    }

    pub fn prepare_builder(&self) -> cc::Build {
        if let Some(cflags) = get_crate_cflags() {
            set_env_for_target("CFLAGS", cflags);
        }

        let mut cc_build = self.create_builder();
        let (_, build_options) = self.collect_universal_build_options(&cc_build, false);
        for option in build_options {
            option.apply_cc(&mut cc_build);
        }

        // Add --noexecstack flag for assembly files to prevent executable stacks
        // This matches the behavior of AWS-LC's CMake build which uses -Wa,--noexecstack
        // See: https://github.com/aws/aws-lc/blob/main/crypto/CMakeLists.txt#L77
        if target_os() == "linux" || target_os().ends_with("bsd") {
            cc_build.asm_flag("-Wa,--noexecstack");
        }

        cc_build
    }

    #[allow(clippy::zero_sized_map_values)]
    fn build_s2n_bignum_source_feature_map() -> HashMap<String, CompilerFeature> {
        let mut source_feature_map: HashMap<String, CompilerFeature> = HashMap::new();
        source_feature_map.insert("sha3_keccak_f1600_alt.S".into(), CompilerFeature::NeonSha3);
        source_feature_map.insert("sha3_keccak2_f1600.S".into(), CompilerFeature::NeonSha3);
        source_feature_map.insert(
            "sha3_keccak4_f1600_alt2.S".into(),
            CompilerFeature::NeonSha3,
        );
        source_feature_map
    }

    #[allow(clippy::unused_self)]
    fn add_defines(&self, build_options: &mut Vec<BuildOption>, is_like_msvc: bool) {
        // WIN32_LEAN_AND_MEAN and NOMINMAX are needed for all Windows targets to avoid
        // header type definition errors, no matter the compiler. This matches the behavior
        // in aws-lc/CMakeLists.txt, which defines these for all WIN32 targets
        if target_os() == "windows" {
            build_options.push(BuildOption::define("WIN32_LEAN_AND_MEAN", ""));
            build_options.push(BuildOption::define("NOMINMAX", ""));
        }

        if is_like_msvc {
            build_options.push(BuildOption::define("_HAS_EXCEPTIONS", "0"));
            build_options.push(BuildOption::define("_CRT_SECURE_NO_WARNINGS", "0"));
            build_options.push(BuildOption::define(
                "_STL_EXTRA_DISABLED_WARNINGS",
                "4774 4987",
            ));

            if target().ends_with("-win7-windows-msvc") {
                // 0x0601 is the value of `_WIN32_WINNT_WIN7`
                build_options.push(BuildOption::define("_WIN32_WINNT", "0x0601"));
                emit_warning(format!(
                    "Setting _WIN32_WINNT to _WIN32_WINNT_WIN7 for {} target",
                    target()
                ));
            }
        }
    }

    fn prepare_jitter_entropy_builder(&self, is_like_msvc: bool) -> cc::Build {
        // See: https://github.com/aws/aws-lc/blob/2294510cd0ecb2d5946461e3dbb038363b7b94cb/third_party/jitterentropy/CMakeLists.txt#L19-L35
        let mut build_options: Vec<BuildOption> = Vec::new();
        self.add_includes(&mut build_options);
        self.add_defines(&mut build_options, is_like_msvc);

        let mut je_builder = cc::Build::new();
        for option in build_options {
            option.apply_cc(&mut je_builder);
        }

        if let Some(original_cflags) = get_crate_cflags() {
            let mut new_cflags = original_cflags.clone();
            if is_like_msvc {
                new_cflags.push_str(" -Od");
            } else {
                new_cflags.push_str(" -O0 -Wp,-U_FORTIFY_SOURCE");
            }
            set_env_for_target("CFLAGS", &new_cflags);
            // cc-rs currently prioritizes flags provided by CFLAGS over the flags provided by the build script.
            // The environment variables used by the compiler are set when `get_compiler` is called.
            je_builder.get_compiler();
            set_env_for_target("CFLAGS", &original_cflags);
        }

        je_builder.define("AWSLC", "1");
        if target_os() == "macos" || target_os() == "darwin" {
            // Certain MacOS system headers are guarded by _POSIX_C_SOURCE and _DARWIN_C_SOURCE
            je_builder.define("_DARWIN_C_SOURCE", "1");
        }
        // Only enable PIC on non-Windows targets. Windows doesn't support -fPIC.
        if target_os() != "windows" {
            je_builder.pic(true);
        }
        if is_like_msvc {
            je_builder.flag("-Od").flag("-W4").flag("-DYNAMICBASE");
        } else {
            je_builder
                .flag("-fwrapv")
                .flag("--param")
                .flag("ssp-buffer-size=4")
                .flag("-fvisibility=hidden")
                .flag("-Wcast-align")
                .flag("-Wmissing-field-initializers")
                .flag("-Wshadow")
                .flag("-Wswitch-enum")
                .flag("-Wextra")
                .flag("-Wall")
                .flag("-pedantic")
                // Compilation will fail if optimizations are enabled.
                .flag("-O0")
                .flag("-fwrapv")
                .flag("-Wconversion");
        }
        je_builder
    }

    fn add_all_files(&self, sources: &[&'static str], cc_build: &mut cc::Build) {
        let compiler = cc_build.get_compiler();

        let force_include_option = if compiler.is_like_msvc() {
            "/FI"
        } else {
            "--include="
        };
        // s2n-bignum is compiled separately due to needing extra flags
        let mut s2n_bignum_builder = cc_build.clone();
        s2n_bignum_builder.flag(format!(
            "{}{}",
            force_include_option,
            self.manifest_dir
                .join("generated-include")
                .join("openssl")
                .join("boringssl_prefix_symbols_asm.h")
                .display()
        ));
        s2n_bignum_builder.define("S2N_BN_HIDE_SYMBOLS", "1");

        // CPU Jitter Entropy is compiled separately due to needing specific flags
        let mut jitter_entropy_builder =
            self.prepare_jitter_entropy_builder(compiler.is_like_msvc());
        jitter_entropy_builder.flag(format!(
            "{}{}",
            force_include_option,
            self.manifest_dir
                .join("generated-include")
                .join("openssl")
                .join("boringssl_prefix_symbols.h")
                .display()
        ));

        let mut build_options = vec![];
        self.add_includes(&mut build_options);
        let mut nasm_builder = NasmBuilder::new(self.manifest_dir.clone(), self.out_dir.clone());

        for option in &build_options {
            option.apply_nasm(&mut nasm_builder);
        }

        let s2n_bignum_source_feature_map = Self::build_s2n_bignum_source_feature_map();
        let compiler_features = self.compiler_features.take();
        for source in sources {
            let source_path = self.manifest_dir.join("aws-lc").join(source);
            let is_s2n_bignum = std::path::Path::new(source).starts_with("third_party/s2n-bignum");
            let is_jitter_entropy =
                std::path::Path::new(source).starts_with("third_party/jitterentropy");

            if !source_path.is_file() {
                emit_warning(format!("Not a file: {:?}", source_path.as_os_str()));
                continue;
            }
            if is_s2n_bignum {
                let filename: String = source_path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string();

                if let Some(compiler_feature) = s2n_bignum_source_feature_map.get(&filename) {
                    if compiler_features.contains(compiler_feature) {
                        s2n_bignum_builder.file(source_path);
                    } else {
                        emit_warning(format!(
                            "Skipping due to missing compiler features: {:?}",
                            source_path.as_os_str()
                        ));
                    }
                } else {
                    s2n_bignum_builder.file(source_path);
                }
            } else if is_jitter_entropy {
                // Only compile if not disabled.
                if Some(true) != disable_jitter_entropy() {
                    jitter_entropy_builder.file(source_path);
                }
            } else if source_path.extension() == Some("asm".as_ref()) {
                nasm_builder.file(source_path);
            } else {
                cc_build.file(source_path);
            }
        }
        self.compiler_features.set(compiler_features);
        let s2n_bignum_object_files = s2n_bignum_builder.compile_intermediates();
        for object in s2n_bignum_object_files {
            cc_build.object(object);
        }
        if Some(true) != disable_jitter_entropy() {
            let jitter_entropy_object_files = jitter_entropy_builder.compile_intermediates();
            for object in jitter_entropy_object_files {
                cc_build.object(object);
            }
        }
        let nasm_object_files = nasm_builder.compile_intermediates();
        for object in nasm_object_files {
            cc_build.object(object);
        }
    }

    fn build_library(&self, sources: &[&'static str]) {
        let mut cc_build = self.prepare_builder();
        self.run_compiler_checks(&mut cc_build);

        self.add_all_files(sources, &mut cc_build);
        if let Some(prefix) = &self.build_prefix {
            cc_build.compile(format!("{}_crypto", prefix.as_str()).as_str());
        } else {
            cc_build.compile("crypto");
        }
    }

    // This performs basic checks of compiler capabilities and sets an appropriate flag on success.
    // This should be kept in alignment with the checks performed by AWS-LC's CMake build.
    // See: https://github.com/search?q=repo%3Aaws%2Faws-lc%20check_compiler&type=code
    fn compiler_check<T, S>(&self, basename: &str, extra_flags: T) -> bool
    where
        T: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut ret_val = false;
        let output_dir = self.out_dir.join(format!("out-{basename}"));
        let source_file = self
            .manifest_dir
            .join("aws-lc")
            .join("tests")
            .join("compiler_features_tests")
            .join(format!("{basename}.c"));
        if !source_file.exists() {
            emit_warning("######");
            emit_warning("###### WARNING: MISSING GIT SUBMODULE ######");
            emit_warning(format!(
                "  -- Did you initialize the repo's git submodules? Unable to find source file: {}.",
                source_file.display()
            ));
            emit_warning("  -- run 'git submodule update --init --recursive' to initialize.");
            emit_warning("######");
            emit_warning("######");
        }
        let mut cc_build = cc::Build::default();
        cc_build
            .file(source_file)
            .warnings_into_errors(true)
            .out_dir(&output_dir);
        for flag in extra_flags {
            let flag = flag.as_ref();
            cc_build.flag(flag);
        }

        let compiler = cc_build.get_compiler();
        if compiler.is_like_gnu() || compiler.is_like_clang() {
            cc_build.flag("-Wno-unused-parameter");
        }
        let result = cc_build.try_compile_intermediates();

        if result.is_ok() {
            ret_val = true;
        }
        if fs::remove_dir_all(&output_dir).is_err() {
            emit_warning(format!("Failed to remove {}", output_dir.display()));
        }
        emit_warning(format!(
            "Compilation of '{basename}.c' {} - {:?}.",
            if ret_val { "succeeded" } else { "failed" },
            &result
        ));
        ret_val
    }

    // This checks whether the compiler contains a critical bug that causes `memcmp` to erroneously
    // consider two regions of memory to be equal when they're not.
    // See GCC bug report: https://gcc.gnu.org/bugzilla/show_bug.cgi?id=95189
    // This should be kept in alignment with the same check performed by the CMake build.
    // See: https://github.com/search?q=repo%3Aaws%2Faws-lc%20check_run&type=code
    fn memcmp_check(&self) {
        // This check compiles, links, and executes a test program. When cross-compiling
        // (HOST != TARGET), we cannot execute the resulting binary, so we skip this check.
        // This also avoids linker configuration issues with cross-compilation toolchains
        // (e.g., cross-rs Darwin toolchains that set invalid -fuse-ld= flags in CFLAGS).
        if cargo_env("HOST") != target() {
            return;
        }

        let basename = "memcmp_invalid_stripped_check";
        let exec_path = out_dir().join(basename);
        let memcmp_build = cc::Build::default();
        let memcmp_compiler = memcmp_build.get_compiler();
        if !memcmp_compiler.is_like_clang() && !memcmp_compiler.is_like_gnu() {
            // The logic below assumes a Clang or GCC compiler is in use
            return;
        }
        let mut memcmp_compile_args = Vec::from(memcmp_compiler.args());

        // This check invokes the compiled executable and hence needs to link
        // it. CMake handles this via LDFLAGS but `cc` doesn't. In setups with
        // custom linker setups this could lead to a mismatch between the
        // expected and the actually used linker. Explicitly respecting LDFLAGS
        // here brings us back to parity with CMake.
        if let Ok(ldflags) = std::env::var("LDFLAGS") {
            for flag in ldflags.split_whitespace() {
                memcmp_compile_args.push(flag.into());
            }
        }

        memcmp_compile_args.push(
            self.manifest_dir
                .join("aws-lc")
                .join("tests")
                .join("compiler_features_tests")
                .join(format!("{basename}.c"))
                .into_os_string(),
        );
        memcmp_compile_args.push("-Wno-unused-parameter".into());
        memcmp_compile_args.push("-o".into());
        memcmp_compile_args.push(exec_path.clone().into_os_string());
        let memcmp_args: Vec<_> = memcmp_compile_args
            .iter()
            .map(std::ffi::OsString::as_os_str)
            .collect();
        let memcmp_compile_result =
            execute_command(memcmp_compiler.path().as_os_str(), memcmp_args.as_slice());
        assert!(
            memcmp_compile_result.status,
            "COMPILER: {}\
            ARGS: {:?}\
            EXECUTED: {}\
            ERROR: {}\
            OUTPUT: {}\
            Failed to compile {basename}
            ",
            memcmp_compiler.path().display(),
            memcmp_args.as_slice(),
            memcmp_compile_result.executed,
            memcmp_compile_result.stderr,
            memcmp_compile_result.stdout
        );

        let result = execute_command(exec_path.as_os_str(), &[]);
        assert!(
            result.status,
            "### COMPILER BUG DETECTED ###\nYour compiler ({}) is not supported due to a memcmp related bug reported in \
            https://gcc.gnu.org/bugzilla/show_bug.cgi?id=95189. \
            We strongly recommend against using this compiler. \n\
            EXECUTED: {}\n\
            ERROR: {}\n\
            OUTPUT: {}\n\
            ",
            memcmp_compiler.path().display(),
            memcmp_compile_result.executed,
            memcmp_compile_result.stderr,
            memcmp_compile_result.stdout
        );
        let _ = fs::remove_file(exec_path);
    }
    fn run_compiler_checks(&self, cc_build: &mut cc::Build) {
        if self.compiler_check("stdalign_check", Vec::<&'static str>::new()) {
            cc_build.define("AWS_LC_STDALIGN_AVAILABLE", Some("1"));
        }
        if self.compiler_check("builtin_swap_check", Vec::<&'static str>::new()) {
            cc_build.define("AWS_LC_BUILTIN_SWAP_SUPPORTED", Some("1"));
        }
        if target_arch() == "aarch64"
            && self.compiler_check("neon_sha3_check", vec!["-march=armv8.4-a+sha3"])
        {
            let mut compiler_features = self.compiler_features.take();
            compiler_features.push(CompilerFeature::NeonSha3);
            self.compiler_features.set(compiler_features);
            cc_build.define("MY_ASSEMBLER_SUPPORTS_NEON_SHA3_EXTENSION", Some("1"));
        }
        if target_os() == "linux" || target_os() == "android" {
            if self.compiler_check("linux_random_h", Vec::<&'static str>::new()) {
                cc_build.define("HAVE_LINUX_RANDOM_H", Some("1"));
            } else if self.compiler_check("linux_random_h", vec!["-DDEFINE_U32"]) {
                cc_build.define("HAVE_LINUX_RANDOM_H", Some("1"));
                cc_build.define("AWS_LC_URANDOM_NEEDS_U32", Some("1"));
            }
        }
        self.memcmp_check();
    }
}

impl crate::Builder for CcBuilder {
    fn check_dependencies(&self) -> Result<(), String> {
        if OutputLibType::Dynamic == self.output_lib_type {
            // https://github.com/rust-lang/cc-rs/issues/594
            return Err("CcBuilder only supports static builds".to_string());
        }

        if target_env() == "ohos" {
            return Err("OpenHarmony targets must be built with CMake.".to_string());
        }

        if Some(true) == env_var_to_bool("CARGO_FEATURE_SSL") {
            return Err("cc_builder for libssl not supported".to_string());
        }

        Ok(())
    }

    fn build(&self) -> Result<(), String> {
        if target_os() == "windows"
            && target_arch() == "aarch64"
            && target_env() == "msvc"
            && get_crate_cc().is_none()
            && test_clang_cl_command()
        {
            set_env_for_target("CC", "clang-cl");
        }

        println!("cargo:root={}", self.out_dir.display());
        let sources = crate::cc_builder::identify_sources();
        self.build_library(sources.as_slice());
        Ok(())
    }

    fn name(&self) -> &'static str {
        "CC"
    }
}
