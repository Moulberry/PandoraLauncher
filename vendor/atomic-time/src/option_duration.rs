use crate::AtomicU128;
use core::{sync::atomic::Ordering, time::Duration};

/// Atomic version of [`Option<Duration>`].
#[repr(transparent)]
pub struct AtomicOptionDuration(AtomicU128);
impl core::fmt::Debug for AtomicOptionDuration {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_tuple("AtomicOptionDuration")
      .field(&self.load(Ordering::SeqCst))
      .finish()
  }
}
impl AtomicOptionDuration {
  /// Creates a new `AtomicOptionDuration` with `None`.
  pub const fn none() -> Self {
    Self(AtomicU128::new(encode_option_duration(None)))
  }

  /// Creates a new `AtomicOptionDuration` with the given value.
  pub const fn new(duration: Option<Duration>) -> Self {
    Self(AtomicU128::new(encode_option_duration(duration)))
  }
  /// Loads `Option<Duration>` from `AtomicOptionDuration`.
  ///
  /// load takes an [`Ordering`] argument which describes the memory ordering of this operation.
  ///
  /// # Panics
  /// Panics if order is [`Release`](Ordering::Release) or [`AcqRel`](Ordering::AcqRel).
  pub fn load(&self, ordering: Ordering) -> Option<Duration> {
    decode_option_duration(self.0.load(ordering))
  }
  /// Stores a value into the `AtomicOptionDuration`.
  ///
  /// `store` takes an [`Ordering`] argument which describes the memory ordering
  /// of this operation.
  ///
  /// # Panics
  ///
  /// Panics if `order` is [`Acquire`](Ordering::Acquire) or [`AcqRel`](Ordering::AcqRel).
  pub fn store(&self, val: Option<Duration>, ordering: Ordering) {
    self.0.store(encode_option_duration(val), ordering)
  }
  /// Stores a value into the `AtomicOptionDuration`, returning the old value.
  ///
  /// `swap` takes an [`Ordering`] argument which describes the memory ordering
  /// of this operation.
  pub fn swap(&self, val: Option<Duration>, ordering: Ordering) -> Option<Duration> {
    decode_option_duration(self.0.swap(encode_option_duration(val), ordering))
  }
  /// Stores a value into the `AtomicOptionDuration` if the current value is the same as the
  /// `current` value.
  ///
  /// Unlike [`compare_exchange`], this function is allowed to spuriously fail
  /// even when the comparison succeeds, which can result in more efficient
  /// code on some platforms. The return value is a result indicating whether
  /// the new value was written and containing the previous value.
  ///
  /// `compare_exchange` takes two [`Ordering`] arguments to describe the memory
  /// ordering of this operation. The first describes the required ordering if
  /// the operation succeeds while the second describes the required ordering
  /// when the operation fails. The failure ordering can't be [`Release`](Ordering::Release) or
  /// [`AcqRel`](Ordering::AcqRel) and must be equivalent or weaker than the success ordering.
  /// success ordering.
  ///
  /// [`compare_exchange`]: #method.compare_exchange
  pub fn compare_exchange_weak(
    &self,
    current: Option<Duration>,
    new: Option<Duration>,
    success: Ordering,
    failure: Ordering,
  ) -> Result<Option<Duration>, Option<Duration>> {
    self
      .0
      .compare_exchange_weak(
        encode_option_duration(current),
        encode_option_duration(new),
        success,
        failure,
      )
      .map(decode_option_duration)
      .map_err(decode_option_duration)
  }
  /// Stores a value into the `AtomicOptionDuration` if the current value is the same as the
  /// `current` value.
  ///
  /// The return value is a result indicating whether the new value was
  /// written and containing the previous value. On success this value is
  /// guaranteed to be equal to `new`.
  ///
  /// [`compare_exchange`] takes two [`Ordering`] arguments to describe the memory
  /// ordering of this operation. The first describes the required ordering if
  /// the operation succeeds while the second describes the required ordering
  /// when the operation fails. The failure ordering can't be [`Release`](Ordering::Release) or
  /// [`AcqRel`](Ordering::AcqRel) and must be equivalent or weaker than the success ordering.
  ///
  /// [`compare_exchange`]: #method.compare_exchange
  pub fn compare_exchange(
    &self,
    current: Option<Duration>,
    new: Option<Duration>,
    success: Ordering,
    failure: Ordering,
  ) -> Result<Option<Duration>, Option<Duration>> {
    self
      .0
      .compare_exchange(
        encode_option_duration(current),
        encode_option_duration(new),
        success,
        failure,
      )
      .map(decode_option_duration)
      .map_err(decode_option_duration)
  }
  /// Fetches the value, and applies a function to it that returns an optional
  /// new value. Returns a `Result` of `Ok(previous_value)` if the function returned `Some(_)`, else
  /// `Err(previous_value)`.
  ///
  /// Note: This may call the function multiple times if the value has been changed from other threads in
  /// the meantime, as long as the function returns `Some(_)`, but the function will have been applied
  /// only once to the stored value.
  ///
  /// `fetch_update` takes two [`Ordering`] arguments to describe the memory ordering of this operation.
  /// The first describes the required ordering for when the operation finally succeeds while the second
  /// describes the required ordering for loads. These correspond to the success and failure orderings of
  /// [`compare_exchange`] respectively.
  ///
  /// Using [`Acquire`](Ordering::Acquire) as success ordering makes the store part
  /// of this operation [`Relaxed`](Ordering::Relaxed), and using [`Release`](Ordering::Release) makes the final successful load
  /// [`Relaxed`](Ordering::Relaxed). The (failed) load ordering can only be [`SeqCst`](Ordering::SeqCst), [`Acquire`](Ordering::Acquire) or [`Relaxed`](Ordering::Release)
  /// and must be equivalent to or weaker than the success ordering.
  ///
  /// [`compare_exchange`]: #method.compare_exchange
  ///
  /// # Examples
  ///
  /// ```rust
  /// use atomic_time::AtomicOptionDuration;
  /// use std::{time::Duration, sync::atomic::Ordering};
  ///
  /// let x = AtomicOptionDuration::new(Some(Duration::from_secs(7)));
  /// assert_eq!(x.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |_| None), Err(Some(Duration::from_secs(7))));
  /// assert_eq!(x.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| Some(x.map(|val| val + Duration::from_secs(1)))), Ok(Some(Duration::from_secs(7))));
  /// assert_eq!(x.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| Some(x.map(|val| val + Duration::from_secs(1)))), Ok(Some(Duration::from_secs(8))));
  /// assert_eq!(x.load(Ordering::SeqCst), Some(Duration::from_secs(9)));
  /// ```
  pub fn fetch_update<F>(
    &self,
    set_order: Ordering,
    fetch_order: Ordering,
    mut f: F,
  ) -> Result<Option<Duration>, Option<Duration>>
  where
    F: FnMut(Option<Duration>) -> Option<Option<Duration>>,
  {
    self
      .0
      .fetch_update(set_order, fetch_order, |d| {
        f(decode_option_duration(d)).map(encode_option_duration)
      })
      .map(decode_option_duration)
      .map_err(decode_option_duration)
  }
  /// Consumes the atomic and returns the contained value.
  ///
  /// This is safe because passing `self` by value guarantees that no other threads are
  /// concurrently accessing the atomic data.
  #[inline]
  pub fn into_inner(self) -> Option<Duration> {
    decode_option_duration(self.0.into_inner())
  }

  /// Returns `true` if operations on values of this type are lock-free.
  /// If the compiler or the platform doesn't support the necessary
  /// atomic instructions, global locks for every potentially
  /// concurrent atomic operation will be used.
  ///
  /// # Examples
  /// ```
  /// use atomic_time::AtomicOptionDuration;
  ///
  /// let is_lock_free = AtomicOptionDuration::is_lock_free();
  /// ```
  #[inline]
  pub fn is_lock_free() -> bool {
    AtomicU128::is_lock_free()
  }
}

/// Encode an [`Option<Duration>`] into an [`u128`].
pub const fn encode_option_duration(option_duration: Option<Duration>) -> u128 {
  match option_duration {
    Some(duration) => {
      let seconds = duration.as_secs() as u128;
      let nanos = duration.subsec_nanos() as u128;
      (1 << 127) | (seconds << 32) | nanos
    }
    None => 0,
  }
}

/// Decode an [`Option<Duration>`] from an encoded [`u128`].
pub const fn decode_option_duration(encoded: u128) -> Option<Duration> {
  if encoded >> 127 == 0 {
    None
  } else {
    let seconds = ((encoded << 1) >> 33) as u64;
    let nanos = (encoded & 0xFFFFFFFF) as u32;
    Some(Duration::new(seconds, nanos))
  }
}

#[cfg(feature = "serde")]
const _: () = {
  use serde::{Deserialize, Serialize};

  impl Serialize for AtomicOptionDuration {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
      self.load(Ordering::SeqCst).serialize(serializer)
    }
  }

  impl<'de> Deserialize<'de> for AtomicOptionDuration {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
      Ok(Self::new(Option::<Duration>::deserialize(deserializer)?))
    }
  }
};

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_new_atomic_option_duration() {
    let duration = Duration::from_secs(5);
    let atomic_duration = AtomicOptionDuration::new(Some(duration));
    assert_eq!(atomic_duration.load(Ordering::SeqCst), Some(duration));
  }

  #[test]
  fn test_atomic_option_duration_load() {
    let duration = Duration::from_secs(10);
    let atomic_duration = AtomicOptionDuration::new(Some(duration));
    assert_eq!(atomic_duration.load(Ordering::SeqCst), Some(duration));
  }

  #[test]
  fn test_atomic_option_duration_store() {
    let initial_duration = Duration::from_secs(3);
    let new_duration = Duration::from_secs(7);
    let atomic_duration = AtomicOptionDuration::new(Some(initial_duration));
    atomic_duration.store(Some(new_duration), Ordering::SeqCst);
    assert_eq!(atomic_duration.load(Ordering::SeqCst), Some(new_duration));
  }

  #[test]
  fn test_atomic_option_duration_store_none() {
    let initial_duration = Duration::from_secs(3);
    let atomic_duration = AtomicOptionDuration::new(Some(initial_duration));
    atomic_duration.store(None, Ordering::SeqCst);
    assert_eq!(atomic_duration.load(Ordering::SeqCst), None);
  }

  #[test]
  fn test_atomic_option_duration_swap() {
    let initial_duration = Duration::from_secs(2);
    let new_duration = Duration::from_secs(8);
    let atomic_duration = AtomicOptionDuration::new(Some(initial_duration));
    let prev_duration = atomic_duration.swap(Some(new_duration), Ordering::SeqCst);
    assert_eq!(prev_duration, Some(initial_duration));
    assert_eq!(atomic_duration.load(Ordering::SeqCst), Some(new_duration));
  }

  #[test]
  fn test_atomic_option_duration_compare_exchange_weak() {
    let initial_duration = Duration::from_secs(4);
    let atomic_duration = AtomicOptionDuration::new(Some(initial_duration));

    // Successful exchange
    let mut result;
    loop {
      result = atomic_duration.compare_exchange_weak(
        Some(initial_duration),
        Some(Duration::from_secs(6)),
        Ordering::SeqCst,
        Ordering::SeqCst,
      );

      if result.is_ok() {
        break;
      }
    }

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Some(initial_duration));
    assert_eq!(
      atomic_duration.load(Ordering::SeqCst),
      Some(Duration::from_secs(6))
    );

    // Failed exchange
    let result = atomic_duration.compare_exchange_weak(
      Some(initial_duration),
      Some(Duration::from_secs(7)),
      Ordering::SeqCst,
      Ordering::SeqCst,
    );
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), Some(Duration::from_secs(6)));
  }

  #[test]
  fn test_atomic_option_duration_compare_exchange() {
    let initial_duration = Duration::from_secs(1);
    let atomic_duration = AtomicOptionDuration::new(Some(initial_duration));

    // Successful exchange
    let result = atomic_duration.compare_exchange(
      Some(initial_duration),
      Some(Duration::from_secs(5)),
      Ordering::SeqCst,
      Ordering::SeqCst,
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Some(initial_duration));
    assert_eq!(
      atomic_duration.load(Ordering::SeqCst),
      Some(Duration::from_secs(5))
    );

    // Failed exchange
    let result = atomic_duration.compare_exchange(
      Some(initial_duration),
      Some(Duration::from_secs(6)),
      Ordering::SeqCst,
      Ordering::SeqCst,
    );
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), Some(Duration::from_secs(5)));
  }

  #[test]
  fn test_atomic_option_duration_with_none_initially() {
    let atomic_duration = AtomicOptionDuration::new(None);
    assert_eq!(atomic_duration.load(Ordering::SeqCst), None);
  }

  #[test]
  fn test_atomic_option_duration_store_none_and_then_value() {
    let atomic_duration = AtomicOptionDuration::new(None);
    atomic_duration.store(Some(Duration::from_secs(5)), Ordering::SeqCst);
    assert_eq!(
      atomic_duration.load(Ordering::SeqCst),
      Some(Duration::from_secs(5))
    );
  }

  #[test]
  fn test_atomic_option_duration_swap_with_none() {
    let initial_duration = Duration::from_secs(2);
    let atomic_duration = AtomicOptionDuration::new(Some(initial_duration));
    let prev_duration = atomic_duration.swap(None, Ordering::SeqCst);
    assert_eq!(prev_duration, Some(initial_duration));
    assert_eq!(atomic_duration.load(Ordering::SeqCst), None);
  }

  #[test]
  fn test_atomic_option_duration_compare_exchange_weak_with_none() {
    let initial_duration = Duration::from_secs(4);
    let atomic_duration = AtomicOptionDuration::new(Some(initial_duration));

    // Change to None
    let result = atomic_duration.compare_exchange_weak(
      Some(initial_duration),
      None,
      Ordering::SeqCst,
      Ordering::SeqCst,
    );
    assert!(result.is_ok());
    assert_eq!(atomic_duration.load(Ordering::SeqCst), None);

    // Change back to Some(Duration)
    let new_duration = Duration::from_secs(6);
    let result = atomic_duration.compare_exchange_weak(
      None,
      Some(new_duration),
      Ordering::SeqCst,
      Ordering::SeqCst,
    );
    assert!(result.is_ok());
    assert_eq!(atomic_duration.load(Ordering::SeqCst), Some(new_duration));
  }

  #[test]
  fn test_atomic_option_duration_compare_exchange_with_none() {
    let initial_duration = Duration::from_secs(1);
    let atomic_duration = AtomicOptionDuration::new(Some(initial_duration));

    // Change to None
    let result = atomic_duration.compare_exchange(
      Some(initial_duration),
      None,
      Ordering::SeqCst,
      Ordering::SeqCst,
    );
    assert!(result.is_ok());
    assert_eq!(atomic_duration.load(Ordering::SeqCst), None);

    // Change back to Some(Duration)
    let new_duration = Duration::from_secs(5);
    let result = atomic_duration.compare_exchange(
      None,
      Some(new_duration),
      Ordering::SeqCst,
      Ordering::SeqCst,
    );
    assert!(result.is_ok());
    assert_eq!(atomic_duration.load(Ordering::SeqCst), Some(new_duration));
  }

  #[test]
  #[cfg(feature = "std")]
  fn test_atomic_option_duration_thread_safety() {
    use std::sync::Arc;
    use std::thread;

    let atomic_duration = Arc::new(AtomicOptionDuration::new(Some(Duration::from_secs(0))));
    let mut handles = vec![];

    // Spawn multiple threads to increment the duration
    for _ in 0..10 {
      let atomic_clone = Arc::clone(&atomic_duration);
      let handle = thread::spawn(move || {
        for _ in 0..100 {
          loop {
            let current = atomic_clone.load(Ordering::SeqCst);
            let new_duration = current
              .map(|d| d + Duration::from_millis(1))
              .or(Some(Duration::from_millis(1)));
            match atomic_clone.compare_exchange_weak(
              current,
              new_duration,
              Ordering::SeqCst,
              Ordering::SeqCst,
            ) {
              Ok(_) => break,     // Successfully updated
              Err(_) => continue, // Spurious failure, retry
            }
          }
        }
      });
      handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
      handle.join().unwrap();
    }

    // Verify the final value
    let expected_duration = Some(Duration::from_millis(10 * 100));
    assert_eq!(atomic_duration.load(Ordering::SeqCst), expected_duration);
  }

  #[cfg(feature = "serde")]
  #[test]
  fn test_atomic_option_duration_serde() {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct Test {
      duration: AtomicOptionDuration,
    }

    let test = Test {
      duration: AtomicOptionDuration::new(Some(Duration::from_secs(5))),
    };
    let serialized = serde_json::to_string(&test).unwrap();
    let deserialized: Test = serde_json::from_str(&serialized).unwrap();
    assert_eq!(
      deserialized.duration.load(Ordering::SeqCst),
      Some(Duration::from_secs(5))
    );

    let test = Test {
      duration: AtomicOptionDuration::new(None),
    };
    let serialized = serde_json::to_string(&test).unwrap();
    let deserialized: Test = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.duration.load(Ordering::SeqCst), None);
  }
}
