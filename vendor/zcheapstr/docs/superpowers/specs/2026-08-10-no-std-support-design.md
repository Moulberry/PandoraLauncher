# no_std support for zcheapstr

Status: approved
Date: 2026-08-10

## Goal

Make zcheapstr usable in `no_std` environments. Nothing in the crate's API needs `std` — every
import resolves in `core` + `alloc` — so the conversion is unconditional and API-identical.

## Semver

Non-breaking: minor bump to 1.1.0, handled by release-plz (`✨` commit triggers a minor
increment; `semver_check = true` independently verifies the release is non-breaking).

Rejected alternatives:

- Major bump (2.0.0): unnecessary, and would force a technically breaking change on zvariant,
  which publicly re-exports `CheapStr` as `zvariant::Str`.
- Optional-`alloc` core-only mode: genuinely breaking (loses the `Owned` variant and
  `String`/`Arc` conversions) with no known user.
- Default `std` feature: an empty placeholder today, since nothing needs `std`.

## Changes

### Crate

- `src/lib.rs`: add `#![no_std]` and `extern crate alloc;`.
- `src/str.rs`: swap `std::` paths for `core::`/`alloc::`:
  - `alloc::{borrow::Cow, string::String, sync::Arc}`
  - `core::{cmp::Ordering, hash::{Hash, Hasher}}`
  - `core::fmt` and `core::ops::Deref` in impl blocks
  - `alloc::borrow::ToOwned` / `alloc::string::ToString` where `to_owned()` / `to_string()`
    are called
  - tests: `core::ptr::eq`; serde_json stays a dev-dependency (tests run on host with std)
- `Cargo.toml`: `serde = { version = "1.0", default-features = false, features = ["derive"],
  optional = true }`. Only borrowed `&str` deserialization is used, so serde's `alloc`/`std`
  features are not needed.
- `README.md`: mention no_std support.

### CI

Add a no_std regression check to `rust.yml`: `cargo check --target x86_64-unknown-none`, once
with `--no-default-features` and once with `--features serde`.

## Accepted caveats

- `alloc::sync::Arc` requires `target_has_atomic = "ptr"`; atomics-less targets (e.g. thumbv6)
  remain unsupported. No `portable-atomic` fallback — that would change the public
  `From<Arc<str>>` type (breaking) and has no known user.
- Dropping serde's default features is observable downstream via feature unification, but is
  conventionally non-breaking; zvariant enables serde's std features itself.

## zbus / zvariant

No change required. zvariant's req `zcheapstr = "1.0.0"` is a caret req and already accepts
1.1.0; Renovate handles the lockfile bump. zvariant is not no_std itself, so it does not need
to require 1.1.

Pre-release sanity check only: build and test zvariant against the local zcheapstr via a
`[patch.crates-io]` override to catch surprises before publishing.

## Testing

- Existing zcheapstr CI: `cargo test` (all features), clippy, fmt, MSRV check.
- New: no_std target check as above.
- zvariant: `cargo check -p zvariant` and its test suite with the patched dependency.
