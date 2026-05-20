# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.4] - 2024-05-04

### Fixed

- Suppport for trailing commas when calling `formatx!` macro. ([#11](https://github.com/clitic/formatx/pull/11))

## [0.2.3] - 2024-12-15

### Fixed

- clippy warnings. ([#6](https://github.com/clitic/formatx/pull/6))

## [0.2.2] - 2023-12-03

### Fixed

- Use `std::result::Result` in `formatx!` macro. ([#4](https://github.com/clitic/formatx/pull/4))

## [0.2.1] - 2023-02-03

### Fixed

- `formatx!` macro clippy warnings.

## [0.2.0] - 2022-12-06

### Added

- Better `formatx!` macro expansion.

### Changed

- `formatx!(template: expr)` macro returns `Result<String, formatx::Error>`.

## [0.1.4] - 2022-08-15

## [0.1.3] - 2022-08-05

## [0.1.2] - 2022-08-05

## [0.1.1] - 2022-08-05

## [0.1.0] - 2022-08-03

[Unreleased]: https://github.com/clitic/formatx/compare/0.2.4...HEAD
[0.2.4]: https://github.com/clitic/formatx/compare/v0.2.3...0.2.4
[0.2.3]: https://github.com/clitic/formatx/compare/v0.2.2...0.2.3
[0.2.2]: https://github.com/clitic/formatx/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/clitic/formatx/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/clitic/formatx/compare/c672c19...v0.2.0
[0.1.4]: https://github.com/clitic/formatx/compare/b2ee21f...c672c19
[0.1.3]: https://github.com/clitic/formatx/compare/39eb3ee...b2ee21f
[0.1.2]: https://github.com/clitic/formatx/compare/0f282e2...39eb3ee
[0.1.1]: https://github.com/clitic/formatx/compare/454cd82...0f282e2
[0.1.0]: https://github.com/clitic/formatx/compare/0a4cc2d...454cd82
