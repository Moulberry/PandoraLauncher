# Str Indices

[![Latest Release][crates-io-badge]][crates-io-url]
[![Documentation][docs-rs-img]][docs-rs-url]

Count and convert between different indexing schemes on utf8 string slices.

The following schemes are currently supported:

* Chars (Unicode scalar values).
* UTF16 code units.
* Lines, with three options for recognized line break characters:
    * Line feed only.
    * Line feed and carriage return.
    * All Unicode line break characters, as specified in [Unicode Annex #14](https://www.unicode.org/reports/tr14/).


## Feature Flags

The `simd` feature flag (enabled by default) enables explicit SIMD
optimizations on supported platforms.  Disabling it will use the
fallback scalar code path on all platforms.  This feature flag only
affects performance, not behavior.


## Unsafe Code

Str Indices uses unsafe code for performance optimizations, primarily
for SIMD intrinsics.  The unsafe code is kept minimal, mostly
compartmentalized, and easy to audit.  The entire code base is also fuzz
tested.

If you would like to run the fuzz testing suite yourself, install Rust
nightly and [cargo fuzz](https://github.com/rust-fuzz/cargo-fuzz/)
and run the following command from the repository root:

```
cargo +nightly fuzz run random_strings
```

Additional runs of the fuzz testing suite are very much appreciated, as
is general auditing of the code base.  If you find any unsoundness,
_please_ file an issue!


## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.


## Contributing

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in Str Indices by you will be licensed as above,
without any additional terms or conditions.

This crate is no-std, doesn't allocate, and has zero dependencies, and
aims to remain that way.  Please adhere to this in any submitted
contributions.


[crates-io-badge]: https://img.shields.io/crates/v/str_indices.svg
[crates-io-url]: https://crates.io/crates/str_indices
[docs-rs-img]: https://docs.rs/str_indices/badge.svg
[docs-rs-url]: https://docs.rs/str_indices
