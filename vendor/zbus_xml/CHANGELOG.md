# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 5.2.1 - 2026-07-17

### Other
- Updated the following local packages: zvariant, zbus_names.

## 5.2.0 - 2026-07-07

### Added
- ✨ Support the Telepathy introspection extensions. #255
- ✨ Add a parser that builds the introspection tree.

### Changed
- ♻️ Port parsing and writing to the bespoke parser.

### Dependencies
- ➖ Drop the quick-xml dependency.

### Deprecated
- 🗑️ Deprecate the quick-xml error variants.

### Documentation
- 📝 Tidy the error module docs.

### Other
- 🧐 Add real-world introspection test data and benchmarks.

### Testing
- ✅ Test the parser.
- ✅ Add regression test for bare struct arg signatures.

## 5.1.1 - 2026-04-26

### Documentation
- 📝 Configure docs.rs to build for all supported targets.

### Other
- 🤖 Fix formatting of CHANGELOG files.
- 🤖 Use the default header in changelog.

## 5.1.0 - 2026-01-09

### Changed
- 🚚 Update name of Github space from dbus2 to z-galaxy.

### Dependencies
- ⬆️ Update quick-xml to 0.38.

### Fixed
- 🩹 Don't use workspace for local deps.
- 🐛 Raise XML parsing buffer size.

### Other
- 🧑‍💻 Use workspace dependencies.

### Removed
- ➖ Drop `static_assertions` dep.
