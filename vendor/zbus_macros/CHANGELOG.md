# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 5.14.0 - 2026-02-22

- New release to go with zbus 5.14.0

## 5.13.1 - 2026-01-11

### Fixed
- 🐛 Allow `out_args` with multiple names for non-tuple types.

### Other
- 🤖 release-plz: Fix formatting of CHANGELOG files.
- 🤖 release-plz: Use the default header in changelog.

## 5.13.0 - 2026-01-09

### Added
- ✨ add special handling for ao DBus signatures. #332
- ✨ Add crate attribute for custom crate paths.

### Changed
- 🎨 Format all files (rust 1.85).
- ♻️ Replace panic with proper Error in introspect_add_output_args.
- ♻️ rename parameters / variables.

### Fixed
- 🐛 zbus_macros shouldn't set features on zbus.
- 🐛 Apply out_args to single outputs in introspection XML. #1599
- 🐛 ignore r# prefix in parameter names. #158
- 🐛 ignore r# prefix in method names. #214

### Other
- 🧱 Fix all clippy warnings (rust 1.85).
- 🧑‍💻 Bump rust version to 1.85.
