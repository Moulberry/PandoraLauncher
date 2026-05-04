// Copyright (c) 2022, Google Inc.
// SPDX-License-Identifier: ISC
// Modifications copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR ISC

// Needed until MSRV >= 1.70
#![allow(clippy::unnecessary_map_or)]
#![allow(clippy::ref_option)]
// Clippy can only be run on nightly toolchain
#![cfg_attr(clippy, feature(custom_inner_attributes))]
#![cfg_attr(clippy, clippy::msrv = "1.77")]

use std::ffi::{OsStr, OsString};
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fmt};

use cc_builder::CcBuilder;
use cmake_builder::CmakeBuilder;

// These should generally match those found in aws-lc/include/openssl/opensslconf.h
const OSSL_CONF_DEFINES: &[&str] = &[
    "OPENSSL_NO_ASYNC",
    "OPENSSL_NO_BF",
    "OPENSSL_NO_BLAKE2",
    "OPENSSL_NO_BUF_FREELISTS",
    "OPENSSL_NO_CAMELLIA",
    "OPENSSL_NO_CAPIENG",
    "OPENSSL_NO_CAST",
    "OPENSSL_NO_CMS",
    "OPENSSL_NO_COMP",
    "OPENSSL_NO_CRYPTO_MDEBUG",
    "OPENSSL_NO_CT",
    "OPENSSL_NO_DANE",
    "OPENSSL_NO_DEPRECATED",
    "OPENSSL_NO_DGRAM",
    "OPENSSL_NO_DYNAMIC_ENGINE",
    "OPENSSL_NO_EC_NISTP_64_GCC_128",
    "OPENSSL_NO_EC2M",
    "OPENSSL_NO_EGD",
    "OPENSSL_NO_ENGINE",
    "OPENSSL_NO_GMP",
    "OPENSSL_NO_GOST",
    "OPENSSL_NO_HEARTBEATS",
    "OPENSSL_NO_HW",
    "OPENSSL_NO_IDEA",
    "OPENSSL_NO_JPAKE",
    "OPENSSL_NO_KRB5",
    "OPENSSL_NO_MD2",
    "OPENSSL_NO_MDC2",
    "OPENSSL_NO_OCB",
    "OPENSSL_NO_RC2",
    "OPENSSL_NO_RC5",
    "OPENSSL_NO_RFC3779",
    "OPENSSL_NO_RIPEMD",
    "OPENSSL_NO_RMD160",
    "OPENSSL_NO_SCTP",
    "OPENSSL_NO_SEED",
    "OPENSSL_NO_SM2",
    "OPENSSL_NO_SM3",
    "OPENSSL_NO_SM4",
    "OPENSSL_NO_SRP",
    "OPENSSL_NO_SSL_TRACE",
    "OPENSSL_NO_SSL2",
    "OPENSSL_NO_SSL3",
    "OPENSSL_NO_SSL3_METHOD",
    "OPENSSL_NO_STATIC_ENGINE",
    "OPENSSL_NO_STORE",
    "OPENSSL_NO_TS",
    "OPENSSL_NO_WHIRLPOOL",
];

mod cc_builder;
mod cmake_builder;
mod nasm_builder;
#[cfg(feature = "bindgen")]
mod sys_bindgen;

pub(crate) fn get_aws_lc_include_path(manifest_dir: &Path) -> PathBuf {
    manifest_dir.join("aws-lc").join("include")
}

pub(crate) fn get_rust_include_path(manifest_dir: &Path) -> PathBuf {
    manifest_dir.join("include")
}

pub(crate) fn get_generated_include_path(manifest_dir: &Path) -> PathBuf {
    manifest_dir.join("generated-include")
}

pub(crate) fn get_aws_lc_sys_includes_path() -> Option<Vec<PathBuf>> {
    optional_env_crate_target("INCLUDES").map(|v| std::env::split_paths(&v).collect())
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputLib {
    Crypto,
    Ssl,
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputLibType {
    Static,
    Dynamic,
}

fn cargo_env<N: AsRef<str>>(name: N) -> String {
    let name = name.as_ref();
    env::var(name).unwrap_or_else(|_| panic!("missing env var {name:?}"))
}

// "CFLAGS" =>
// "AWS_LC_SYS_CFLAGS_aarch64_unknown_linux_gnu" OR "AWS_LC_SYS_CFLAGS"
//    OR "CFLAGS_aarch64_unknown_linux_gnu" OR "CFLAGS"
fn optional_env_optional_crate_target<N: AsRef<str>>(name: N) -> Option<String> {
    let name = name.as_ref();
    optional_env_crate_target(name).or(optional_env_target(name))
}

// "EFFECTIVE_TARGET" => "AWS_LC_SYS_EFFECTIVE_TARGET_aarch64_unknown_linux_gnu" + "AWS_LC_SYS_EFFECTIVE_TARGET"
fn optional_env_crate_target<N: AsRef<str>>(name: N) -> Option<String> {
    let name = name.as_ref();
    let crate_name = crate_name().to_uppercase().replace('-', "_");
    let target_name = target().to_lowercase().replace('-', "_");
    let name_for_crate = format!("{crate_name}_{name}");
    let name_for_crate_target = format!("{crate_name}_{name}_{target_name}");
    optional_env(name_for_crate_target).or(optional_env(name_for_crate))
}

fn optional_env_target<N: AsRef<str>>(name: N) -> Option<String> {
    let name = name.as_ref();
    let target_name = target().to_lowercase().replace('-', "_");
    let name_for_target = format!("{}_{}", &name, target_name);
    optional_env(name_for_target).or(optional_env(name))
}

fn optional_env<N: AsRef<str>>(name: N) -> Option<String> {
    let name = name.as_ref();
    println!("cargo:rerun-if-env-changed={name}");
    if let Ok(value) = env::var(name) {
        emit_warning(format!("Environment Variable found '{name}': '{}'", &value));
        return Some(value);
    }
    None
}

fn set_env_for_target<K, V>(env_var: K, value: V)
where
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    let target = target().to_lowercase();
    let target = target.replace('-', "_");
    let env_var = format!("{}_{target}", env_var.as_ref().to_str().unwrap());
    #[allow(unused_unsafe)]
    unsafe {
        env::set_var(&env_var, &value);
    }
    emit_warning(format!(
        "Setting {env_var}: {}",
        value.as_ref().to_str().unwrap()
    ));
}

fn set_env<K, V>(env_var: K, value: V)
where
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    #[allow(unused_unsafe)]
    unsafe {
        env::set_var(&env_var, &value);
    }
    emit_warning(format!(
        "Setting {}: {}",
        env_var.as_ref().to_str().unwrap(),
        value.as_ref().to_str().unwrap()
    ));
}

fn env_var_to_bool(name: &str) -> Option<bool> {
    if let Some(value) = optional_env(name) {
        return parse_to_bool(&value);
    }
    None
}

fn env_crate_var_to_bool(name: &str) -> Option<bool> {
    if let Some(value) = optional_env_crate_target(name) {
        return parse_to_bool(&value);
    }
    None
}

fn parse_to_bool(env_var_value: &str) -> Option<bool> {
    let env_var_value = env_var_value.to_lowercase();
    if env_var_value.starts_with('0')
        || env_var_value.starts_with('n')
        || env_var_value.starts_with("off")
        || env_var_value.starts_with('f')
    {
        emit_warning(format!("Value: {} is false.", &env_var_value));
        return Some(false);
    }
    if env_var_value.starts_with(|c: char| c.is_ascii_digit())
        || env_var_value.starts_with('y')
        || env_var_value.starts_with("on")
        || env_var_value.starts_with('t')
    {
        emit_warning(format!("Value: {} is true.", &env_var_value));
        return Some(true);
    }
    None
}

impl Default for OutputLibType {
    fn default() -> Self {
        if Some(false) == env_crate_var_to_bool("STATIC") {
            // Only dynamic if the value is set and is a "negative" value
            OutputLibType::Dynamic
        } else {
            OutputLibType::Static
        }
    }
}

impl OutputLibType {
    fn rust_lib_type(&self) -> &str {
        match self {
            OutputLibType::Static => "static",
            OutputLibType::Dynamic => "dylib",
        }
    }
}

impl OutputLib {
    fn libname(self, prefix: &Option<String>) -> String {
        let name = match self {
            OutputLib::Crypto => "crypto",
            OutputLib::Ssl => "ssl",
        };
        if let Some(prefix) = prefix {
            format!("{prefix}_{name}")
        } else {
            name.to_string()
        }
    }
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn prefix_string() -> String {
    format!("aws_lc_{}", VERSION.to_string().replace('.', "_"))
}

#[cfg(all(feature = "bindgen", feature = "all-bindings"))]
fn target_platform_prefix(name: &str) -> String {
    format!("{}_{}", effective_target().replace('-', "_"), name)
}

#[cfg(not(feature = "all-bindings"))]
fn target_has_prefixed_symbols() -> bool {
    target_vendor() == "apple" || (target_arch() == "x86" && target_os() == "windows")
}

#[cfg(not(feature = "all-bindings"))]
fn is_cranelift_backend() -> bool {
    // CARGO_ENCODED_RUSTFLAGS contains flags separated by 0x1f (ASCII Unit Separator)
    if let Some(rustflags) = optional_env("CARGO_ENCODED_RUSTFLAGS") {
        for flag in rustflags.split('\x1f') {
            if flag.contains("codegen-backend=cranelift") {
                return true;
            }
        }
    }
    false
}

#[cfg(not(feature = "all-bindings"))]
fn target_chokes_on_u1() -> bool {
    target_arch() == "mips" || target_arch() == "mips64" || is_cranelift_backend()
}

#[cfg(all(feature = "bindgen", not(feature = "all-bindings")))]
fn target_platform_prefix(name: &str) -> String {
    if target_has_prefixed_symbols() {
        format!("universal_prefixed_{}", name.replace('-', "_"))
    } else {
        format!("universal_{}", name.replace('-', "_"))
    }
}

pub(crate) struct TestCommandResult {
    #[allow(dead_code)]
    stderr: Box<str>,
    #[allow(dead_code)]
    stdout: Box<str>,
    executed: bool,
    status: bool,
}

const MAX_CMD_OUTPUT_SIZE: usize = 1 << 15;
fn execute_command(executable: &OsStr, args: &[&OsStr]) -> TestCommandResult {
    if let Ok(mut result) = Command::new(executable).args(args).output() {
        result.stderr.truncate(MAX_CMD_OUTPUT_SIZE);
        let stderr = String::from_utf8(result.stderr)
            .unwrap_or_default()
            .into_boxed_str();
        result.stdout.truncate(MAX_CMD_OUTPUT_SIZE);
        let stdout = String::from_utf8(result.stdout)
            .unwrap_or_default()
            .into_boxed_str();
        return TestCommandResult {
            stderr,
            stdout,
            executed: true,
            status: result.status.success(),
        };
    }
    TestCommandResult {
        stderr: String::new().into_boxed_str(),
        stdout: String::new().into_boxed_str(),
        executed: false,
        status: false,
    }
}

#[cfg(feature = "bindgen")]
fn generate_bindings(manifest_dir: &Path, prefix: &Option<String>, bindings_path: &PathBuf) {
    let options = BindingOptions {
        build_prefix: prefix.clone(),
        include_ssl: cfg!(feature = "ssl"),
        disable_prelude: true,
    };

    let bindings = sys_bindgen::generate_bindings(manifest_dir, &options);

    bindings
        .write(Box::new(std::fs::File::create(bindings_path).unwrap()))
        .expect("written bindings");
}

#[cfg(feature = "bindgen")]
fn generate_src_bindings(manifest_dir: &Path, prefix: &Option<String>, src_bindings_path: &Path) {
    sys_bindgen::generate_bindings(
        manifest_dir,
        &BindingOptions {
            build_prefix: prefix.clone(),
            include_ssl: false,
            ..Default::default()
        },
    )
    .write_to_file(src_bindings_path)
    .expect("write bindings");
}

fn emit_rustc_cfg(cfg: &str) {
    let cfg = cfg.replace('-', "_");
    emit_warning(format!("Emitting configuration: cargo:rustc-cfg={cfg}"));
    println!("cargo:rustc-cfg={cfg}");
}

fn emit_warning<T: AsRef<str>>(message: T) {
    println!("cargo:warning={}", message.as_ref());
}

#[allow(dead_code)]
fn target_family() -> String {
    cargo_env("CARGO_CFG_TARGET_FAMILY")
}

fn target_os() -> String {
    cargo_env("CARGO_CFG_TARGET_OS")
}

fn target_arch() -> String {
    cargo_env("CARGO_CFG_TARGET_ARCH")
}

#[allow(unused)]
fn target_env() -> String {
    cargo_env("CARGO_CFG_TARGET_ENV")
}

#[allow(unused)]
fn target_vendor() -> String {
    cargo_env("CARGO_CFG_TARGET_VENDOR")
}

fn target() -> String {
    cargo_env("TARGET")
}

fn crate_name() -> String {
    cargo_env("CARGO_PKG_NAME")
}

fn effective_target() -> String {
    #[allow(unknown_lints)]
    #[allow(static_mut_refs)]
    unsafe {
        if !AWS_LC_SYS_EFFECTIVE_TARGET.is_empty() {
            return AWS_LC_SYS_EFFECTIVE_TARGET.clone();
        }
    }
    let target = target();
    match target.as_str() {
        "x86_64-alpine-linux-musl" => "x86_64-unknown-linux-musl".to_string(),
        "aarch64-alpine-linux-musl" => "aarch64-unknown-linux-musl".to_string(),
        _ => target,
    }
}

#[allow(unused)]
fn target_underscored() -> String {
    effective_target().replace('-', "_")
}

fn out_dir() -> PathBuf {
    PathBuf::from(cargo_env("OUT_DIR"))
}

fn current_dir() -> PathBuf {
    std::env::current_dir().unwrap()
}

fn get_builder(prefix: &Option<String>, manifest_dir: &Path, out_dir: &Path) -> Box<dyn Builder> {
    let cmake_builder_builder = || {
        Box::new(CmakeBuilder::new(
            manifest_dir.to_path_buf(),
            out_dir.to_path_buf(),
            prefix.clone(),
            OutputLibType::default(),
        ))
    };

    let cc_builder_builder = || {
        Box::new(CcBuilder::new(
            manifest_dir.to_path_buf(),
            out_dir.to_path_buf(),
            prefix.clone(),
            OutputLibType::default(),
        ))
    };

    if let Some(val) = is_cmake_builder() {
        let builder: Box<dyn Builder> = if val {
            cmake_builder_builder()
        } else {
            cc_builder_builder()
        };
        builder.check_dependencies().unwrap();
        return builder;
    } else if is_no_asm() {
        let builder = cmake_builder_builder();
        builder.check_dependencies().unwrap();
        return builder;
    } else if !is_bindgen_required() {
        let cc_builder = cc_builder_builder();
        if cc_builder.check_dependencies().is_ok() {
            return cc_builder;
        }
    }
    let cmake_builder = cmake_builder_builder();
    cmake_builder.check_dependencies().unwrap();
    cmake_builder
}

trait Builder {
    fn check_dependencies(&self) -> Result<(), String>;
    fn build(&self) -> Result<(), String>;
    fn name(&self) -> &str;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CStdRequested {
    C99,
    C11,
    None,
}

impl CStdRequested {
    fn from_env() -> Self {
        if let Some(val) = optional_env_crate_target("C_STD") {
            let cstd = match val.as_str() {
                "99" => CStdRequested::C99,
                "11" => CStdRequested::C11,
                _ => CStdRequested::None,
            };
            emit_warning(format!(
                "AWS_LC_SYS_C_STD environment variable set: {cstd:?}"
            ));
            return cstd;
        }
        CStdRequested::None
    }
}

static mut PREGENERATED: bool = false;
static mut AWS_LC_SYS_NO_PREFIX: bool = false;
static mut AWS_LC_SYS_PREGENERATING_BINDINGS: bool = false;
static mut AWS_LC_SYS_EXTERNAL_BINDGEN: Option<bool> = None;
static mut AWS_LC_SYS_NO_ASM: bool = false;
static mut AWS_LC_SYS_PREBUILT_NASM: Option<bool> = None;
static mut AWS_LC_SYS_CMAKE_BUILDER: Option<bool> = None;
static mut AWS_LC_SYS_NO_PREGENERATED_SRC: bool = false;
static mut AWS_LC_SYS_EFFECTIVE_TARGET: String = String::new();
static mut AWS_LC_SYS_NO_JITTER_ENTROPY: Option<bool> = None;
static mut AWS_LC_SYS_NO_U1_BINDINGS: Option<bool> = None;

static mut AWS_LC_SYS_C_STD: CStdRequested = CStdRequested::None;

fn initialize() {
    unsafe {
        AWS_LC_SYS_NO_PREFIX = env_crate_var_to_bool("NO_PREFIX").unwrap_or(false);
        AWS_LC_SYS_PREGENERATING_BINDINGS =
            env_crate_var_to_bool("PREGENERATING_BINDINGS").unwrap_or(false);
        AWS_LC_SYS_EXTERNAL_BINDGEN = env_crate_var_to_bool("EXTERNAL_BINDGEN");
        AWS_LC_SYS_NO_ASM = env_crate_var_to_bool("NO_ASM").unwrap_or(false);
        AWS_LC_SYS_PREBUILT_NASM = env_crate_var_to_bool("PREBUILT_NASM");
        AWS_LC_SYS_C_STD = CStdRequested::from_env();
        AWS_LC_SYS_CMAKE_BUILDER = env_crate_var_to_bool("CMAKE_BUILDER");
        AWS_LC_SYS_NO_PREGENERATED_SRC =
            env_crate_var_to_bool("NO_PREGENERATED_SRC").unwrap_or(false);
        AWS_LC_SYS_EFFECTIVE_TARGET =
            optional_env_crate_target("EFFECTIVE_TARGET").unwrap_or_default();
        AWS_LC_SYS_NO_JITTER_ENTROPY = env_crate_var_to_bool("NO_JITTER_ENTROPY");
        AWS_LC_SYS_NO_U1_BINDINGS = env_crate_var_to_bool("NO_U1_BINDINGS");
    }

    if !is_external_bindgen_requested().unwrap_or(false)
        && (is_pregenerating_bindings() || !has_bindgen_feature())
    {
        #[cfg(feature = "all-bindings")]
        {
            assert!(
                use_no_u1_bindings() != Some(true),
                "Bindgen currently cannot generate prefixed bindings w/o the \\x01 prefix.",
            );
            let target = effective_target();
            let supported_platform = match target.as_str() {
                "aarch64-apple-darwin"
                | "aarch64-linux-android"
                | "aarch64-pc-windows-msvc"
                | "aarch64-unknown-linux-gnu"
                | "aarch64-unknown-linux-musl"
                | "i686-pc-windows-msvc"
                | "i686-unknown-linux-gnu"
                | "riscv64gc-unknown-linux-gnu"
                | "x86_64-apple-darwin"
                | "x86_64-pc-windows-gnu"
                | "x86_64-pc-windows-msvc"
                | "x86_64-unknown-linux-gnu"
                | "x86_64-unknown-linux-musl" => Some(target),
                _ => None,
            };
            if let Some(platform) = supported_platform {
                emit_rustc_cfg(platform.as_str());
                unsafe {
                    PREGENERATED = true;
                }
            }
        }
        #[cfg(not(feature = "all-bindings"))]
        {
            if use_no_u1_bindings() == Some(true)
                || (target_chokes_on_u1() && use_no_u1_bindings().is_none())
            {
                if is_cranelift_backend() {
                    emit_warning(
                        "Cranelift codegen backend detected. Using universal_no_u1 bindings.",
                    );
                }
                if target_has_prefixed_symbols() {
                    emit_rustc_cfg("universal-no-u1-prefixed");
                } else {
                    emit_rustc_cfg("universal-no-u1");
                }
            } else if target_has_prefixed_symbols() {
                emit_rustc_cfg("universal-prefixed");
            } else {
                emit_rustc_cfg("universal");
            }
            unsafe {
                PREGENERATED = true;
            }
        }
    }
}

fn is_bindgen_required() -> bool {
    is_no_prefix()
        || is_pregenerating_bindings()
        || is_external_bindgen_requested().unwrap_or(false)
        || has_bindgen_feature()
        || !has_pregenerated()
}

#[cfg(feature = "bindgen")]
fn internal_bindgen_supported() -> bool {
    let cv = bindgen::clang_version();
    emit_warning(format!("Clang version: {}", cv.full));
    true
}

fn is_no_prefix() -> bool {
    unsafe { AWS_LC_SYS_NO_PREFIX }
}

fn is_pregenerating_bindings() -> bool {
    unsafe { AWS_LC_SYS_PREGENERATING_BINDINGS }
}

fn is_external_bindgen_requested() -> Option<bool> {
    unsafe { AWS_LC_SYS_EXTERNAL_BINDGEN }
}

fn is_no_asm() -> bool {
    unsafe { AWS_LC_SYS_NO_ASM }
}

fn is_cmake_builder() -> Option<bool> {
    if is_no_pregenerated_src() {
        Some(true)
    } else {
        unsafe { AWS_LC_SYS_CMAKE_BUILDER }
    }
}

fn is_no_pregenerated_src() -> bool {
    unsafe { AWS_LC_SYS_NO_PREGENERATED_SRC }
}

fn disable_jitter_entropy() -> Option<bool> {
    unsafe { AWS_LC_SYS_NO_JITTER_ENTROPY }
}

fn use_no_u1_bindings() -> Option<bool> {
    unsafe { AWS_LC_SYS_NO_U1_BINDINGS }
}

fn get_crate_cc() -> Option<String> {
    optional_env_optional_crate_target("TARGET_CC").or(optional_env_optional_crate_target("CC"))
}

fn get_crate_cxx() -> Option<String> {
    optional_env_optional_crate_target("TARGET_CXX").or(optional_env_optional_crate_target("CXX"))
}

fn get_crate_cflags() -> Option<String> {
    optional_env_optional_crate_target("TARGET_CFLAGS")
        .or(optional_env_optional_crate_target("CFLAGS"))
}

fn use_prebuilt_nasm() -> bool {
    target_os() == "windows"
        && target_arch() == "x86_64"
        && !is_no_asm()
        && !test_nasm_command() // NASM not found in environment
        && Some(false) != allow_prebuilt_nasm() // not prevented by environment
        && !cfg!(feature = "disable-prebuilt-nasm") // not prevented by feature
        // permitted by environment or by feature
        && (Some(true) == allow_prebuilt_nasm() || cfg!(feature = "prebuilt-nasm"))
}

fn allow_prebuilt_nasm() -> Option<bool> {
    unsafe { AWS_LC_SYS_PREBUILT_NASM }
}

fn requested_c_std() -> CStdRequested {
    unsafe { AWS_LC_SYS_C_STD }
}

fn has_bindgen_feature() -> bool {
    cfg!(feature = "bindgen")
}

fn has_pregenerated() -> bool {
    unsafe { PREGENERATED }
}

fn test_nasm_command() -> bool {
    let status = execute_command("nasm".as_ref(), &["-version".as_ref()]).status;
    if !status {
        emit_warning("NASM command not found or failed to execute.");
    }
    status
}

fn test_clang_cl_command() -> bool {
    execute_command("clang-cl".as_ref(), &["--version".as_ref()]).status
}

fn prepare_cargo_cfg() {
    if cfg!(clippy) {
        println!("cargo:rustc-check-cfg=cfg(use_bindgen_pregenerated)");
        println!("cargo:rustc-check-cfg=cfg(aarch64_linux_android)");
        println!("cargo:rustc-check-cfg=cfg(aarch64_apple_darwin)");
        println!("cargo:rustc-check-cfg=cfg(aarch64_pc_windows_msvc)");
        println!("cargo:rustc-check-cfg=cfg(aarch64_unknown_linux_gnu)");
        println!("cargo:rustc-check-cfg=cfg(aarch64_unknown_linux_musl)");
        println!("cargo:rustc-check-cfg=cfg(i686_pc_windows_msvc)");
        println!("cargo:rustc-check-cfg=cfg(i686_unknown_linux_gnu)");
        println!("cargo:rustc-check-cfg=cfg(riscv64gc_unknown_linux_gnu)");
        println!("cargo:rustc-check-cfg=cfg(x86_64_apple_darwin)");
        println!("cargo:rustc-check-cfg=cfg(x86_64_pc_windows_gnu)");
        println!("cargo:rustc-check-cfg=cfg(x86_64_pc_windows_msvc)");
        println!("cargo:rustc-check-cfg=cfg(x86_64_unknown_linux_gnu)");
        println!("cargo:rustc-check-cfg=cfg(x86_64_unknown_linux_musl)");
        println!("cargo:rustc-check-cfg=cfg(universal)");
        println!("cargo:rustc-check-cfg=cfg(universal_no_u1)");
        println!("cargo:rustc-check-cfg=cfg(universal_no_u1_prefixed)");
        println!("cargo:rustc-check-cfg=cfg(universal_prefixed)");
    }
}

fn is_crt_static() -> bool {
    let features = cargo_env("CARGO_CFG_TARGET_FEATURE");
    features.contains("crt-static")
}

#[cfg(feature = "bindgen")]
fn handle_bindgen(manifest_dir: &Path, prefix: &Option<String>) -> bool {
    if internal_bindgen_supported() && !is_external_bindgen_requested().unwrap_or(false) {
        emit_warning(format!(
            "Generating bindings - internal bindgen. Platform: {}",
            effective_target()
        ));
        let gen_bindings_path = out_dir().join("bindings.rs");
        generate_bindings(manifest_dir, prefix, &gen_bindings_path);
        emit_rustc_cfg("use_bindgen_pregenerated");
        true
    } else {
        false
    }
}

#[cfg(not(feature = "bindgen"))]
fn handle_bindgen(_manifest_dir: &Path, _prefix: &Option<String>) -> bool {
    false
}

fn main() {
    initialize();
    prepare_cargo_cfg();

    let manifest_dir = current_dir();
    let manifest_dir = dunce::canonicalize(Path::new(&manifest_dir)).unwrap();
    let prefix_str = prefix_string();
    let prefix = if is_no_prefix() {
        None
    } else {
        Some(prefix_str)
    };

    let builder = get_builder(&prefix, &manifest_dir, &out_dir());
    emit_warning(format!("Building with: {}", builder.name()));
    emit_warning(format!("Symbol Prefix: {:?}", &prefix));

    builder.check_dependencies().unwrap();

    #[allow(unused_assignments)]
    let mut bindings_available = false;
    emit_warning(format!("Target platform: '{}'", target()));
    if is_pregenerating_bindings() {
        #[cfg(feature = "bindgen")]
        {
            let src_bindings_path = Path::new(&manifest_dir)
                .join("src")
                .join(format!("{}.rs", target_platform_prefix("crypto")));
            if is_external_bindgen_requested().unwrap_or(false) {
                assert!(
                    !is_pregenerating_bindings(),
                    "Pregenerated bindings not supported using external bindgen.",
                );
                invoke_external_bindgen(&manifest_dir, &prefix, &src_bindings_path).unwrap();
            } else {
                generate_src_bindings(&manifest_dir, &prefix, &src_bindings_path);
            }
            bindings_available = true;
        }
    } else if is_bindgen_required() {
        emit_warning("######");
        emit_warning(
            "If bindgen is unable to locate a header file, use the \
            BINDGEN_EXTRA_CLANG_ARGS environment variable to specify additional include paths.",
        );
        emit_warning("See: https://github.com/rust-lang/rust-bindgen?tab=readme-ov-file#environment-variables");
        emit_warning("######");
        let aws_lc_crypto_dir = Path::new(&manifest_dir).join("aws-lc").join("crypto");
        if !aws_lc_crypto_dir.exists() {
            emit_warning("######");
            emit_warning("###### WARNING: MISSING GIT SUBMODULE ######");
            emit_warning(format!(
                "  -- Did you initialize the repo's git submodules? Unable to find crypto directory: {}.",
                &aws_lc_crypto_dir.display()
            ));
            emit_warning("  -- run 'git submodule update --init --recursive' to initialize.");
            emit_warning("######");
            emit_warning("######");
        }
        bindings_available = handle_bindgen(&manifest_dir, &prefix);
    } else {
        bindings_available = true;
    }

    if !bindings_available {
        if cfg!(feature = "ssl") {
            emit_warning(
                "External bindgen required, but external bindgen unable to produce SSL bindings.",
            );
        } else {
            let gen_bindings_path = out_dir().join("bindings.rs");
            let result = invoke_external_bindgen(&manifest_dir, &prefix, &gen_bindings_path);
            match result {
                Ok(()) => {
                    emit_rustc_cfg("use_bindgen_pregenerated");
                    bindings_available = true;
                }
                Err(msg) => eprintln!("Failure invoking external bindgen! {msg}"),
            }
        }
    }

    assert!(
        bindings_available,
        "aws-lc-sys build failed. Please enable the 'bindgen' feature on aws-lc-rs or aws-lc-sys.\
        For more information, see the aws-lc-rs User Guide: https://aws.github.io/aws-lc-rs/index.html"
    );
    builder.build().unwrap();

    println!(
        "cargo:include={}",
        setup_include_paths(&out_dir(), &manifest_dir).display()
    );

    // export the artifact names
    println!("cargo:libcrypto={}_crypto", prefix_string());
    if cfg!(feature = "ssl") {
        println!("cargo:libssl={}_ssl", prefix_string());
    }

    println!("cargo:conf={}", OSSL_CONF_DEFINES.join(","));

    println!("cargo:rerun-if-changed=builder/");
    println!("cargo:rerun-if-changed=aws-lc/");
}

fn setup_include_paths(out_dir: &Path, manifest_dir: &Path) -> PathBuf {
    let mut include_paths = vec![
        get_rust_include_path(manifest_dir),
        get_generated_include_path(manifest_dir),
        get_aws_lc_include_path(manifest_dir),
    ];

    if let Some(extra_paths) = get_aws_lc_sys_includes_path() {
        include_paths.extend(extra_paths);
    }

    let include_dir = out_dir.join("include");
    std::fs::create_dir_all(&include_dir).unwrap();

    // iterate over all the include paths and copy them into the final output
    for path in include_paths {
        for child in std::fs::read_dir(path).into_iter().flatten().flatten() {
            if child.file_type().map_or(false, |t| t.is_file()) {
                let _ = std::fs::copy(
                    child.path(),
                    include_dir.join(child.path().file_name().unwrap()),
                );
                continue;
            }

            // prefer the earliest paths
            let options = fs_extra::dir::CopyOptions::new()
                .skip_exist(true)
                .copy_inside(true);
            let _ = fs_extra::dir::copy(child.path(), &include_dir, &options);
        }
    }

    include_dir
}

#[derive(Default)]
#[allow(dead_code)]
pub(crate) struct BindingOptions {
    pub build_prefix: Option<String>,
    pub include_ssl: bool,
    pub disable_prelude: bool,
}

impl Debug for BindingOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingOptions")
            .field("build_prefix", &self.build_prefix)
            .field("include_ssl", &self.include_ssl)
            .field("disable_prelude", &self.disable_prelude)
            .finish()
    }
}

fn verify_bindgen() -> Result<(), String> {
    if !is_external_bindgen_requested().unwrap_or(true) {
        return Err(
            "external bindgen usage prevented by AWS_LC_SYS_EXTERNAL_BINDGEN=0".to_string(),
        );
    }

    let result = execute_command("bindgen".as_ref(), &["--version".as_ref()]);
    if !result.status {
        if result.executed {
            eprintln!(
                "bindgen-cli exited with an error status:\nSTDOUT: {}\n\nSTDERR: {}",
                result.stdout, result.stderr
            );
        } else {
            eprintln!(
                "Consider installing the bindgen-cli: \
            `cargo install --force --locked bindgen-cli`\
            \n\
            See our User Guide for more information about bindgen:\
            https://aws.github.io/aws-lc-rs/index.html"
            );
        }
        return Err("External bindgen command failed.".to_string());
    }
    let mut major_version: u32 = 0;
    let mut minor_version: u32 = 0;
    let mut patch_version: u32 = 0;
    let bindgen_version = result.stdout.split(' ').nth(1);
    if let Some(version) = bindgen_version {
        let version_parts: Vec<&str> = version.trim().split('.').collect();
        if version_parts.len() == 3 {
            major_version = version_parts[0].parse::<u32>().unwrap_or(0);
            minor_version = version_parts[1].parse::<u32>().unwrap_or(0);
            patch_version = version_parts[2].parse::<u32>().unwrap_or(0);
        }
    }
    // We currently expect to support all bindgen versions >= 0.69.5
    if major_version == 0 && (minor_version < 69 || (minor_version == 69 && patch_version < 5)) {
        eprintln!(
            "bindgen-cli was used. Detected version was: \
            {major_version}.{minor_version}.{patch_version} \n\
        Consider upgrading : \
        `cargo install --force --locked bindgen-cli`\
        \n\
        See our User Guide for more information about bindgen:\
        https://aws.github.io/aws-lc-rs/index.html"
        );
    }
    Ok(())
}

const PRELUDE: &str = r"
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::default_trait_access,
    clippy::missing_safety_doc,
    clippy::must_use_candidate,
    clippy::not_unsafe_ptr_arg_deref,
    clippy::ptr_as_ptr,
    clippy::ptr_offset_with_cast,
    clippy::pub_underscore_fields,
    clippy::semicolon_if_nothing_returned,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::used_underscore_binding,
    clippy::useless_transmute,
    dead_code,
    improper_ctypes,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unpredictable_function_pointer_comparisons,
    unused_imports
)]
";

fn invoke_external_bindgen(
    manifest_dir: &Path,
    prefix: &Option<String>,
    gen_bindings_path: &Path,
) -> Result<(), String> {
    verify_bindgen()?;

    emit_warning(format!(
        "Generating bindings - external bindgen. Platform: {}",
        effective_target()
    ));

    let options = BindingOptions {
        // We collect the symbols w/o the prefix added
        build_prefix: None,
        include_ssl: false,
        disable_prelude: true,
    };

    let clang_args = prepare_clang_args(manifest_dir, &options);
    let header = get_rust_include_path(manifest_dir)
        .join("rust_wrapper.h")
        .display()
        .to_string();

    let sym_prefix: String;
    let mut bindgen_params = vec![];
    if let Some(prefix_str) = prefix {
        sym_prefix = if target_os().to_lowercase() == "macos"
            || target_os().to_lowercase() == "ios"
            || target_os().to_lowercase() == "tvos"
            || (target_os().to_lowercase() == "windows" && target_arch() == "x86")
        {
            format!("_{prefix_str}_")
        } else {
            format!("{prefix_str}_")
        };
        bindgen_params.extend(vec!["--prefix-link-name", sym_prefix.as_str()]);
    }

    // These flags needs to be kept in sync with the setup in bindgen::prepare_bindings_builder
    // If `bindgen-cli` makes backwards incompatible changes, we will update the parameters below
    // to conform with the most recent release. We will guide consumers to likewise use the
    // latest version of bindgen-cli.
    bindgen_params.extend(vec![
        "--allowlist-file",
        r".*(/|\\)openssl((/|\\)[^/\\]+)+\.h",
        "--allowlist-file",
        r".*(/|\\)rust_wrapper\.h",
        "--rustified-enum",
        r"point_conversion_form_t",
        "--default-macro-constant-type",
        r"signed",
        "--with-derive-default",
        "--with-derive-partialeq",
        "--with-derive-eq",
        "--raw-line",
        COPYRIGHT,
        "--generate",
        "functions,types,vars,methods,constructors,destructors",
        header.as_str(),
        "--rust-target",
        r"1.70",
        "--output",
        gen_bindings_path.to_str().unwrap(),
        "--formatter",
        r"rustfmt",
    ]);
    if !options.disable_prelude {
        bindgen_params.extend(["--raw-line", PRELUDE]);
    }
    bindgen_params.push("--");

    for x in &clang_args {
        bindgen_params.push(x.as_str());
    }
    let cmd_params: Vec<OsString> = bindgen_params.iter().map(OsString::from).collect();
    let cmd_params: Vec<&OsStr> = cmd_params.iter().map(OsString::as_os_str).collect();

    let result = execute_command("bindgen".as_ref(), cmd_params.as_ref());
    if !result.status {
        return Err(format!(
            "\n\n\
            bindgen-PARAMS: {}\n\
            bindgen-STDOUT: {}\n\
            bindgen-STDERR: {}",
            bindgen_params.join(" "),
            result.stdout.as_ref(),
            result.stderr.as_ref()
        ));
    }
    Ok(())
}

fn add_header_include_path(args: &mut Vec<String>, path: String) {
    args.push("-I".to_string());
    args.push(path);
}

fn prepare_clang_args(manifest_dir: &Path, options: &BindingOptions) -> Vec<String> {
    let mut clang_args: Vec<String> = Vec::new();

    add_header_include_path(
        &mut clang_args,
        get_rust_include_path(manifest_dir).display().to_string(),
    );

    if options.build_prefix.is_some() {
        // NOTE: It's possible that the prefix embedded in the header files doesn't match the prefix
        // specified. This only happens when the version number as changed in Cargo.toml, but the
        // new headers have not yet been generated.
        add_header_include_path(
            &mut clang_args,
            get_generated_include_path(manifest_dir)
                .display()
                .to_string(),
        );
    }

    add_header_include_path(
        &mut clang_args,
        get_aws_lc_include_path(manifest_dir).display().to_string(),
    );

    if let Some(include_paths) = get_aws_lc_sys_includes_path() {
        for path in include_paths {
            add_header_include_path(&mut clang_args, path.display().to_string());
        }
    }

    clang_args
}

const COPYRIGHT: &str = r"
// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR ISC
";
