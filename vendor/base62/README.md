# base62

A fast, zero-dependency base62 encoder/decoder library for Rust, typically used in URL shorteners. It supports both standard [0-9A-Za-z] and alternative [0-9a-zA-Z] variants.

[![Build status](https://github.com/fbernier/base62/workflows/ci/badge.svg)](https://github.com/fbernier/base62/actions)
[![Crates.io](https://img.shields.io/crates/v/base62.svg)](https://crates.io/crates/base62)
[![Docs](https://docs.rs/base62/badge.svg)](https://docs.rs/base62)

## Features

- `no_std` compatible with optional `alloc` and `std` support
- Encodes integers up to `u128`
- Zero-copy decoding
- Efficient string handling
- Two encoding variants:
  - Standard [0-9A-Za-z]
  - Alternative [0-9a-zA-Z]

## Usage

Add this to your `Cargo.toml`:
```toml
[dependencies]
base62 = "2"
```

### Basic Example

```rust
use base62;

// Encoding
let encoded = base62::encode(1234567890);
assert_eq!(encoded, "1LY7VK");

// Decoding
let decoded = base62::decode("1LY7VK").unwrap();
assert_eq!(decoded, 1234567890);
```

### No-std Usage

The crate works in `no_std` environments by default:

```rust
#![no_std]
use base62;

// Encode into a fixed buffer
let mut buf = [0u8; 22];  // Maximum size needed for u128
let len = base62::encode_bytes(1234567890, &mut buf).unwrap();
assert_eq!(&buf[..len], b"1LY7VK");

// Decode from bytes
let decoded = base62::decode(&buf[..len]).unwrap();
assert_eq!(decoded, 1234567890);
```

## Feature Flags

- `alloc`: Enables String allocation support (enabled by default)
- `std`: Enables std::io traits support

## Performance

The library is optimized for both encoding and decoding performance:
- Zero-copy decoding
- Efficient buffer management
- Direct string manipulation for optimal performance when appending

## License

Licensed under the MIT license. See LICENSE for details.
