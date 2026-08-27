[![Rust Crate](https://img.shields.io/crates/v/exr.svg)](https://crates.io/crates/exr)
[![Crates.io MSRV](https://img.shields.io/crates/msrv/exr?label=minimum%20rust%20version)](https://doc.rust-lang.org/cargo/reference/manifest.html#the-rust-version-field)
[![Rust Docs](https://docs.rs/exr/badge.svg)](https://docs.rs/exr)
[![NPM](https://img.shields.io/npm/v/exrs?style=flat&color=green)](https://www.npmjs.com/package/exrs)
[![downloads](https://img.shields.io/crates/d/exr)](https://crates.io/crates/exr)
[![Lines of Code](https://tokei.rs/b1/github/johannesvollmer/exrs?category=code)](https://tokei.rs)

# EXRS

This library is a 100% Rust and 100% safe code library for
reading and writing OpenEXR images.

[OpenEXR](http://www.openexr.com/)
is the de-facto standard image format in animation, VFX, and
other computer graphics pipelines, for it can represent an immense variety of pixel data with lossless compression.

Features include:
- any number of layers placed anywhere in 2d space, like in Photoshop
- any set of channels in an image (rgb, xyz, lab, depth, motion, mask, anything, ...)
- three types of high dynamic range values (16bit float, 32bit float, 32bit unsigned integer) per channel
- uncompressed pixel data for fast file access
- lossless compression for any image type
- lossy compression for non-deep image types to produce very small files
- load specific sections of an image without processing the whole file
- compress and decompress image pixels on multiple threads in parallel
- add arbitrary meta data to any image, including custom byte data, with full backwards compatibility
- any number of samples per pixel ("deep data") (not yet supported)

### Current Status

This library has matured quite a bit, but should still be considered incomplete.
For example, deep data and DWA compression algorithms are not supported yet.

If you encounter an exr file that cannot be opened by this crate but should be,
please leave an issue on this repository, containing the image file.

The focus is set on supporting all feature and correctness;
some performance optimizations are to be done.

__What we can do:__

- Supported OpenEXR Features
    - [x] custom attributes
    - [x] multi-part images (multiple layers, like Photoshop)
    - [x] multi-resolution images (mip maps, rip maps)
    - [x] access meta data and raw pixel blocks independently
    - [x] automatically crop away transparent pixels of an image (opt-in)
    - [ ] channel subsampling
    - [ ] deep data
    - [x] compression methods
        - [x] uncompressed
        - [x] zip line (lossless)
        - [x] zip block (lossless)
        - [x] rle (lossless)
        - [x] piz (lossless) (huge thanks to @dgsantana)
        - [x] pxr24 (lossless for f16 and u32)
        - [x] b44, b44a (huge thanks to @narann)
        - [x] dwaa, dwab (hunge thanks to @zinezockt)
        - [ ] HTJ2K32, HTJ2K256

- Nice Things
    - [x] no unsafe code, no undefined behaviour
    - [x] no CMake required or environment variables required
    - [x] re-imagined exr api with low barrier of entry
            (see `read_rgba_file`, `write_rgba_file`, `read_all_data_from_file`),
            plus embracing common high-level Rust abstractions
    - [x] a full-fledged image data structure that can contain any exr image,
            can open any image with a single function call (`read_all_data_from_file`)
            without knowing anything about the file in advance
    - [x] compress and decompress image sections either
            in parallel or with low memory overhead
    - [x] read and write progress callback
    - [x] write blocks streams, one after another
    - [x] memory mapping automatically supported
            by using the generic `std::io::Read` and `std::io::Write` traits


<!-- detailed internal feature checklist:
- [x] Inspecting Metadata
    - [x] Singlepart
        - [x] Tiles
        - [x] Scan lines
        - [x] Deep Tiles
        - [ ] Deep Scan Lines _(coded, but untested)_
    - [x] Multipart
        - [x] Tiles
        - [x] Scan lines
        - [ ] Deep Tiles _(coded, but untested)_
        - [x] Deep Scan Lines
    - [x] Multi Resolution
        - [x] Singular Resolution
        - [x] MipMaps
        - [x] RipMaps _(coded, but untested)_
    - [x] Non-Standard Attributes
        - [x] Reading those with known names and unknown names
        - [x] Reading those with known types
        - [x] Reading those with unknown types into a plain byte buffer
    - [x] Nice API for preview attribute extraction

- [ ] Decompressing Pixel Data
    - [x] Any LineOrder
    - [x] Any Pixel Type (`f16`, `f32`, `u32`)
    - [x] Multipart
    - [ ] Deep Data
    - [x] Rip/Mip Maps  _(coded, but untested)_
    - [ ] Nice API for RGBA conversion and displaying other color spaces?
    - [ ] Compression Methods
        - [x] Uncompressed
        - [x] ZIPS
        - [x] ZIP
        - [x] RLE
        - [x] PIZ
        - [x] RXR24
        - [x] B44, B44A
        - [x] dwaa, dwab

- [ ] Writing images
    - [x] Scan Lines
    - [x] Tiles
    - [x] Multipart
    - [ ] Deep Data
    - [x] User supplied line order
    - [x] Rip/Mip Maps _(coded, but untested)_
    - [x] 100% correct meta data
    - [x] Compression Methods
        - [x] Uncompressed
        - [x] ZIPS (lossless)
        - [x] ZIP (lossless)
        - [x] RLE (lossless)
        - [x] PIZ (lossless)
        - [x] PXR24 (lossless for f16 and u32)
        - [x] B44, B44A
        - [ ] DWAA, DWAB

- [x] De/compressing multiple blocks in parallel

- [ ] Profiling and real optimization
    - [x] Memory Mapping

- [x] IO Progress callback?
- [ ] SIMD
- [x] Detailed file validation
    - [x] Channels with an x or y sampling rate other than 1 are allowed only in flat, scan-line based images.
    - [x] If the headers include timeCode and chromaticities attributes, then the values of those attributes must also be the same for all parts of a file
    - [x] Scan-line based images cannot be multi-resolution images. (encoded in type system)
    - [x] Scan-line based images cannot have unspecified line order apparently?
    - [x] layer name is required for multipart images
    - [x] Enforce minimum length of 1 for arrays
    - [x] [Validate data_window matches data size when writing images] is not required because one is inferred from the other
    - [x] Channel names and layer names must be unique

- [x] Explore different APIs
    - [x] Let user decide how to store data
    - [x] Loading Metadata and specific tiles or blocks separately
-->


### Usage

> [!TIP]
> If you want to use the newest version of `exrs` with an older Rust version, you can still do that, by forcing Rust to use an older version of the `half` crate via `cargo update -p half --precise 2.2.1`, or downgrade all dependencies via `cargo +nightly -Zminimal-versions generate-lockfile`. Version `half 2.3.0` and higher have an MSRV above 1.61.

Add this to your `Cargo.toml`:
```toml
[dependencies]
exr = "1.74.2"

# also, optionally add this to your crate for smaller binary size
# and better runtime performance
[profile.release]
lto = true
```

The master branch of this repository always matches the `crates.io` version,
so you could also link the github repository master branch.

### Example

Example: [generate an rgb exr file](https://github.com/johannesvollmer/exrs/blob/master/examples/0_write_rgba.rs).

```rust
extern crate exr;

/// To write your image data, you need to specify how to retrieve a single pixel from it.
/// The closure may capture variables or generate data on the fly.
fn main() {
    use exr::prelude::*;

    // write a file, with 32-bit float precision per channel
    write_rgba_file(

        // this accepts paths or &str
        "minimal_rgba.exr",

        // image resolution is 2k
        2048, 2048,

        // generate (or lookup in your own image)
        // an f32 rgb color for each of the 2048x2048 pixels
        // (you could also create f16 values here to save disk space)
        |x,y| {
            (
                x as f32 / 2048.0, // red
                y as f32 / 2048.0, // green
                1.0 - (y as f32 / 2048.0), // blue
                1.0 // alpha
            )
        }

    ).unwrap();
}
```

See the [the examples folder](https://github.com/johannesvollmer/exrs/tree/master/examples) for more examples.

Or read [the guide](https://github.com/johannesvollmer/exrs/tree/master/GUIDE.md).


### Goals

`exrs` aims to provide a safe and convenient
interface to the OpenEXR file format. It is designed
to minimize the possibility of invalid files and runtime errors.
It contains a full-fledged image data structure that can contain any exr image,
but also grants access a low level block interface.

This library does not try to be a general purpose image file or image processing library.
Therefore, color conversion, beautiful subsampling, and mip map generation are left to other crates for now.
As the original OpenEXR implementation supports those operations, this library may choose to support them later.
Furthermore, this implementation does not try to produce byte-exact file output
matching the original implementation, instead, it is only aimed for correct output.

#### Safety

This library uses no unsafe code. In fact, this crate is annotated with `#[forbid(unsafe_code)]`.
Some dependencies use unsafe code, though this is minimized by selecting dependencies carefully.

All information from a file is handled with caution.
Allocations have a safe maximum size that will not be exceeded at once,
to reduce memory exhaustion attacks.

### What I am proud of

-   Flexible API (choose how to store your data instead of receiving an allocated image)
-   Safe API (almost impossible to accidentally write invalid files)
-   "if it compiles, it runs" methodology
-   [Awesome Contributors!](CONTRIBUTORS.md)

### Wasm

This crate supports the `wasm32-unknown-unknown` target for use in browsers and Node.js.

#### npm Package

For JavaScript/TypeScript projects, install the [`exrs`](https://www.npmjs.com/package/exrs) package from npm:

```bash
npm install exrs
```

```javascript
import { init, encodeExr, decodeExr } from 'exrs';

await init();

const bytes = encodeExr({
  width: 1920,
  height: 1080,
  layers: [{ name: 'rgba', channels: 'rgba', data: rgbaPixels }]
});
```

See the [exrs-wasm documentation](exrs-wasm/js/README.md) for the full API reference.

#### Notes
- Until WASM has threads, decoding and encoding will be slower for compressed files
- Read from byte buffers (`Uint8Array`) instead of file handles

### Motivation

This library does not support the toxic mindset of
rewriting existing C++ code in Rust just for the sake of switching the language.
The OpenEXR image format is defined by a proven
and battle-tested reference implementation.

However, as an alternative to the official reference implementation,
this library has the opportunity to explore radically different
designs, no matter what language it is written in. Neat!

Also, I really wanted to have a library
which had an 'X' in its name in my git repositories.

Keep in mind that there are official Rust bindings to the C++ reference implementation,
and they offer several advantages over this Rust implementation:
- they support all the features and can read any file, no surprises
- they are constantly driven by industry giants,
  so they have the higher probability of still being maintained in a decade
- they are battle tested and relied upon by a lot of existing projects

### Specification

This library is modeled after the
official [`OpenEXR Documentation`](https://openexr.com/en/latest/)
document. Saved older PDFs in /specification. Unspecified behavior is concluded from the C++ library.

### Roadmap
1. Support all compression formats
1. Support subsampling
1. Support Deep Data
1. Automatic conversion between color spaces
1. Profiling and other optimization
1. Tooling (Image Viewer App, Metadata Extraction Tool, ...)

## Contributing
This project has awesome contributors and is welcoming for
contributions on [Github](https://github.com/johannesvollmer/exrs).

### Code Formatting

This repository uses Rustfmt's standard style. Continuous Integration checks formatting on every pull request and will fail if files are not properly formatted.

How to format locally:

- Format the entire workspace in-place:
  - `cargo +nightly fmt --all`
- Check formatting without changing files (what CI runs):
  - `cargo +nightly fmt --all -- --check`

If `cargo +nightly fmt` is not found, install the Rustfmt component via Rustup:

- Install Rustfmt for your current toolchain:
  - `rustup component add rustfmt`
- If needed, ensure the stable toolchain is installed, then add Rustfmt explicitly:
  - `rustup toolchain install nightly`
  - `rustup component add --toolchain nightly rustfmt`

You may also configure your editor to run `cargo +nightly fmt` on save.

### Running Tests

To run all fast tests on your native system, use `cargo test`.

To start fuzzing on your native system indefinitely,
use `cargo test --package exr --test fuzz fuzz -- --exact --ignored`.

To run all fast tests on an emulated system, use one of the following commands.
Each command requires a running `docker` instance,
and `cross-rs` to be installed on your machine (`cargo install cross`).
- Powerpc (Big Endian) `cross test --target powerpc-unknown-linux-gnu --verbose`

You may also need to install the toolchain beforehand, using 
`rustup toolchain add stable-x86_64-unknown-linux-gnu --profile minimal --force-non-host` 
and `rustup target add powerpc-unknown-linux-gnu`.
 
To benchmark the library, simply run `cargo bench`.

#### SIMD tests (Intel SDE)

The DWA DCT/IDCT code has SIMD kernels (AVX2 and SSE2) that are selected at
runtime via [`pulp`](https://crates.io/crates/pulp), mirroring OpenEXR's own
cpuid dispatch. Each tier has its own feature-gated unit-test module in
`src/compression/dwa/idct.rs`:

- `mod avx2_tests` — requires the `avx2-tests` feature, exercises the AVX2 kernel
- `mod sse2_tests` — requires the `sse2-tests` feature, exercises the SSE2 kernel

These tests are **opt-in** and are excluded from the normal `cargo test` run. A
test only passes if the requested tier is actually available on the CPU it runs
on, so the AVX2 test needs an AVX2-capable CPU and the SSE2 test additionally
asserts that AVX2 is *not* present (so it verifies the fallback path).

Because most machines only expose their native tier, we run these tests under
[Intel SDE](https://www.intel.com/content/www/us/en/developer/articles/tool/software-development-emulator.html)
(Software Development Emulator), which emulates a chosen microarchitecture. This
is exactly what the `SIMD tests` CI workflow does: Cargo still builds a generic
x86/x86-64 binary, and SDE only changes the runtime CPUID/features seen by
`pulp`. To reproduce it locally:

1. **Install Intel SDE.** Download it from the link above and put the `sde64`
   binary on your `PATH` (or note its full path). SDE runs on x86-64 Linux,
   macOS, and Windows.

2. **Build the library test binary without running it,** then locate the executable:

   ```bash
   # AVX2
   cargo test --lib --features avx2-tests --no-run

   # SSE2
   cargo test --lib --features sse2-tests --no-run
   ```

   Cargo prints the path to the compiled unit-test binary (under
   `target/debug/deps/exr-<hash>`). Since these are now in-crate unit tests, you
   select a tier by passing a test-name **filter** (there is no per-file test
   binary). A short substring like `avx2`/`sse2` is enough — it matches the
   respective `mod avx2_tests`/`mod sse2_tests`.

3. **Run that binary under SDE,** selecting a chip that exposes the target tier
   and filtering to the tier's test module:

   ```bash
   # AVX2 — Haswell is the first microarchitecture with AVX2 + FMA
   sde64 -hsw -- target/debug/deps/exr-<hash> --nocapture avx2

   # SSE2 — Merom is the lowest 64-bit chip SDE models: it has SSE2 but no AVX,
   #        so pulp falls back to the SSE2 kernel
   sde64 -mrm -- target/debug/deps/exr-<hash> --nocapture sse2
   ```

   The CI workflow asserts the run reports the exact expected number of passing
   tests, so a mistyped filter (which libtest would report as `0 passed` while
   still exiting `0`) fails the job instead of silently passing.

> [!IMPORTANT]
> Do **not** set a global `RUSTFLAGS="-C target-feature=+avx2"` (or similar) to
> run these tests. This is a runtime-dispatched library: forcing a target
> feature globally would bake AVX2 into otherwise-portable code, break the
> plain x86-64 baseline, and make the fallback tests meaningless. The kernels
> already carry their own per-function `#[target_feature]`, so no global flag is
> needed. SDE alone controls which tier the runtime dispatch selects.

If your own CPU already exposes the required tier, you can skip SDE and run the
tests directly, e.g. on an AVX2-capable machine:

```bash
cargo test --lib --features avx2-tests -- avx2
```

The SSE2 module additionally asserts that AVX2 is *absent*, so it only passes
under SDE (or a CPU without AVX2) — running it directly on a modern machine will
trip that assertion by design.
