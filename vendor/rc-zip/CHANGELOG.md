# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [5.4.1](https://github.com/bearcove/rc-zip/compare/rc-zip-v5.4.0...rc-zip-v5.4.1) - 2025-11-19

### Other

- *(rc-zip)* manually implement `thiserror` code

## [5.4.0](https://github.com/bearcove/rc-zip/compare/rc-zip-v5.3.7...rc-zip-v5.4.0) - 2025-11-18

### Added

- *(rc-zip)* re-export common items
- *(rc-zip)* add `rc_zip::error::Result<T>` type alias
- *(rc-zip)* add `EntryKind::is_*(self)` helpers

### Other

- *(rc-zip)* simplify `Mode` display impl

## [5.3.7](https://github.com/bearcove/rc-zip/compare/rc-zip-v5.3.6...rc-zip-v5.3.7) - 2025-11-16

### Other

- Update
- *(rc-zip)* split `corpus` into its own crate
- add trailing zip to corpus

## [5.3.6](https://github.com/bearcove/rc-zip/compare/rc-zip-v5.3.5...rc-zip-v5.3.6) - 2025-11-14

### Other

- *(rc-zip)* split `corpus` into its own crate
- add trailing zip to corpus

## [5.3.5](https://github.com/bearcove/rc-zip/compare/rc-zip-v5.3.4...rc-zip-v5.3.5) - 2025-11-05

### Other

- Merge branch 'main' into chore/cargo_toml_repository

## [5.3.4](https://github.com/bearcove/rc-zip/compare/rc-zip-v5.3.3...rc-zip-v5.3.4) - 2025-11-04

### Fixed

- *(rc-zip/corpus)* ignore failures when installing global subscriber

### Other

- Merge pull request #126 from CosmicHorrorDev/workspace-deps
- Merge pull request #127 from CosmicHorrorDev/placate-clippy
- `$ cargo fmt`
- turn off `tracing`s default features
- *(rc-zip)* gate tests on `corpus` feature

## [5.3.3](https://github.com/bearcove/rc-zip/compare/rc-zip-v5.3.2...rc-zip-v5.3.3) - 2025-10-17

### Other

- add changelog entry
- manually implement `num_enum` code

### Other

- Replace `num_enum` with a manual implementation ([#120](https://github.com/bearcove/rc-zip/pull/120))

## [5.3.2](https://github.com/bearcove/rc-zip/compare/rc-zip-v5.3.1...rc-zip-v5.3.2) - 2025-08-30

### Other

- Upgrade bzip2 to v0.6.0
- fix typos in the `Archive` docs

## [5.3.1](https://github.com/bearcove/rc-zip/compare/rc-zip-v5.3.0...rc-zip-v5.3.1) - 2025-03-02

### Other

- Bump bzip2 to v0.5.0

## [5.3.0](https://github.com/bearcove/rc-zip/compare/rc-zip-v5.2.0...rc-zip-v5.3.0) - 2025-02-05

### Added

- derive Eq and PartialEq for EntryKind (#95)

## [5.2.0](https://github.com/bearcove/rc-zip/compare/rc-zip-v5.1.3...rc-zip-v5.2.0) - 2024-12-17

### Added

- Export DecompressOutcome

## [5.1.3](https://github.com/bearcove/rc-zip/compare/rc-zip-v5.1.2...rc-zip-v5.1.3) - 2024-09-17

### Other

- Remove unnecessary deps
- Add/fix logo attribution

## [5.1.2](https://github.com/bearcove/rc-zip/compare/rc-zip-v5.1.1...rc-zip-v5.1.2) - 2024-09-05

### Other
- Update logo attribution

## [5.1.1](https://github.com/bearcove/rc-zip/compare/rc-zip-v5.1.0...rc-zip-v5.1.1) - 2024-09-04

### Other
- Add rc-zip logo to main crate, too

## [5.1.0](https://github.com/fasterthanlime/rc-zip/compare/rc-zip-v5.0.1...rc-zip-v5.1.0) - 2024-03-19

### Added
- Measure code coverage differently ([#79](https://github.com/fasterthanlime/rc-zip/pull/79))
- Run one-byte-read tests in CI in release ([#77](https://github.com/fasterthanlime/rc-zip/pull/77))
- Resolve winnow + chrono deprecations ([#70](https://github.com/fasterthanlime/rc-zip/pull/70))

### Fixed
- lzma_dec: count all input in outcome.bytes_read
- In Entry FSM, don't recurse infinitely if buffer doesn't contain full local header
- Fix doc comment for read_offset

### Other
- Fix zstd bug similar to lzma bug

## [5.0.1](https://github.com/fasterthanlime/rc-zip/compare/rc-zip-v5.0.0...rc-zip-v5.0.1) - 2024-03-12

### Other
- Point rc-zip crate's README to its own README? release-plz is confused

## [5.0.0](https://github.com/fasterthanlime/rc-zip/compare/rc-zip-v4.0.0...rc-zip-v5.0.0) - 2024-03-12

### Added
- Add fuzz target, fix several panics ([#67](https://github.com/fasterthanlime/rc-zip/pull/67))

### Other
- Decomplexify match some more
- Decomplexify match

## [4.0.0](https://github.com/fasterthanlime/rc-zip/compare/rc-zip-v3.0.0...rc-zip-v4.0.0) - 2024-02-05

### Added
- [**breaking**] Introduce `ReadZipStreaming` trait ([#62](https://github.com/fasterthanlime/rc-zip/pull/62))

### Other
- Remove unused dependencies

## [3.0.0](https://github.com/fasterthanlime/rc-zip/compare/rc-zip-v2.0.1...rc-zip-v3.0.0) - 2024-02-02

### Other
- Introduce rc-zip-sync, rc-zip-tokio ([#60](https://github.com/fasterthanlime/rc-zip/pull/60))
