# [0.3.5] - 2025-10-03

- Fix compilation on platforms other than Unix and Windows by not
  providing `RandomAccessFile`.

# [0.3.4] - 2025-04-06

- Implement `ReadAt` and `Size` for `Arc<RandomAccessFile>`.
- Start specifying minimum supported Rust version (currently 1.71).

# [0.3.3] - 2023-11-01

* Add `Slice::{into_inner,get_ref,get_mut,offset,set_offset}()`.

# [0.3.2] - 2023-08-19

* Forward `ReatAt`, `WriteAt`, and `Size` for boxed types.
* Implement `Size for RandomAccessFile`.

# [0.3.1] - 2022-11-03

* Fixed compilation on 32-bit systems due to `posix_fadvise()` signature.

# [0.3.0] - 2022-08-27

* Renamed `{Read,Write}BytesExt` to `{Read,Write}BytesAtExt` to avoid overlap
  with `byteorder`.
* `{Read,Write}Int` and `{Read,Write}IntAt` are now inherent methods of
  `ByteIo`.
* Removed `Deref` and `DerefMut` implementations for `ByteIo`.
  Use `ByteIo::get_ref()` and `ByteIo::get_mut()` instead.
* Removed `Deref` and `DerefMut` for `SizeCursor`. `Cursor` methods are now
  also implemented on `SizeCursor`. Use `SizeCursor::as_cursor()` or
  `SizeCursor::as_cursor_mut()` to borrow the underlying `Cursor`.
* Fixed `WriteAt for File` on Windows: Writes were not working at all.
* Fixed `ReadAt for File` on Windows: Positioned reads were moving the
  file cursor. The new implementation is much slower but no longer modifies
  the read position.
* Various methods are now inlinable across crate boundaries.


# [0.2.2] - 2016-07-24

* Add 8-bit operations to integer read/write traits.


# [0.2.1] - 2016-07-12

* Remove dependency on nix.


# [0.2.0] - 2016-07-11

* Add traits ReadInt, ReadIntAt, WriteInt, WriteIntAt to describe behaviour of
  ByteIo.
* Make ByteIo implement Read, ReadAt, Write, WriteAt, so that it's more useful
  as a trait object.
* Add a changelog.


# [0.1.0] - 2016-07-06

Initial release.

* ReadAt, WriteAt, Size traits
* Implementations for:
  * Files (Unix and Windows)
  * Arrays
  * Vectors
  * References
* byteorder functionality
  * Positioned byte-order extensions ReadBytesExt, WriteBytesExt
  * ByteIo adapter, to make ByteOrder type parameter implicit
* Cursor, to turn a ReadAt/WriteAt into a Read/Write
* Slices, to turn a ReadAt/WriteAt into a smaller ReadAt/WriteAt
* Documentation
* Basic integration tests
