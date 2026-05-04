# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
