# PPMd in native Rust

[![Crate](https://img.shields.io/crates/v/ppmd-rust.svg)](https://crates.io/crates/ppmd-rust)
[![Documentation](https://docs.rs/ppmd-rust/badge.svg)](https://docs.rs/ppmd-rust)

PPMd compression / decompression. It's a port of the PPMd C-code from 7-Zip to Rust.

The following variants are provided:

- The PPMd7 (PPMdH) as used by the 7z archive format
- The PPMd8 (PPMdI rev.1) as used by the zip archive format

## Notice

There are two ways to properly bound the uncompressed data:

1. Save the uncompressed size along the compressed data and use `std::io::Read::read_exact()` to read the data
   (this is what is used in the 7z archive format).
2. Encode an end marker by calling `finish(true)` on the encoder when finishing the encoder process. You can then use
   `std::io::Read::read_to_end()` to read the data. You are of course free to also use the `std::io::Read::read_exact()`
   if you have stored the uncompressed data size (this is what is used in the zip archive format).

Failing to do so will result in garbage symbols at the end of the actual data.

## Acknowledgement

This port is based on the 7zip version of PPMd by Igor Pavlov, which in turn was based on the PPMd var.H (2001) /
PPMd var.I (2002) code by Dmitry Shkarin. The carryless range coder of PPMd8 was originally written by
Dmitry Subbotin (1999). These authors put the original code into the public domain.

## License

Licensed under [CC-0](https://spdx.org/licenses/CC0-1.0) or [MIT-0](https://spdx.org/licenses/MIT-0),
at your option.
