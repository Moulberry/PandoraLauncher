# range-alloc

A generic range allocator for Rust.

`RangeAllocator<T>` manages a contiguous range and hands out non-overlapping
sub-ranges on request. It uses a best-fit strategy to reduce fragmentation and
automatically merges adjacent free ranges on deallocation. Allocations can
optionally be aligned to a given boundary without wasting the padding space.

## Example

```rust
use range_alloc::RangeAllocator;

let mut alloc = RangeAllocator::new(0u64..1024);

// Basic allocation.
let a = alloc.allocate_range(256).unwrap();
assert_eq!(a, 0..256);

// Aligned allocation -- the returned range starts on a 128-byte boundary.
let b = alloc.allocate_range_aligned(64, 128).unwrap();
assert_eq!(b, 256..320);

// Free a range so it can be reused.
alloc.free_range(a);

// Grow the pool if you need more space.
alloc.grow_to(2048);
```

## Minimum Supported Rust Version

The MSRV of this crate is at least 1.31, possibly earlier. It will only be
bumped in a breaking release.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE.APACHE](LICENSE.APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE.MIT](LICENSE.MIT) or http://opensource.org/licenses/MIT)

at your option.
