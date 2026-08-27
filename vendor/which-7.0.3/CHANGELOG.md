# CHANGELOG

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
