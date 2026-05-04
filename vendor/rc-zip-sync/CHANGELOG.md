# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [4.4.2](https://github.com/bearcove/rc-zip/compare/rc-zip-sync-v4.4.1...rc-zip-sync-v4.4.2) - 2025-11-27

### Other

- Merge pull request #138 from CosmicHorrorDev/extract-mt
- *(rc-zip-sync)* add a multi-threaded byte count example

## [4.4.1](https://github.com/bearcove/rc-zip/compare/rc-zip-sync-v4.4.0...rc-zip-sync-v4.4.1) - 2025-11-19

### Other

- updated the following local packages: rc-zip, rc-zip-corpus

## [4.4.0](https://github.com/bearcove/rc-zip/compare/rc-zip-sync-v4.3.6...rc-zip-sync-v4.4.0) - 2025-11-18

### Added

- *(rc-zip)* re-export common items

## [4.3.6](https://github.com/bearcove/rc-zip/compare/rc-zip-sync-v4.3.5...rc-zip-sync-v4.3.6) - 2025-11-16

### Other

- check `Files` for streaming corpus
- (poorly) wire up streaming error checking
- Update
- *(rc-zip)* split `corpus` into its own crate
- *(rc-zip-sync)* add a self-extracting zip file example

## [4.3.5](https://github.com/bearcove/rc-zip/compare/rc-zip-sync-v4.3.4...rc-zip-sync-v4.3.5) - 2025-11-14

### Other

- *(rc-zip)* split `corpus` into its own crate
- *(rc-zip-sync)* add a self-extracting zip file example

## [4.3.4](https://github.com/bearcove/rc-zip/compare/rc-zip-sync-v4.3.3...rc-zip-sync-v4.3.4) - 2025-11-05

### Other

- Merge pull request #124 from CosmicHorrorDev/archive-oob
- Merge branch 'main' into chore/cargo_toml_repository

## [4.3.3](https://github.com/bearcove/rc-zip/compare/rc-zip-sync-v4.3.2...rc-zip-sync-v4.3.3) - 2025-11-04

### Other

- Merge pull request #126 from CosmicHorrorDev/workspace-deps
- Merge pull request #127 from CosmicHorrorDev/placate-clippy
- `$ cargo fmt`
- turn off `tracing`s default features
- *(rc-zip-sync)* tests need `rc-zip/corpus`

## [4.3.2](https://github.com/bearcove/rc-zip/compare/rc-zip-sync-v4.3.1...rc-zip-sync-v4.3.2) - 2025-10-17

### Other

- updated the following local packages: rc-zip

## [4.3.1](https://github.com/bearcove/rc-zip/compare/rc-zip-sync-v4.3.0...rc-zip-sync-v4.3.1) - 2025-08-30

### Other

- updated the following local packages: rc-zip

## [4.3.0](https://github.com/bearcove/rc-zip/compare/rc-zip-sync-v4.2.6...rc-zip-sync-v4.3.0) - 2025-04-18

### Added

- *(rc-zip-sync)* make EntryReader public

## [4.2.6](https://github.com/bearcove/rc-zip/compare/rc-zip-sync-v4.2.5...rc-zip-sync-v4.2.6) - 2025-03-02

### Other

- updated the following local packages: rc-zip

## [4.2.5](https://github.com/bearcove/rc-zip/compare/rc-zip-sync-v4.2.4...rc-zip-sync-v4.2.5) - 2025-02-05

### Other

- updated the following local packages: rc-zip

## [4.2.4](https://github.com/bearcove/rc-zip/compare/rc-zip-sync-v4.2.3...rc-zip-sync-v4.2.4) - 2024-12-17

### Other

- updated the following local packages: rc-zip

## [4.2.3](https://github.com/bearcove/rc-zip/compare/rc-zip-sync-v4.2.2...rc-zip-sync-v4.2.3) - 2024-10-08

### Other

- Remove dead example
- Make jean a workspace member rather than an example

## [4.2.2](https://github.com/bearcove/rc-zip/compare/rc-zip-sync-v4.2.1...rc-zip-sync-v4.2.2) - 2024-09-17

### Other

- Remove unnecessary deps
- Add/fix logo attribution

## [4.2.1](https://github.com/bearcove/rc-zip/compare/rc-zip-sync-v4.2.0...rc-zip-sync-v4.2.1) - 2024-09-05

### Other
- Update logo attribution

## [4.2.0](https://github.com/bearcove/rc-zip/compare/rc-zip-sync-v4.1.0...rc-zip-sync-v4.2.0) - 2024-09-04

### Added
- Add logo

## [4.1.0](https://github.com/fasterthanlime/rc-zip/compare/rc-zip-sync-v4.0.1...rc-zip-sync-v4.1.0) - 2024-03-19

### Added
- Measure code coverage differently ([#79](https://github.com/fasterthanlime/rc-zip/pull/79))
- Run one-byte-read tests in CI in release ([#77](https://github.com/fasterthanlime/rc-zip/pull/77))
- rc-zip-tokio: Re-use cursor if it's at the right offset already ([#71](https://github.com/fasterthanlime/rc-zip/pull/71))

### Fixed
- lzma_dec: count all input in outcome.bytes_read

### Other
- release ([#68](https://github.com/fasterthanlime/rc-zip/pull/68))

## [4.0.1](https://github.com/fasterthanlime/rc-zip/compare/rc-zip-sync-v4.0.0...rc-zip-sync-v4.0.1) - 2024-03-12

### Other
- updated the following local packages: rc-zip, rc-zip

## [4.0.0](https://github.com/fasterthanlime/rc-zip/compare/rc-zip-sync-v3.0.0...rc-zip-sync-v4.0.0) - 2024-02-05

### Added
- [**breaking**] Introduce `ReadZipStreaming` trait ([#62](https://github.com/fasterthanlime/rc-zip/pull/62))

### Fixed
- Windows build was failing

### Other
- Remove unused dependencies

## [3.0.0](https://github.com/fasterthanlime/rc-zip/releases/tag/rc-zip-sync-v3.0.0) - 2024-02-02

### Other
- Bump rc-zip-sync & rc-zip-tokio
- Introduce rc-zip-sync, rc-zip-tokio ([#60](https://github.com/fasterthanlime/rc-zip/pull/60))
