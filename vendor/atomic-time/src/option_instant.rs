use std::time::Instant;

use super::*;

/// Atomic version of [`Option<Instant>`].
#[repr(transparent)]
pub struct AtomicOptionInstant(AtomicOptionDuration);

impl Default for AtomicOptionInstant {
  /// Equivalent to `Option::<SystemTime>>::None`.
  #[inline]
  fn default() -> Self {
    Self::none()
  }
}

impl AtomicOptionInstant {
  /// Equivalent to atomic version `Option::<SystemTime>>::None`.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use atomic_time::AtomicOptionInstant;
  ///
  /// let none = AtomicOptionInstant::none();
  /// assert_eq!(none.load(std::sync::atomic::Ordering::SeqCst), None);
  /// ```
  #[inline]
  pub const fn none() -> Self {
    Self(AtomicOptionDuration::new(None))
  }

  /// Returns the system time corresponding to "now".
  ///
  /// # Examples
  /// ```
  /// use atomic_time::AtomicOptionInstant;
  ///
  /// let now = AtomicOptionInstant::now();
  /// ```
  pub fn now() -> Self {
    Self::new(Some(Instant::now()))
  }

  /// Creates a new `AtomicInstant` with the given `SystemTime` value.
  pub fn new(instant: Option<Instant>) -> Self {
    Self(AtomicOptionDuration::new(
      instant.map(encode_instant_to_duration),
    ))
  }

  /// Loads a value from the atomic instant.
  pub fn load(&self, order: Ordering) -> Option<Instant> {
    self.0.load(order).map(decode_instant_from_duration)
  }

  /// Stores a value into the atomic instant.
  pub fn store(&self, instant: Option<Instant>, order: Ordering) {
    self.0.store(instant.map(encode_instant_to_duration), order)
  }

  /// Stores a value into the atomic instant, returning the previous value.
  pub fn swap(&self, instant: Option<Instant>, order: Ordering) -> Option<Instant> {
    self
      .0
      .swap(instant.map(encode_instant_to_duration), order)
      .map(decode_instant_from_duration)
  }

  /// Stores a value into the atomic instant if the current value is the same as the `current`
  /// value.
  pub fn compare_exchange(
    &self,
    current: Option<Instant>,
    new: Option<Instant>,
    success: Ordering,
    failure: Ordering,
  ) -> Result<Option<Instant>, Option<Instant>> {
    match self.0.compare_exchange(
      current.map(encode_instant_to_duration),
      new.map(encode_instant_to_duration),
      success,
      failure,
    ) {
      Ok(duration) => Ok(duration.map(decode_instant_from_duration)),
      Err(duration) => Err(duration.map(decode_instant_from_duration)),
    }
  }

  /// Stores a value into the atomic instant if the current value is the same as the `current`
  /// value.
  pub fn compare_exchange_weak(
    &self,
    current: Option<Instant>,
    new: Option<Instant>,
    success: Ordering,
    failure: Ordering,
  ) -> Result<Option<Instant>, Option<Instant>> {
    match self.0.compare_exchange_weak(
      current.map(encode_instant_to_duration),
      new.map(encode_instant_to_duration),
      success,
      failure,
    ) {
      Ok(duration) => Ok(duration.map(decode_instant_from_duration)),
      Err(duration) => Err(duration.map(decode_instant_from_duration)),
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
  /// use atomic_time::AtomicOptionInstant;
  /// use std::{time::{Duration, Instant}, sync::atomic::Ordering};
  ///
  /// let now = Instant::now();
  /// let x = AtomicOptionInstant::none();
  /// assert_eq!(x.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |_| None), Err(None));
  /// x.store(Some(now), Ordering::SeqCst);
  /// assert_eq!(x.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| Some(x.map(|val| val + Duration::from_secs(1)))), Ok(Some(now)));
  /// assert_eq!(x.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| Some(x.map(|val| val + Duration::from_secs(1)))), Ok(Some(now + Duration::from_secs(1))));
  /// assert_eq!(x.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| Some(x.map(|val| val + Duration::from_secs(1)))), Ok(Some(now + Duration::from_secs(2))));
  /// assert_eq!(x.load(Ordering::SeqCst), Some(now + Duration::from_secs(3)));
  /// ```
  pub fn fetch_update<F>(
    &self,
    set_order: Ordering,
    fetch_order: Ordering,
    mut f: F,
  ) -> Result<Option<Instant>, Option<Instant>>
  where
    F: FnMut(Option<Instant>) -> Option<Option<Instant>>,
  {
    self
      .0
      .fetch_update(set_order, fetch_order, |duration| {
        f(duration.map(decode_instant_from_duration))
          .map(|system_time| system_time.map(encode_instant_to_duration))
      })
      .map(|duration| duration.map(decode_instant_from_duration))
      .map_err(|duration| duration.map(decode_instant_from_duration))
  }

  /// Returns `true` if operations on values of this type are lock-free.
  /// If the compiler or the platform doesn't support the necessary
  /// atomic instructions, global locks for every potentially
  /// concurrent atomic operation will be used.
  ///
  /// # Examples
  /// ```
  /// use atomic_time::AtomicOptionInstant;
  ///
  /// let is_lock_free = AtomicOptionInstant::is_lock_free();
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

  impl Serialize for AtomicOptionInstant {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
      self.0.load(Ordering::SeqCst).serialize(serializer)
    }
  }

  impl<'de> Deserialize<'de> for AtomicOptionInstant {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
      Ok(Self::new(
        Option::<Duration>::deserialize(deserializer)?.map(decode_instant_from_duration),
      ))
    }
  }
};

#[cfg(test)]
mod tests {
  use super::*;
  use std::time::Duration;

  #[test]
  fn test_atomic_option_instant_none() {
    let atomic_instant = AtomicOptionInstant::none();
    assert_eq!(atomic_instant.load(Ordering::SeqCst), None);
  }

  #[test]
  fn test_atomic_option_instant_now() {
    let atomic_instant = AtomicOptionInstant::now();
    let now = Instant::now();
    if let Some(loaded_instant) = atomic_instant.load(Ordering::SeqCst) {
      assert!(loaded_instant <= now);
      assert!(loaded_instant >= now - Duration::from_secs(1));
    } else {
      panic!("AtomicOptionInstant::now() should not be None");
    }
  }

  #[test]
  fn test_atomic_option_instant_new_and_load() {
    let now = Some(Instant::now());
    let atomic_instant = AtomicOptionInstant::new(now);
    assert_eq!(atomic_instant.load(Ordering::SeqCst), now);
  }

  #[test]
  fn test_atomic_option_instant_store_and_load() {
    let now = Some(Instant::now());
    let after_one_sec = now.map(|t| t + Duration::from_secs(1));
    let atomic_instant = AtomicOptionInstant::new(now);
    atomic_instant.store(after_one_sec, Ordering::SeqCst);
    assert_eq!(atomic_instant.load(Ordering::SeqCst), after_one_sec);
  }

  #[test]
  fn test_atomic_option_instant_swap() {
    let now = Some(Instant::now());
    let after_one_sec = now.map(|t| t + Duration::from_secs(1));
    let atomic_instant = AtomicOptionInstant::new(now);
    let prev_instant = atomic_instant.swap(after_one_sec, Ordering::SeqCst);
    assert_eq!(prev_instant, now);
    assert_eq!(atomic_instant.load(Ordering::SeqCst), after_one_sec);
  }

  #[test]
  fn test_atomic_option_instant_compare_exchange() {
    let now = Some(Instant::now());
    let after_one_sec = now.map(|t| t + Duration::from_secs(1));
    let atomic_instant = AtomicOptionInstant::new(now);
    let result =
      atomic_instant.compare_exchange(now, after_one_sec, Ordering::SeqCst, Ordering::SeqCst);
    assert!(result.is_ok());
    assert_eq!(atomic_instant.load(Ordering::SeqCst), after_one_sec);
  }

  #[test]
  fn test_atomic_option_instant_compare_exchange_weak() {
    let now = Some(Instant::now());
    let after_one_sec = now.map(|t| t + Duration::from_secs(1));
    let atomic_instant = AtomicOptionInstant::new(now);

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
  fn test_atomic_option_instant_fetch_update() {
    let now = Some(Instant::now());
    let atomic_instant = AtomicOptionInstant::new(now);

    let result = atomic_instant.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |prev| {
      Some(prev.map(|t| t + Duration::from_secs(1)))
    });
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), now);
    assert_eq!(
      atomic_instant.load(Ordering::SeqCst),
      now.map(|t| t + Duration::from_secs(1))
    );
  }

  #[test]
  fn test_atomic_option_instant_thread_safety() {
    use std::sync::Arc;
    use std::thread;

    let atomic_time = Arc::new(AtomicOptionInstant::now());
    let mut handles = vec![];

    // Spawn multiple threads to update the time
    for _ in 0..4 {
      let atomic_clone = Arc::clone(&atomic_time);
      let handle = thread::spawn(move || {
        let current = atomic_clone.load(Ordering::SeqCst);
        if let Some(current_time) = current {
          // Adding 1 second to the current time
          let new_time = current_time + Duration::from_secs(1);
          atomic_clone.store(Some(new_time), Ordering::SeqCst);
        }
      });
      handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
      handle.join().unwrap();
    }

    // Verify that the time has been updated
    if let Some(updated_time) = atomic_time.load(Ordering::SeqCst) {
      assert!(updated_time > Instant::now() - Duration::from_secs(4));
    } else {
      panic!("AtomicOptionInstant should not be None");
    }
  }

  #[cfg(feature = "serde")]
  #[test]
  fn test_atomic_system_time_serde() {
    use std::time::SystemTime;

    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct Test {
      time: AtomicOptionSystemTime,
    }

    let now = SystemTime::now();
    let test = Test {
      time: AtomicOptionSystemTime::new(Some(now)),
    };
    let serialized = serde_json::to_string(&test).unwrap();
    let deserialized: Test = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.time.load(Ordering::SeqCst), Some(now));
  }
}
