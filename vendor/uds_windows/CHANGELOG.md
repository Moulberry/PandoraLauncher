# Changelog

## [1.2.1] - 2026-03-14

- Require `windows-sys >= 0.60` since `BOOL` moved to `windows_sys::core` in 0.60

## [1.2.0] - 2026-03-04

- Migrate from unmaintained `winapi` crate to official Microsoft `windows-sys` crate
- Set minimum Rust version (MSRV) to 1.85
- Update to Rust edition 2021
- Remove unused `NetInt` trait
- Fix clippy warnings and use explicit lifetime syntax

## [1.1.0] - 2022-12-14

- Support for I/O Safety types and traits: `AsSocket`, `BorrowedSocket`, and `OwnedSocket`
- Added safety comments above unsafe blocks within safe functions
- Fix clippy warnings and removal of null pointer dereference
