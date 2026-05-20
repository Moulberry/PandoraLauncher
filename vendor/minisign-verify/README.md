# minisign-verify

A small, zero-dependencies Rust crate to verify [Minisign](https://jedisct1.github.io/minisign/) signatures.

[API documentation](https://docs.rs/minisign-verify)

## Features

* Verify signatures for both standard and pre-hashed modes
* Streaming verification for large files
* No external dependencies
* Simple, auditable code
* Comprehensive error reporting

## Basic Example

```rust
let public_key =
    PublicKey::from_base64("RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3")
        .expect("Unable to decode the public key");

let signature = Signature::decode(
    "untrusted comment: signature from minisign secret key
RWQf6LRCGA9i59SLOFxz6NxvASXDJeRtuZykwQepbDEGt87ig1BNpWaVWuNrm73YiIiJbq71Wi+dP9eKL8OC351vwIasSSbXxwA=
trusted comment: timestamp:1555779966\tfile:test
QtKMXWyYcwdpZAlPF7tE2ENJkRd1ujvKjlj1m9RtHTBnZPa5WKU5uWRs5GoP5M/VqE81QFuMKI5k/SfNQUaOAA==",
    ).expect("Unable to decode the signature");

let bin = b"test";
public_key.verify(&bin[..], &signature, false).expect("Signature didn't verify");
```

## Loading from Files

```rust
use minisign_verify::{PublicKey, Signature};
use std::path::Path;

// Load a public key from a file
let public_key = PublicKey::from_file(Path::new("minisign.pub"))
    .expect("Unable to load the public key");

// Load a signature from a file
let signature = Signature::from_file(Path::new("file.sig"))
    .expect("Unable to load the signature");

// Load the file content to verify
let content = std::fs::read("file").expect("Unable to read the file");

// Verify the signature
public_key
    .verify(&content, &signature, false)
    .expect("Signature didn't verify");
```

## Streaming Verification (for Large Files)

```rust
use minisign_verify::{PublicKey, Signature};
use std::fs::File;
use std::io::Read;
use std::path::Path;

// Load a public key and signature
let public_key = PublicKey::from_file(Path::new("minisign.pub"))
    .expect("Unable to load the public key");

let signature = Signature::from_file(Path::new("large_file.sig"))
    .expect("Unable to load the signature");

// Create a stream verifier
let mut verifier = public_key.verify_stream(&signature)
    .expect("Unable to create stream verifier");

// Process the file in chunks
let mut file = File::open("large_file").expect("Unable to open file");
let mut buffer = [0u8; 8192]; // 8KB buffer

loop {
    let bytes_read = file.read(&mut buffer).expect("Error reading file");
    if bytes_read == 0 {
        break; // End of file
    }

    verifier.update(&buffer[..bytes_read]);
}

// Verify the signature
verifier.finalize().expect("Signature verification failed");
```

Note that the streaming verification mode only works with pre-hashed signatures (the default in newer versions of Minisign).

## Running Benchmarks

To run the benchmarks:

```bash
cargo +nightly bench
```

## License

MIT
