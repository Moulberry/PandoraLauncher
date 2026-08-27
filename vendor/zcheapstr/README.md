# zcheapstr

[![](https://docs.rs/zcheapstr/badge.svg)](https://docs.rs/zcheapstr/) [![](https://img.shields.io/crates/v/zcheapstr)](https://crates.io/crates/zcheapstr)

This crate provides a single type, `CheapStr`: a string wrapper that is similar to the
[`Cow<'_, str>`][`Cow`] type, but it:

* is specialized for strings.
* treats `&'static str` as a separate type. This allows you to avoid allocations and copying when
  turning a `CheapStr` instance created from a `&'static str` into an owned version in generic code
  that doesn't/can't assume the inner lifetime of the source `CheapStr` instance.
* stores owned strings in an [`Arc`], so `Clone` never copies or allocates: it either copies a
  reference or increments a reference count.
* is immutable. Consequently, unlike [`Cow`], it is *not* a copy-on-write type: there is no way to
  get a mutable reference to the underlying string.

API is provided to convert from, and to a [`&str`] and [`String`].

The crate is `no_std`: it only requires [`alloc`], not `std` (this includes the `serde`
feature).

**Status:** Stable. This code was extracted from (and is battle-tested in) the [zvariant] crate,
where the type was originally named `Str`.

## Example code

```rust
use zcheapstr::CheapStr;

// Borrowed data: no allocation, and `Clone` is cheap.
let borrowed = CheapStr::from("hello");
assert_eq!(borrowed.as_str(), "hello");
assert_eq!(borrowed, "hello");

// `&'static str` is kept as-is, even in a `const` context.
const GREETING: CheapStr<'static> = CheapStr::from_static("hi");
assert_eq!(GREETING.as_str(), "hi");
// Turning it into an owned instance is free.
assert_eq!(GREETING.to_owned(), "hi");

// Borrowed data is only copied when an owned instance is actually needed.
let owned: CheapStr<'static> = borrowed.to_owned();
assert_eq!(owned, "hello");

// Owned data is reference-counted, so neither `into_owned` nor `Clone` copies it.
let from_string = CheapStr::from(String::from("world"));
let owned: CheapStr<'static> = from_string.into_owned();
let cheap_clone = owned.clone(); // Just increments a reference count.
assert_eq!(String::from(owned), "world");
assert_eq!(cheap_clone, "world");
```

## Features

All features are disabled by default.

| Feature | Description |
| ---     | ----------- |
| serde   | Implement [serde]'s `Serialize` and `Deserialize` for `CheapStr` |

With the `serde` feature enabled, `CheapStr` is serialized as a plain string. Deserialization always
borrows from the input data: `CheapStr<'de>` implements `Deserialize<'de>` but *not*
`DeserializeOwned`, so deserialization fails when the deserializer cannot provide a borrowed
`&str` (for example, JSON strings containing escape sequences, or reader-based deserialization).
If you need owned deserialization, deserialize into a `String` (or `Cow<'_, str>`) and convert it
via `CheapStr::from`.

[`alloc`]: https://doc.rust-lang.org/alloc/
[`Cow`]: https://doc.rust-lang.org/std/borrow/enum.Cow.html
[`Arc`]: https://doc.rust-lang.org/std/sync/struct.Arc.html
[`&str`]: https://doc.rust-lang.org/std/str/index.html
[`String`]: https://doc.rust-lang.org/std/string/struct.String.html
[zvariant]: https://crates.io/crates/zvariant
[serde]: https://crates.io/crates/serde
