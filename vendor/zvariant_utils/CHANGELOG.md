# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 4.2.0 - 2026-08-18

### Deprecated
- 🗑️ Deprecate GVariant support in zvariant.

### Fixed
- 🐛 Support zvariant_utils/gvariant being enabled externally. #1910

## 4.1.0 - 2026-08-16

### Added
- ✨ Parameterize attribute parsing by list names.

### Changed
- ♻️ Move Value & Dict derive codegen to zvariant_utils.
- ♻️ Move Type derive & signature! codegen to zvariant_utils.

### Fixed
- 🥅 Propagate field-attribute errors in dict Value codegen.

## 4.0.0 - 2026-08-09

### Breaking
- ⬆️ Update syn to v3. Syn types appear in this crate's public API, hence the major
  version bump.

## 3.5.0 - 2026-07-07

### Fixed
- 🐛 Change bool alignment to 1 in GVariant.

### Security
- 🔒️ Bound struct and array nesting when parsing signatures.

## 3.4.0 - 2026-05-27

### Performance
- ⚡️ Add Fields::get(i) for constant-time positional access.

## 3.3.1 - 2026-04-26

### Documentation
- 📝 Configure docs.rs to build for all supported targets.

### Other
- 🤖 Fix formatting of CHANGELOG files.
- 🤖 Use the default header in changelog.

## 3.3.0 - 2026-01-09

### Added
- ✨ Add crate_path -> crate attribute mapping in def_attrs.

### Changed
- 🎨 Format all files (rust 1.85).

### Documentation
- 📝 Document signature! macro and update Signature docs.
