# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [1.74.2] - 2026-07-10
### Added
- Adds encoding images using DWAA/DWAB compression, without any API change.


## [1.74.1] - 2026-07-08
### Added
- Adds decoding images that have DWAA/DWAB compression, without any API change.
- Introduces safe SIMD runtime dispatch using `pulp`, for DWA decoding.