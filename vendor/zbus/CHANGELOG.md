# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 5.19.0 - 2026-08-09

### Added
- ✨ Add Builder::async_io_unix_stream and async_io_tcp_stream.
- ✨ Make the tokio and async-io features additive.

### Changed
- ♻️ Remove unnecessary clones.

### Deprecated
- 🗑️ Deprecate unix_stream/tcp_stream when tokio is disabled.

### Documentation
- 📝 Document the tokio and async-io runtime selection.

### Fixed
- 🥅 throw Error::Connection on connection failure.
- 🥅 Add new error variant Error::Connection.

### Other
- ✏️ Fix a typo in the executor docs.

### Testing
- ✅ Test runtime selection with both backends enabled.
- ✅ add tests for Error::Connection.

## 5.18.0 - 2026-07-17

### Added
- ✨ Add Connection::closed() and is_closed() API.

### Documentation
- 📝 Document ObjectManager-ancestor deadlock in ObjectServer::at.

### Other
- 🚨 Replace manual match-and-return with the ? operator.

### Testing
- ✅ Test registering an interface from a &mut self callback.

## 5.17.0 - 2026-07-07

### Changed
- ♻️ Move pending method call to separate module.

### Other
- 🧐 Add concurrent method-call benchmark.

### Performance
- ⚡️ Route method replies directly by serial.

## 5.16.0 - 2026-05-27

### Changed
- 🔧 Restrict vsock features to Linux targets.

### Documentation
- 📝 Replace docs.rs all-features with explicit feature list.

### Fixed
- 🚑️ Fix sendmsg for iOS and other apple OS.

### Other
- 🔊 warn on GetAll error during property cache init. #1325

## 5.15.0 - 2026-04-26

### Added
- ✨ Introduce DispatchResult2 with fdo::Result for dispatch futures.
- ✨ Add Builder::build_message_stream.

### Changed
- ♻️ Port Interface and dispatch sites to DispatchResult2.

### Deprecated
- 🗑️ Deprecate DispatchResult in favour of DispatchResult2.

### Documentation
- 📝 Configure docs.rs to build for all supported targets.

### Testing
- ✅ Cover D-Bus error name preservation on property setters.
- ✅ Explicitly choose host endianess in a test.

## 5.14.0 - 2026-02-22

### Added
- ✨ Add helper for IBus connection creation. #964
- 🚸 Add Display trait to D-Bus name request reply types.

### Changed
- 🔧 Extend process module run() to all Unix platforms.

### Fixed
- 🐛 Do not use SendFlags::NOSIGNAL on Redox.

### Other
- 📦️ Add async-recursion for Unix targets.
- 🚨 silence unused import on windows.
- 🚨 silence unused warning on windows test.

## 5.13.2 - 2026-01-19

### Fixed
- 🐛 fix regression on windows build. #1686
- 🐛 Correct Peer interface to work on any arbitrary object path.

## 5.13.1 - 2026-01-11

### Fixed
- 🐛 Implement `get_machine_id()` for *BSD platforms.

## 5.13.0 - 2026-01-09

### Added
- ✨ Add crate attribute for custom crate paths.

### Changed
- 🎨 Format all files (rust 1.85).

### Fixed
- 🚑️ Send on unix sockets w/ `MSG_NOSIGNAL` flag enabled. #1657
- 🐛 Fix `get_machine_id` for macOS.

### Other
- 🧱 Fix all clippy warnings (rust 1.85).
- 🧑‍💻 Bump rust version to 1.85.
- 🔊 lower trace/instrument verbosity.

### Testing
- ✅ Add introspection test for out_args with single output.
- ✅ Remove unused imports from tests.
