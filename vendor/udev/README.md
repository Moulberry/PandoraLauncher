# udev
This crate provides a safe wrapper around the native `libudev` library. It applies the RAII pattern
and Rust lifetimes to ensure safe usage of all `libudev` functionality. The RAII pattern ensures
that all acquired resources are released when they're no longer needed, and Rust lifetimes ensure
that resources are released in a proper order.

* [Documentation](http://docs.rs/udev/)

## Dependencies
In order to use the `libudev` crate, you must have a Linux system with the `libudev` library
installed where it can be found by `pkg-config`. To install `libudev` on Debian-based Linux
distributions, execute the following command:

```
sudo apt-get install libudev-dev
```

`libudev` is a Linux-specific package. It is not available for Windows, OS X, or other operating
systems.

### Cross-Compiling
The `libudev` crate can be used when cross-compiling to a foreign target. Details on how to
cross-compile `libudev` are explained in the [`libudev-sys` crate's
README](https://github.com/dcuddeback/libudev-sys#cross-compiling).

## Usage
Add `udev` as a dependency in `Cargo.toml`:

```toml
[dependencies]
udev = "^0.9.3"
```

If you plan to support operating systems other than Linux, you'll need to add `udev` as a
target-specific dependency:

```toml
[target.x86_64-unknown-linux-gnu.dependencies]
udev = "^0.9.3"
```

Import the `udev` crate.

```rust
extern crate udev;

fn main() {
  let mut enumerator = udev::Enumerator::new().unwrap();

  enumerator.match_subsystem("tty").unwrap();

  for device in enumerator.scan_devices().unwrap() {
    println!("found device: {:?}", device.syspath());
  }
}
```

### Thread-safety

The udev-api does not have any thread-safety guarantees, however many implementations are indeed thread-safe.
If you are certain the implementations you are targetting and supporting are, you can enable the `send`-feature
to have `udev-rs` types implement `Send`:

```toml
[dependencies]
udev = { version = "^0.9.3", features = ["send"] }
```

The even stronger `Sync` bound can be added with the `sync` feature.


## Contributors
* [drakulix](https://github.com/drakulix)
* [dcuddeback](https://github.com/dcuddeback)
* [mulkieran](https://github.com/mulkieran)
* [Susurrus](https://github.com/Susurrus)
* [woodruffw](https://github.com/woodruffw)
* [Ravenslofty](https://github.com/Ravenslofty)
* [sjoerdsimons](https://github.com/sjoerdsimons)
* [anelson](https://github.com/anelson)
* [ollpu](https://github.com/ollpu)
* [a1ien](https://github.com/a1ien)
* [lj94093](https://github.com/lj94093)
* [patrickelectric](https://github.com/patrickelectric)
* [TomzBench](https://github.com/TomzBench)
* [endrift](https://github.com/endrift)

## License
Copyright © 2017 Victoria Brekenfeld
Copyright © 2015 David Cuddeback

Copyright for portions of the project are held by [David Cuddeback, 2015] as part of the project.
All other copyright for the project are held by [Victoria Brekenfeld, 2017].

Distributed under the [MIT License](LICENSE).
