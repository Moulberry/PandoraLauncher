# CHANGELOG

## 8.0.6

- Bug fix: [#128](https://github.com/harryfei/which-rs/pull/128) Resolve relative PATH entries against the user provided CWD when available, rather than the process CWD.
  Thanks [@RSS1102](https://github.com/RSS1102) for your contribution to which!

## 8.0.5

- Bug fix: [#126](https://github.com/harryfei/which-rs/pull/126) Do not use current directory for search when provided path is absolute. Thanks [@weifanglab](https://github.com/weifanglab) for your contribution to which!

## 8.0.4

- On Windows we now emit a `NonFatalError` if the `PATHEXT` environment variable is not populated, and the query did not specify a file extension.

## 8.0.3

- Add fallback implementation of `is_valid_executable` allowing `which-rs` to compile on targets which are not Unix, Windows, WASI, or Redox. Thanks [@pmikolajczyk41](https://github.com/pmikolajczyk41) for your contribution to which!

## 8.0.2

- Dependency on `home_env` removed, the implementation found in rust 1.85.0 for a home directory has been fixed.
  Thanks, [@madsmtm],(https://github.com/madsmtm) for this contribution to which!
- Dependency on `winsafe` removed, code for Windows API is now handwritten.
- Dependency on `rustix` removed, we now depend on `libc` directly to reduce compile times.

## 8.0.1

- Fix Windows bug reported in https://github.com/harryfei/which-rs/issues/108#issuecomment-3998654328

## 8.0.0

- Add new `Sys` trait to allow abstracting over the underlying filesystem. Particularly useful for `wasm32-unknown-unknown` targets. Thanks [@dsherret](https://github.com/dsherret) for this contribution to which!
- Add more debug level tracing for otherwise silent I/O errors.
- Call the `NonFatalHandler` in more places to catch previously ignored I/O errors.  
- Remove use of the `either` dependency.

## 7.0.3

- Update rustix to version 1.0. Congrats to rustix on this milestone, and thanks [@mhils](https://github.com/mhils) for this contribution to which!

## 7.0.2

- Don't return paths containing the single dot `.` reference to the current directory, even if the original request was given in
terms of the current directory. Thanks [@jakobhellermann](https://github.com/jakobhellermann) for this contribution!

## 7.0.1

- Get user home directory from `env_home` instead of `home`. Thanks [@micolous](https://github.com/micolous) for this contribution!
- If home directory is unavailable, do not expand the tilde to an empty string. Leave it as is.

## 7.0.0

- Add support to `WhichConfig` for a user provided closure that will be called whenever a nonfatal error occurs.
  This technically breaks a few APIs due to the need to add more generics and lifetimes. Most code will compile
  without changes.

## 6.0.3

- Enhance `tracing` feature with some `debug` level logs for higher level logic.

## 6.0.2

- Add `tracing` feature which outputs debugging information to the [`tracing`](https://crates.io/crates/tracing) ecosystem.

## 6.0.1

- Remove dependency on `once_cell` for Windows users, replace with `std::sync::OnceLock`.

## 6.0.0

- MSRV is now 1.70
- Upgraded all dependencies to latest version

## 5.0.0

- Remove several unused error messages
- Windows executables can now be found even if they don't have a '.exe' extension.
- Add new error message, `Error::CannotGetCurrentDirAndPathListEmpty`

## 4.4.2

- Remove dependency on `dirs` crate due to MPL licensing in its tree. Use `home` crate instead. (@Xaeroxe)

## 4.4.1

- Add tilde expansion for home directory (@Xaeroxe)
- Swap out libc for rustix, forbid unsafe (@notgull)
