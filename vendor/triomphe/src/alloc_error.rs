use core::fmt;

/// The error type returned by the fallible (`try_*`) allocation methods of
/// `triomphe` when the underlying allocator fails to allocate or the requested
/// layout overflows.
///
/// This mirrors [`core::alloc::AllocError`].
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct AllocError;

impl fmt::Display for AllocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("memory allocation failed")
    }
}

impl core::error::Error for AllocError {}
