# no_std Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make zcheapstr unconditionally `#![no_std]` (requiring only `core` + `alloc`), with an
API-identical surface, released as a non-breaking 1.1.0.

**Architecture:** Single-crate refactor: swap `std::` paths for `core::`/`alloc::`, relax the
serde dependency to `default-features = false`, and guard the property in CI with a
`x86_64-unknown-none` target check. No zbus/zvariant code changes — only a local pre-release
compatibility verification there.

**Tech Stack:** Rust (edition 2024, MSRV 1.87), serde (optional), GitHub Actions, release-plz.

Spec: `docs/superpowers/specs/2026-08-10-no-std-support-design.md`.

## Global Constraints

- MSRV is 1.87; edition 2024. Do not use newer language features.
- 100 chars per line in all files, including comments (`comment_width = 100`).
- Formatting uses nightly-only rustfmt options: always `cargo +nightly fmt`, never plain stable
  `cargo fmt` warnings ignored.
- Commit messages: gimoji prefix copied verbatim from the gitmoji set (keep U+FE0F variation
  selectors), single space after emoji, commitlint-enforced. No `Signed-off-by`. Add
  `Assisted-by: Claude Fable 5 (claude-fable-5)` trailer.
- GPG signing is broken on this machine: commit with `--no-gpg-sign`.
- Work happens on the existing `no-std-support` branch in `/home/zeenix/checkout/z-galaxy/zcheapstr`.
- The public API must not change: `cargo semver-checks` runs at release time
  (`semver_check = true` in release-plz.toml) and must pass as non-breaking.

---

### Task 1: Convert the crate to unconditional no_std

**Files:**
- Modify: `src/lib.rs`
- Modify: `src/str.rs`
- Modify: `Cargo.toml`
- Modify: `README.md`

**Interfaces:**
- Consumes: nothing (first task).
- Produces: a crate that passes `cargo check --target x86_64-unknown-none` with and without
  `--all-features`. Task 2's CI job runs exactly those commands.

- [ ] **Step 1: Install the no_std check target**

Run: `rustup target add x86_64-unknown-none`

- [ ] **Step 2: Verify the no_std check fails today (RED)**

Run: `cargo check --target x86_64-unknown-none`
Expected: FAIL with `can't find crate for `std``.

- [ ] **Step 3: Convert `src/lib.rs`**

Full new content (only `#![no_std]` and `extern crate alloc;` are added):

```rust
#![no_std]
#![deny(rust_2018_idioms)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/z-galaxy/zbus/9f7a90d2b594ddc48b7a5f39fda5e00cd56a7dfb/logo.png"
)]
#![doc = include_str!("../README.md")]
#![doc(test(attr(warn(unused), deny(warnings))))]

extern crate alloc;

mod str;
pub use crate::str::CheapStr;
```

Note: doc tests (the README example) are compiled as separate host crates that link `std`, so
they are unaffected by `#![no_std]` on the library.

- [ ] **Step 4: Convert imports and paths in `src/str.rs`**

Replace the top-of-file imports (lines 1–8):

```rust
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use alloc::{
    borrow::{Cow, ToOwned},
    string::{String, ToString},
    sync::Arc,
};
use core::{
    cmp::Ordering,
    hash::{Hash, Hasher},
};
```

`ToOwned` is needed for `s.to_owned()` in `into_owned()`; `ToString` for `s.to_string()` in
`From<CheapStr<'a>> for String`.

Then replace the remaining inline `std::` paths with `core::` equivalents:

- `impl std::ops::Deref for CheapStr<'_>` → `impl core::ops::Deref for CheapStr<'_>`
- The `Debug` impl becomes:

```rust
impl core::fmt::Debug for CheapStr<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_str(), f)
    }
}
```

- The `Display` impl becomes:

```rust
impl core::fmt::Display for CheapStr<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self.as_str(), f)
    }
}
```

In the `tests` module, the `std` prelude is gone, so add the needed imports below
`use super::CheapStr;`:

```rust
use alloc::string::{String, ToString};
```

In the `serde_tests` module, add below `use super::CheapStr;`:

```rust
use alloc::string::String;
```

and change `std::ptr::eq` to `core::ptr::eq` in `serde_borrowed_deserialization`.

- [ ] **Step 5: Relax the serde dependency and add the no-std category in `Cargo.toml`**

The serde dependency becomes (only borrowed `&str` deserialization is used, so serde's
`std`/`alloc` features are unnecessary):

```toml
serde = { version = "1.0", default-features = false, features = ["derive"], optional = true }
```

Add `"no-std"` to the categories list:

```toml
categories = ["data-structures", "memory-management", "no-std", "text-processing"]
```

Leave `serde_json` (dev-dependency) untouched: tests run on the host with `std`, and dev-dep
features do not leak into the lib build with the edition-2024 feature resolver.

- [ ] **Step 6: Document no_std support in `README.md`**

After the "API is provided to convert from, and to a [`&str`] and [`String`]." paragraph
(line 17), add:

```markdown
The crate is `no_std`: it only requires [`alloc`], not `std` (this includes the `serde`
feature).
```

and add the link definition next to the existing ones at the bottom:

```markdown
[`alloc`]: https://doc.rust-lang.org/alloc/
```

- [ ] **Step 7: Verify the no_std check passes (GREEN)**

Run:
```bash
cargo check --target x86_64-unknown-none
cargo check --target x86_64-unknown-none --all-features
```
Expected: both PASS. (`default = []`, so the first command also covers `--no-default-features`.)

- [ ] **Step 8: Verify nothing regressed on the host**

Run:
```bash
cargo test --all-features
cargo test
cargo clippy --all-features -- -D warnings
cargo +nightly fmt
git diff --stat
```
Expected: all tests pass (including README doc tests), clippy clean, and fmt produces no
further changes (if it does, include them).

- [ ] **Step 9: Commit**

```bash
git add src/lib.rs src/str.rs Cargo.toml README.md
git commit --no-gpg-sign -m "✨ Add no_std support

Everything the crate uses lives in core + alloc, so the conversion is
unconditional and the API is unchanged. serde loses its default features;
only borrowed &str deserialization is used, so serde's std/alloc features
are not needed.

Assisted-by: Claude Fable 5 (claude-fable-5)"
```

The `✨` prefix makes release-plz bump the minor version (→ 1.1.0) and groups the change under
"Added" in the changelog.

---

### Task 2: Guard no_std in CI

**Files:**
- Modify: `.github/workflows/rust.yml`

**Interfaces:**
- Consumes: the commands proven green in Task 1, Step 7.
- Produces: a `no_std` CI job; nothing downstream depends on it.

- [ ] **Step 1: Add the no_std job**

Append to `.github/workflows/rust.yml`, matching the style of the existing jobs (e.g. after the
`doc_build` job):

```yaml
  no_std:
    runs-on: ubuntu-latest
    env:
      RUSTFLAGS: -D warnings
    steps:
      - uses: actions/checkout@v6
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: stable
          targets: x86_64-unknown-none
      - uses: Swatinem/rust-cache@v2
      - name: Check no_std build
        run: |
          cargo --locked check --target x86_64-unknown-none
          cargo --locked check --target x86_64-unknown-none --all-features
```

- [ ] **Step 2: Validate the workflow locally**

Run the job's commands verbatim (they must still pass):
```bash
cargo --locked check --target x86_64-unknown-none
cargo --locked check --target x86_64-unknown-none --all-features
```
Expected: PASS. Also re-read the YAML diff for indentation consistency with sibling jobs
(2-space nesting, same action versions: `actions/checkout@v6`, `dtolnay/rust-toolchain@master`,
`Swatinem/rust-cache@v2`).

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/rust.yml
git commit --no-gpg-sign -m "👷 Check no_std build in CI

Assisted-by: Claude Fable 5 (claude-fable-5)"
```

`👷` is CI-only, so release-plz correctly does not treat it as a version-bumping change.

---

### Task 3: Verify zvariant compatibility (no commits)

**Files:**
- Temporarily modify (then revert): `/home/zeenix/checkout/z-galaxy/zbus/Cargo.toml`,
  `/home/zeenix/checkout/z-galaxy/zbus/Cargo.lock`

**Interfaces:**
- Consumes: the converted zcheapstr working tree from Tasks 1–2.
- Produces: verification evidence only. Nothing is committed to zbus; Renovate handles the
  lockfile bump there after the 1.1.0 release.

- [ ] **Step 1: Record the zbus working tree state**

Run: `git -C /home/zeenix/checkout/z-galaxy/zbus status --porcelain`
Expected: empty (clean). If not clean, note the pre-existing changes and do not touch those
files beyond the patch below.

- [ ] **Step 2: Patch zbus to use the local zcheapstr**

Append to `/home/zeenix/checkout/z-galaxy/zbus/Cargo.toml` (the workspace root):

```toml
[patch.crates-io]
zcheapstr = { path = "../zcheapstr" }
```

- [ ] **Step 3: Build and test zvariant against it**

Run (from the zbus checkout; `cargo -C` is nightly-only, so `cd` first):
```bash
cd /home/zeenix/checkout/z-galaxy/zbus
cargo check -p zvariant --all-features
cargo test -p zvariant
```
Expected: both PASS, proving the serde `default-features = false` change and the no_std
conversion are invisible to zvariant.

- [ ] **Step 4: Revert the patch**

Run:
```bash
git -C /home/zeenix/checkout/z-galaxy/zbus checkout -- Cargo.toml Cargo.lock
git -C /home/zeenix/checkout/z-galaxy/zbus status --porcelain
```
Expected: status matches what Step 1 recorded.

- [ ] **Step 5: Report results**

Summarize the verification outcome (pass/fail, any warnings) in the final report to the user.
No commit in either repository for this task.
