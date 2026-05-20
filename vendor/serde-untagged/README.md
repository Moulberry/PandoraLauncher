serde-untagged
==============

[<img alt="github" src="https://img.shields.io/badge/github-dtolnay/serde--untagged-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/dtolnay/serde-untagged)
[<img alt="crates.io" src="https://img.shields.io/crates/v/serde-untagged.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/serde-untagged)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-serde--untagged-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/serde-untagged)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/dtolnay/serde-untagged/ci.yml?branch=master&style=for-the-badge" height="20">](https://github.com/dtolnay/serde-untagged/actions?query=branch%3Amaster)

This crate provides a Serde `Visitor` implementation that is useful for
deserializing untagged enums.

```toml
[dependencies]
serde-untagged = "0.1"
```

<br>

Untagged enum `Deserialize` impls look like this:

```rust
use serde::de::{Deserialize, Deserializer};
use serde_untagged::UntaggedEnumVisitor;

impl<'de> Deserialize<'de> for $MyType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        UntaggedEnumVisitor::new()
            /*
             *
             */
            .deserialize(deserializer)
    }
}
```

Inside the `/* ... */`, we list each type that the untagged enum needs to
support deserializing from, giving a closure that turns the input into $MyType.
The following types are supported:

- bool
- i8, i16, i32, i64, i128, u8, u16, u32, u64, u128
- f32
- f64
- char
- string
- borrowed\_str
- bytes
- borrowed\_bytes
- byte\_buf
- unit
- seq
- map

### Example: string or struct

Cargo's `http.ssl-version` configuration supports deserialization from the
following two representations:

```toml
[http]
ssl-version = "tlsv1.3"
```

```toml
[http]
ssl-version.min = "tlsv1.2"
ssl-version.max = "tlsv1.3"
```

```rust
use serde::de::{Deserialize, Deserializer};
use serde_derive::Deserialize;
use serde_untagged::UntaggedEnumVisitor;

pub enum SslVersionConfig {
    Single(String),
    Range(SslVersionConfigRange),
}

impl<'de> Deserialize<'de> for SslVersionConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        UntaggedEnumVisitor::new()
            .string(|single| Ok(SslVersionConfig::Single(single.to_owned())))
            .map(|map| map.deserialize().map(SslVersionConfig::Range))
            .deserialize(deserializer)
    }
}

#[derive(Deserialize)]
pub struct SslVersionConfigRange {
    pub min: Option<String>,
    pub max: Option<String>,
}
```

### Example: unit variant or bool

Cargo's LTO setting in profiles supports the 5 values `false`, `true`, `"fat"`,
`"thin"`, and `"off"`.

```toml
[profile.release]
lto = "thin"
```

```rust
use serde::de::{Deserialize, Deserializer, IntoDeserializer};
use serde_derive::Deserialize;
use serde_untagged::UntaggedEnumVisitor;

pub enum LinkTimeOptimization {
    Enabled(bool),
    Enum(LinkTimeOptimizationString),
}

impl<'de> Deserialize<'de> for LinkTimeOptimization {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        UntaggedEnumVisitor::new()
            .bool(|b| Ok(LinkTimeOptimization::Enabled(b)))
            .string(|string| {
                let de = string.into_deserializer();
                LinkTimeOptimizationString::deserialize(de).map(LinkTimeOptimization::Enum)
            })
            .deserialize(deserializer)
    }
}

#[derive(Deserialize)]
#[serde(rename = "lowercase")]
pub enum LinkTimeOptimizationString {
    Fat,
    Thin,
    Off,
}
```

Since `lto = true` means the same thing as `lto = "fat"` to Cargo, there are
really only 4 distinct options. This type could be implemented alternatively as:

```rust
use serde::de::{Deserialize, Deserializer, Unexpected};
use serde_untagged::UntaggedEnumVisitor;

pub enum LinkTimeOptimization {
    ThinLocal,  // false
    Fat,        // true or "fat"
    Thin,       // "thin"
    Off,        // "off"
}

impl<'de> Deserialize<'de> for LinkTimeOptimization {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        UntaggedEnumVisitor::new()
            .bool(|b| match b {
                false => Ok(LinkTimeOptimization::ThinLocal),
                true => Ok(LinkTimeOptimization::Fat),
            })
            .string(|string| match string {
                "fat" => Ok(LinkTimeOptimization::Fat),
                "thin" => Ok(LinkTimeOptimization::Thin),
                "off" => Ok(LinkTimeOptimization::Off),
                _ => Err(serde::de::Error::invalid_value(
                    Unexpected::Str(string),
                    &r#""fat" or "thin" or "off""#,
                )),
            })
            .deserialize(deserializer)
    }
}
```

<br>

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
