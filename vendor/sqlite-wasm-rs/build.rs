// SQLite compile flags tuned for WASM: no threads/dlopen, keep common extensions.
const FULL_FEATURED: [&str; 23] = [
    "-DSQLITE_OS_OTHER",
    "-DSQLITE_USE_URI",
    // SQLite is configured for a single-threaded environment, as WebAssembly is single-threaded by default.
    "-DSQLITE_THREADSAFE=0",
    "-DSQLITE_TEMP_STORE=2",
    "-DSQLITE_DEFAULT_CACHE_SIZE=-16384",
    "-DSQLITE_DEFAULT_PAGE_SIZE=8192",
    "-DSQLITE_OMIT_DEPRECATED",
    // Disable extension loading, as dynamic linking (dlopen) is not supported in WASM.
    "-DSQLITE_OMIT_LOAD_EXTENSION",
    // In a single-threaded context, a shared cache is unnecessary.
    "-DSQLITE_OMIT_SHARED_CACHE",
    "-DSQLITE_ENABLE_UNLOCK_NOTIFY",
    "-DSQLITE_ENABLE_API_ARMOR",
    "-DSQLITE_ENABLE_BYTECODE_VTAB",
    "-DSQLITE_ENABLE_DBPAGE_VTAB",
    "-DSQLITE_ENABLE_DBSTAT_VTAB",
    "-DSQLITE_ENABLE_FTS5",
    "-DSQLITE_ENABLE_MATH_FUNCTIONS",
    "-DSQLITE_ENABLE_OFFSET_SQL_FUNC",
    "-DSQLITE_ENABLE_PREUPDATE_HOOK",
    "-DSQLITE_ENABLE_RTREE",
    "-DSQLITE_ENABLE_SESSION",
    "-DSQLITE_ENABLE_STMTVTAB",
    "-DSQLITE_ENABLE_UNKNOWN_SQL_FUNCTION",
    "-DSQLITE_ENABLE_COLUMN_METADATA",
];

#[cfg(feature = "sqlite3mc")]
const SQLITE3_MC_FEATURED: [&str; 2] = ["-D__WASM__", "-DARGON2_NO_THREADS"];

const UPDATE_BINDGEN_ENV: &str = "SQLITE_WASM_RS_UPDATE_BINDGEN";

fn main() {
    println!("cargo::rerun-if-env-changed={UPDATE_BINDGEN_ENV}");
    println!("cargo::rerun-if-changed=shim");

    #[cfg(feature = "sqlite3mc")]
    println!("cargo::rerun-if-changed=sqlite3mc");

    #[cfg(not(feature = "sqlite3mc"))]
    println!("cargo::rerun-if-changed=sqlite3");

    compile();

    #[cfg(feature = "bindgen")]
    {
        let update_bindgen = std::env::var(UPDATE_BINDGEN_ENV).is_ok();
        let output =
            std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR env not set"))
                .join("bindgen.rs");
        bindgen(&output);

        if update_bindgen {
            #[cfg(not(feature = "sqlite3mc"))]
            const SQLITE3_BINDGEN: &str = "src/bindings/sqlite3_bindgen.rs";
            #[cfg(feature = "sqlite3mc")]
            const SQLITE3_BINDGEN: &str = "src/bindings/sqlite3mc_bindgen.rs";
            std::fs::copy(&output, SQLITE3_BINDGEN).unwrap();
        }
    }
}

#[cfg(feature = "bindgen")]
fn bindgen(output: &std::path::PathBuf) {
    #[cfg(not(feature = "sqlite3mc"))]
    const SQLITE3_HEADER: &str = "sqlite3/sqlite3.h";
    #[cfg(feature = "sqlite3mc")]
    const SQLITE3_HEADER: &str = "sqlite3mc/sqlite3mc_amalgamation.h";

    use bindgen::callbacks::{IntKind, ParseCallbacks};

    #[derive(Debug)]
    struct SqliteTypeChooser;

    impl ParseCallbacks for SqliteTypeChooser {
        fn int_macro(&self, name: &str, _value: i64) -> Option<IntKind> {
            if name == "SQLITE_SERIALIZE_NOCOPY"
                || name.starts_with("SQLITE_DESERIALIZE_")
                || name.starts_with("SQLITE_PREPARE_")
                || name.starts_with("SQLITE_TRACE_")
            {
                Some(IntKind::UInt)
            } else {
                None
            }
        }
    }

    let mut bindings = bindgen::builder()
        .default_macro_constant_type(bindgen::MacroTypeVariation::Signed)
        .disable_nested_struct_naming()
        .generate_cstr(true)
        .trust_clang_mangling(false)
        .header(SQLITE3_HEADER)
        .parse_callbacks(Box::new(SqliteTypeChooser));

    bindings = bindings
        .blocklist_function("sqlite3_auto_extension")
        .raw_line(
            r#"extern "C" {
    pub fn sqlite3_auto_extension(
        xEntryPoint: ::core::option::Option<
            unsafe extern "C" fn(
                db: *mut sqlite3,
                pzErrMsg: *mut *mut ::core::ffi::c_char,
                _: *const sqlite3_api_routines,
            ) -> ::core::ffi::c_int,
        >,
    ) -> ::core::ffi::c_int;
}"#,
        )
        .blocklist_function("sqlite3_cancel_auto_extension")
        .raw_line(
            r#"extern "C" {
    pub fn sqlite3_cancel_auto_extension(
        xEntryPoint: ::core::option::Option<
            unsafe extern "C" fn(
                db: *mut sqlite3,
                pzErrMsg: *mut *mut ::core::ffi::c_char,
                _: *const sqlite3_api_routines,
            ) -> ::core::ffi::c_int,
        >,
    ) -> ::core::ffi::c_int;
}"#,
        )
        // Block functions related to dynamic library loading, which is not available.
        .blocklist_function("sqlite3_load_extension")
        .raw_line(
            r#"pub unsafe fn sqlite3_load_extension(
    _db: *mut sqlite3,
    _zFile: *const ::core::ffi::c_char,
    _zProc: *const ::core::ffi::c_char,
    _pzErrMsg: *mut *mut ::core::ffi::c_char,
) -> ::core::ffi::c_int {
    // SQLITE_ERROR
    1
}"#,
        )
        .blocklist_function("sqlite3_enable_load_extension")
        .raw_line(
            r#"pub unsafe fn sqlite3_enable_load_extension(
    _db: *mut sqlite3,
    _onoff: ::core::ffi::c_int,
) -> ::core::ffi::c_int {
    // SQLITE_ERROR
    1
}"#,
        )
        // Block deprecated functions that are omitted from the build via the DSQLITE_OMIT_DEPRECATED flag.
        .blocklist_function("sqlite3_profile")
        .blocklist_function("sqlite3_trace")
        // Exclude UTF-16 entrypoints to keep the WASM surface minimal.
        .blocklist_function(".*16.*")
        .blocklist_function("sqlite3_close_v2")
        .blocklist_function("sqlite3_create_collation")
        .blocklist_function("sqlite3_create_function")
        .blocklist_function("sqlite3_create_module")
        .blocklist_function("sqlite3_prepare");

    bindings = bindings.clang_args(FULL_FEATURED);

    #[cfg(feature = "sqlite3mc")]
    {
        bindings = bindings.clang_args(SQLITE3_MC_FEATURED);
    }

    bindings = bindings
        .blocklist_function("sqlite3_vmprintf")
        .blocklist_function("sqlite3_vsnprintf")
        .blocklist_function("sqlite3_str_vappendf")
        .blocklist_type("va_list")
        .blocklist_item("__.*");

    bindings = bindings
        // Workaround for bindgen issue #1941, ensuring symbols are public.
        // https://github.com/rust-lang/rust-bindgen/issues/1941
        .clang_arg("-fvisibility=default");

    let bindings = bindings
        .layout_tests(false)
        .use_core()
        .formatter(bindgen::Formatter::Prettyplease)
        .generate()
        .unwrap();

    bindings.write_to_file(output).unwrap();
}

fn compile() {
    const C_SOURCE: [&str; 36] = [
        // string
        "string/memchr.c",
        "string/memrchr.c",
        "string/stpcpy.c",
        "string/stpncpy.c",
        "string/strcat.c",
        "string/strchr.c",
        "string/strchrnul.c",
        "string/strcmp.c",
        "string/strcpy.c",
        "string/strcspn.c",
        "string/strlen.c",
        "string/strncat.c",
        "string/strncmp.c",
        "string/strncpy.c",
        "string/strrchr.c",
        "string/strspn.c",
        // stdlib
        "stdlib/atoi.c",
        "stdlib/bsearch.c",
        "stdlib/qsort.c",
        "stdlib/qsort_nr.c",
        "stdlib/strtod.c",
        "stdlib/strtol.c",
        // math
        "math/__fpclassifyl.c",
        "math/acosh.c",
        "math/asinh.c",
        "math/atanh.c",
        "math/fmodl.c",
        "math/scalbn.c",
        "math/scalbnl.c",
        "math/sqrt.c",
        "math/trunc.c",
        // errno
        "errno/__errno_location.c",
        // stdio
        "stdio/__toread.c",
        "stdio/__uflow.c",
        // internal
        "internal/floatscan.c",
        "internal/shgetc.c",
    ];

    #[cfg(not(feature = "sqlite3mc"))]
    const SQLITE3_SOURCE: &str = "sqlite3/sqlite3.c";
    #[cfg(feature = "sqlite3mc")]
    const SQLITE3_SOURCE: &str = "sqlite3mc/sqlite3mc_amalgamation.c";

    let mut cc = cc::Build::new();
    cc.warnings(false)
        .flag("-Wno-macro-redefined")
        .include("shim")
        .include("shim/musl/arch/generic")
        .include("shim/musl/include")
        .file("shim/printf/printf.c")
        .file(SQLITE3_SOURCE)
        .files(C_SOURCE.map(|s| format!("shim/musl/{s}")))
        .flag("-DPRINTF_ALIAS_STANDARD_FUNCTION_NAMES_HARD")
        .flag("-include")
        .flag("shim/wasm-shim.h");

    cc.flags(FULL_FEATURED);
    #[cfg(feature = "sqlite3mc")]
    cc.flags(SQLITE3_MC_FEATURED);

    cc.compile("wsqlite3");
}
