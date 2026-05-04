[![Build Status](https://github.com/harryfei/which-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/harryfei/which-rs/actions/workflows/rust.yml)

# which

A Rust equivalent of Unix command "which". Locate installed executable in cross platforms.

## Support platforms

* Linux
* Windows
* macOS
* wasm32-wasi*

### A note on WebAssembly

This project aims to support WebAssembly with the [wasi](https://wasi.dev/) extension. This extension is a requirement. `which` is a library for exploring a filesystem, and
WebAssembly without wasi does not have a filesystem. `which` cannot do anything useful without this extension. Issues and PRs relating to
`wasm32-unknown-unknown` and `wasm64-unknown-unknown` will not be resolved or merged. All `wasm32-wasi*` targets are officially supported.

If you need to add a conditional dependency on `which` for this reason please refer to [the relevant cargo documentation for platform specific dependencies.](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#platform-specific-dependencies)

Here's an example of how to conditionally add `which`. You should tweak this to your needs.

```toml
[target.'cfg(not(all(target_family = "wasm", target_os = "unknown")))'.dependencies]
which = "7.0.0"
```

## Examples

1) To find which rustc executable binary is using.

    ``` rust
    use which::which;

    let result = which("rustc").unwrap();
    assert_eq!(result, PathBuf::from("/usr/bin/rustc"));
    ```

2. After enabling the `regex` feature, find all cargo subcommand executables on the path:

    ``` rust
    use which::which_re;

    which_re(Regex::new("^cargo-.*").unwrap()).unwrap()
        .for_each(|pth| println!("{}", pth.to_string_lossy()));
    ```

## MSRV

This crate currently has an MSRV of Rust 1.70. Increasing the MSRV is considered a breaking change and thus requires a major version bump.

We cannot make any guarantees about the MSRV of our dependencies. You may be required to pin one of our dependencies to a lower version in your own Cargo.toml in order to compile
with the minimum supported Rust version. Eventually Cargo will handle this automatically. See [rust-lang/cargo#9930](https://github.com/rust-lang/cargo/issues/9930) for more.

## Documentation

The documentation is [available online](https://docs.rs/which/).
