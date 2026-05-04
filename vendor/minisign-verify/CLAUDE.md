# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`minisign-verify` is a small, zero-dependency Rust crate to verify signatures created with Minisign (https://jedisct1.github.io/minisign/). The library is designed to be lightweight and focused solely on signature verification, not creation.

## Common Commands

### Building

```bash
# Build the library in debug mode
cargo build

# Build in release mode (with optimizations)
cargo build --release
```

### Testing

```bash
# Run all tests
cargo test

# Run a specific test
cargo test verify

# Run a specific test with more verbose output
cargo test verify -- --nocapture
```

### Documentation

```bash
# Generate documentation
cargo doc

# Generate and open documentation in browser
cargo doc --open
```

### Linting and Formatting

```bash
# Run the linter
cargo clippy

# Apply suggestions from clippy
cargo clippy --fix

# Format code according to Rust style guidelines
cargo fmt
```

## Project Architecture

The codebase is organized as follows:

1. **Main Components:**
   - `PublicKey`: Handles the parsing and management of Minisign public keys
   - `Signature`: Handles the parsing and management of Minisign signatures
   - `StreamVerifier`: Provides streaming verification for larger files

2. **Cryptographic Components:**
   - Custom implementations of:
     - Ed25519 (EdDSA signature scheme)
     - Blake2b (cryptographic hash function)
     - SHA-512
     - Curve25519 operations
   - Base64 encoding/decoding for handling key/signature formats

The verification process:
1. Parse the public key and signature from their text representations
2. Verify the signature matches the expected public key ID
3. Handle optional prehashing of data with Blake2b (for non-legacy mode)
4. Perform Ed25519 signature verification
5. Verify both the main signature and the trusted comment signature

The `StreamVerifier` allows for incremental verification of large files by:
1. Creating a Blake2b hasher
2. Updating it with chunks of data
3. Finalizing the verification process with Ed25519 verification of the computed hash

## Development Notes

- The crate is designed to be minimal and zero-dependency
- All cryptographic primitives are implemented within the crate (in the crypto/ folder)
- The codebase avoids unsafe code and uses Rust's type system for safety
- The API is designed to be simple and follows Rust idioms for error handling