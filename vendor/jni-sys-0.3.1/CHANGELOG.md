# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1] - 2026-03-22

### Added

- GetModule was added to `JNINativeInterface` ([#22](https://github.com/jni-rs/jni-sys/pull/22))
- `JNI_VERSION_{9,10,19,20,21,24}` version definitions were added

### Changed

- Compatible types are now re-exported from `jni-sys 0.4` to make it easier for `jobject` references
  to be passed around between APIs depending on different versions of `jni-sys`

## [0.3.0] - 2017-07-20

### Changed

- Changed jvalue into a union

[unreleased]: https://github.com/jni-rs/jni-sys/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/jni-rs/jni-sys/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/jni-rs/jni-sys/compare/v0.2.5...v0.3.0
