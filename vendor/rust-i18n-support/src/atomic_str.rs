use std::fmt;
use std::ops::Deref;

use arc_swap::{ArcSwapAny, Guard};
use triomphe::Arc;

/// A thread-safe atomically reference-counting string.
pub struct AtomicStr(ArcSwapAny<Arc<String>>);

/// A thread-safe view the string that was stored when `AtomicStr::as_str()` was called.
struct GuardedStr(Guard<Arc<String>>);

impl Deref for GuardedStr {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

impl AtomicStr {
    /// Create a new `AtomicStr` with the given value.
    pub fn new(value: &str) -> Self {
        let arced = Arc::new(value.into());
        Self(ArcSwapAny::new(arced))
    }

    /// Get the string slice.
    pub fn as_str(&self) -> impl Deref<Target = str> {
        GuardedStr(self.0.load())
    }

    /// Replaces the value at self with src.
    pub fn replace(&self, src: impl Into<String>) {
        let arced = Arc::new(src.into());
        self.0.store(arced);
    }
}

impl From<&str> for AtomicStr {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for AtomicStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_str(s: &str) {
        assert_eq!(s, "hello");
    }

    #[test]
    fn test_atomic_str() {
        let s = AtomicStr::from("hello");
        test_str(&s.as_str());
    }
}
