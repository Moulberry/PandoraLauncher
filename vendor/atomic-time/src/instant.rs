use std::time::Instant;

use super::*;

/// Atomic version of [`Instant`].
#[repr(transparent)]
pub struct AtomicInstant(AtomicDuration);

impl AtomicInstant {
  /// Returns the system time corresponding to "now".
  ///
  /// # Examples
  /// ```
  /// use atomic_time::AtomicInstant;
  ///
  /// let now = AtomicInstant::now();
  /// ```
  pub fn now() -> Self {
    Self::new(Instant::now())
  }

  /// Creates a new `AtomicInstant` with the given `SystemTime` value.
  pub fn new(instant: Instant) -> Self {
    Self(AtomicDuration::new(encode_instant_to_duration(instant)))
  }

  /// Loads a value from the atomic instant.
  pub fn load(&self, order: Ordering) -> Instant {
    decode_instant_from_duration(self.0.load(order))
  }

  /// Stores a value into the atomic instant.
  pub fn store(&self, instant: Instant, order: Ordering) {
    self.0.store(encode_instant_to_duration(instant), order)
  }

  /// Stores a value into the atomic instant, returning the previous value.
  pub fn swap(&self, instant: Instant, order: Ordering) -> Instant {
    decode_instant_from_duration(self.0.swap(encode_instant_to_duration(instant), order))
  }

  /// Stores a value into the atomic instant if the current value is the same as the `current`
  /// value.
  pub fn compare_exchange(
    &self,
    current: Instant,
    new: Instant,
    success: Ordering,
    failure: Ordering,
  ) -> Result<Instant, Instant> {
    match self.0.compare_exchange(
      encode_instant_to_duration(current),
      encode_instant_to_duration(new),
      success,
      failure,
    ) {
      Ok(duration) => Ok(decode_instant_from_duration(duration)),
      Err(duration) => Err(decode_instant_from_duration(duration)),
    }
  }

  /// Stores a value into the atomic instant if the current value is the same as the `current`
  /// value.
  pub fn compare_exchange_weak(
    &self,
    current: Instant,
    new: Instant,
    success: Ordering,
    failure: Ordering,
  ) -> Result<Instant, Instant> {
    match self.0.compare_exchange_weak(
      encode_instant_to_duration(current),
      encode_instant_to_duration(new),
      success,
      failure,
    ) {
      Ok(duration) => Ok(decode_instant_from_duration(duration)),
      Err(duration) => Err(decode_instant_from_duration(duration)),
    }
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
  /// use atomic_time::AtomicInstant;
  /// use std::{time::{Duration, Instant}, sync::atomic::Ordering};
  ///
  /// let now = Instant::now();
  /// let x = AtomicInstant::new(now);
  /// assert_eq!(x.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |_| None), Err(now));
  ///
  /// assert_eq!(x.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| Some(x + Duration::from_secs(1))), Ok(now));
  /// assert_eq!(x.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| Some(x + Duration::from_secs(1))), Ok(now + Duration::from_secs(1)));
  /// assert_eq!(x.load(Ordering::SeqCst), now + Duration::from_secs(2));
  /// ```
  pub fn fetch_update<F>(
    &self,
    set_order: Ordering,
    fetch_order: Ordering,
    mut f: F,
  ) -> Result<Instant, Instant>
  where
    F: FnMut(Instant) -> Option<Instant>,
  {
    self
      .0
      .fetch_update(set_order, fetch_order, |duration| {
        f(decode_instant_from_duration(duration)).map(encode_instant_to_duration)
      })
      .map(decode_instant_from_duration)
      .map_err(decode_instant_from_duration)
  }

  /// Returns `true` if operations on values of this type are lock-free.
  /// If the compiler or the platform doesn't support the necessary
  /// atomic instructions, global locks for every potentially
  /// concurrent atomic operation will be used.
  ///
  /// # Examples
  /// ```
  /// use atomic_time::AtomicInstant;
  ///
  /// let is_lock_free = AtomicInstant::is_lock_free();
  /// ```
  #[inline]
  pub fn is_lock_free() -> bool {
    AtomicU128::is_lock_free()
  }
}

#[cfg(feature = "serde")]
const _: () = {
  use core::time::Duration;
  use serde::{Deserialize, Serialize};

  impl Serialize for AtomicInstant {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
      self.0.load(Ordering::SeqCst).serialize(serializer)
    }
  }

  impl<'de> Deserialize<'de> for AtomicInstant {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
      Ok(Self::new(decode_instant_from_duration(
        Duration::deserialize(deserializer)?,
      )))
    }
  }
};

#[cfg(test)]
mod tests {
  use super::*;
  use std::time::Duration;

  #[test]
  fn test_atomic_instant_now() {
    let atomic_instant = AtomicInstant::now();
    // Check that the time is reasonable (not too far from now).
    let now = Instant::now();
    let loaded_instant = atomic_instant.load(Ordering::SeqCst);
    assert!(loaded_instant <= now);
    assert!(loaded_instant >= now - Duration::from_secs(1));
  }

  #[test]
  fn test_atomic_instant_new_and_load() {
    let now = Instant::now();
    let atomic_instant = AtomicInstant::new(now);
    assert_eq!(atomic_instant.load(Ordering::SeqCst), now);
  }

  #[test]
  fn test_atomic_instant_store_and_load() {
    let now = Instant::now();
    let after_one_sec = now + Duration::from_secs(1);
    let atomic_instant = AtomicInstant::new(now);
    atomic_instant.store(after_one_sec, Ordering::SeqCst);
    assert_eq!(atomic_instant.load(Ordering::SeqCst), after_one_sec);
  }

  #[test]
  fn test_atomic_instant_swap() {
    let now = Instant::now();
    let after_one_sec = now + Duration::from_secs(1);
    let atomic_instant = AtomicInstant::new(now);
    let prev_instant = atomic_instant.swap(after_one_sec, Ordering::SeqCst);
    assert_eq!(prev_instant, now);
    assert_eq!(atomic_instant.load(Ordering::SeqCst), after_one_sec);
  }

  #[test]
  fn test_atomic_instant_compare_exchange() {
    let now = Instant::now();
    let after_one_sec = now + Duration::from_secs(1);
    let atomic_instant = AtomicInstant::new(now);
    let result =
      atomic_instant.compare_exchange(now, after_one_sec, Ordering::SeqCst, Ordering::SeqCst);
    assert!(result.is_ok());
    assert_eq!(atomic_instant.load(Ordering::SeqCst), after_one_sec);
  }

  #[test]
  fn test_atomic_instant_compare_exchange_weak() {
    let now = Instant::now();
    let after_one_sec = now + Duration::from_secs(1);
    let atomic_instant = AtomicInstant::new(now);

    let mut result;
    loop {
      result = atomic_instant.compare_exchange_weak(
        now,
        after_one_sec,
        Ordering::SeqCst,
        Ordering::SeqCst,
      );
      if result.is_ok() {
        break;
      }
    }
    assert!(result.is_ok());
    assert_eq!(atomic_instant.load(Ordering::SeqCst), after_one_sec);
  }

  #[test]
  fn test_atomic_instant_fetch_update() {
    let now = Instant::now();
    let atomic_instant = AtomicInstant::new(now);

    let result = atomic_instant.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |prev| {
      Some(prev + Duration::from_secs(1))
    });
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), now);
    assert_eq!(
      atomic_instant.load(Ordering::SeqCst),
      now + Duration::from_secs(1)
    );
  }

  #[test]
  fn test_atomic_instant_thread_safety() {
    use std::sync::Arc;
    use std::thread;

    let atomic_instant = Arc::new(AtomicInstant::now());
    let mut handles = vec![];

    for _ in 0..4 {
      let atomic_clone = atomic_instant.clone();
      let handle = thread::spawn(move || {
        let current = atomic_clone.load(Ordering::SeqCst);
        let new = current + Duration::from_millis(50);
        atomic_clone.store(new, Ordering::SeqCst);
      });
      handles.push(handle);
    }

    for handle in handles {
      handle.join().unwrap();
    }

    // Check that the instant has advanced by at least 50 milliseconds * 4 threads
    let loaded_instant = atomic_instant.load(Ordering::SeqCst);
    assert!(loaded_instant >= Instant::now() - Duration::from_millis(200));
  }

  #[cfg(feature = "serde")]
  #[test]
  fn test_atomic_instant_serde() {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct Test {
      time: AtomicInstant,
    }

    let now = Instant::now();
    let test = Test {
      time: AtomicInstant::new(now),
    };
    let serialized = serde_json::to_string(&test).unwrap();
    let deserialized: Test = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.time.load(Ordering::SeqCst), now);
  }
}
