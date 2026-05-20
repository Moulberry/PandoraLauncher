# Versions

[![Tests](https://github.com/fosskers/rs-versions/workflows/Tests/badge.svg)](https://github.com/fosskers/rs-versions/actions)
[![](https://img.shields.io/crates/v/versions.svg)](https://crates.io/crates/versions)

<!-- cargo-rdme start -->

A library for parsing and comparing software version numbers.

We like to give version numbers to our software in a myriad of different
ways. Some ways follow strict guidelines for incrementing and comparison.
Some follow conventional wisdom and are generally self-consistent. Some are
just plain asinine. This library provides a means of parsing and comparing
*any* style of versioning, be it a nice Semantic Version like this:

> 1.2.3-r1

...or a monstrosity like this:

> 2:10.2+0.0093r3+1-1

## Usage

If you're parsing several version numbers that don't follow a single scheme
(say, as in system packages), then use the [`Versioning`] type and its
parser [`Versioning::new`]. Otherwise, each main type - [`SemVer`],
[`Version`], or [`Mess`] - can be parsed on their own via the `new` method
(e.g. [`SemVer::new`]).

## Examples

```rust
use versions::Versioning;

let good = Versioning::new("1.6.0").unwrap();
let evil = Versioning::new("1.6.0a+2014+m872b87e73dfb-1").unwrap();

assert!(good.is_ideal());   // It parsed as a `SemVer`.
assert!(evil.is_complex()); // It parsed as a `Mess`.
assert!(good > evil);       // We can compare them anyway!
```

## Version Constraints

Tools like `cargo` also allow version constraints to be prepended to a
version number, like in `^1.2.3`.

```rust
use versions::{Requirement, Versioning};

let req = Requirement::new("^1.2.3").unwrap();
let ver = Versioning::new("1.2.4").unwrap();
assert!(req.matches(&ver));
```

In this case, the incoming version `1.2.4` satisfies the "caret" constraint,
which demands anything greater than or equal to `1.2.3`.

See the [`Requirement`] type for more details.

## Usage with `nom`

In constructing your own [`nom`](https://lib.rs/nom) parsers, you can
integrate the parsers used for the types in this crate via
[`Versioning::parse`], [`SemVer::parse`], [`Version::parse`], and
[`Mess::parse`].

## Features

You can enable [`Serde`](https://serde.rs/) support for serialization and
deserialization with the `serde` feature.

By default the version structs are serialized/deserialized as-is. If instead
you'd like to deserialize directly from a raw version string like `1.2.3`,
see [`Versioning::deserialize_pretty`].

<!-- cargo-rdme end -->

