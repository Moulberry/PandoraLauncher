positioned-io
=============

This crate allows you to specify an offset for reads and writes, without changing the current
position in a file. This is similar to [`pread()` and `pwrite()`][pread] in C.

The major advantages of this type of I/O are:

* You don't need to seek before doing a random-access read or write, which is convenient.
* Reads don't modify the file at all, so don't require mutability.

[pread]: http://man7.org/linux/man-pages/man2/pread.2.html

[![Crates.io](https://img.shields.io/crates/v/positioned-io.svg)](https://crates.io/crates/positioned-io)
[![Documentation](https://docs.rs/positioned-io/badge.svg)](https://docs.rs/positioned-io)

Example
-------

Read the fifth 512-byte sector of a file:

```rust
use std::fs::File;
use positioned_io::ReadAt;

// note that file does not need to be mut
let file = File::open("tests/pi.txt")?;

// read up to 512 bytes
let mut buf = [0; 512];
let bytes_read = file.read_at(2048, &mut buf)?;
```

**Note:** If possible use the `RandomAccessFile` wrapper. On Windows `ReadAt`
directly on `File` is very slow.

Documentation
-------------

https://docs.rs/positioned-io

License
-------

positioned-io is licensed under the [MIT license](https://github.com/vasi/positioned-io/blob/master/LICENSE-MIT).
