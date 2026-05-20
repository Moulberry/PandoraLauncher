use core::sync::atomic::Ordering;
use std::time::SystemTime;

use crate::AtomicDuration;

/// An atomic version of [`std::time::SystemTime`].
#[repr(transparent)]
pub struct AtomicSystemTime(AtomicDuration);

impl AtomicSystemTime {
  /// Returns the system time corresponding to "now".
  ///
  /// # Examples
  /// ```
  /// use atomic_time::AtomicSystemTime;
  ///
  /// let sys_time = AtomicSystemTime::now();
  /// ```
  pub fn now() -> Self {
    Self::new(SystemTime::now())
  }

  /// Creates a new `AtomicSystemTime` with the given `SystemTime` value.
  ///
  /// # Panics
  ///
  /// If the given `SystemTime` value is earlier than the [`UNIX_EPOCH`](SystemTime::UNIX_EPOCH).
  pub fn new(system_time: SystemTime) -> Self {
    Self(AtomicDuration::new(
      system_time.duration_since(SystemTime::UNIX_EPOCH).unwrap(),
    ))
  }

  /// Loads a value from the atomic system time.
  pub fn load(&self, order: Ordering) -> SystemTime {
    SystemTime::UNIX_EPOCH + self.0.load(order)
  }

  /// Stores a value into the atomic system time.
  ///
  /// # Panics
  ///
  /// If the given `SystemTime` value is earlier than the [`UNIX_EPOCH`](SystemTime::UNIX_EPOCH).
  pub fn store(&self, system_time: SystemTime, order: Ordering) {
    self.0.store(
      system_time.duration_since(SystemTime::UNIX_EPOCH).unwrap(),
      order,
    )
  }

  /// Stores a value into the atomic system time, returning the previous value.
  ///
  /// # Panics
  ///
  /// If the given `SystemTime` value is earlier than the [`UNIX_EPOCH`](SystemTime::UNIX_EPOCH).
  pub fn swap(&self, system_time: SystemTime, order: Ordering) -> SystemTime {
    SystemTime::UNIX_EPOCH
      + self.0.swap(
        system_time.duration_since(SystemTime::UNIX_EPOCH).unwrap(),
        order,
      )
  }

  /// Stores a value into the atomic system time if the current value is the same as the `current`
  /// value.
  ///
  /// # Panics
  ///
  /// If the given `SystemTime` value is earlier than the [`UNIX_EPOCH`](SystemTime::UNIX_EPOCH).
  pub fn compare_exchange(
    &self,
    current: SystemTime,
    new: SystemTime,
    success: Ordering,
    failure: Ordering,
  ) -> Result<SystemTime, SystemTime> {
    match self.0.compare_exchange(
      current.duration_since(SystemTime::UNIX_EPOCH).unwrap(),
      new.duration_since(SystemTime::UNIX_EPOCH).unwrap(),
      success,
      failure,
    ) {
      Ok(duration) => Ok(SystemTime::UNIX_EPOCH + duration),
      Err(duration) => Err(SystemTime::UNIX_EPOCH + duration),
    }
  }

  /// Stores a value into the atomic system time if the current value is the same as the `current`
  /// value.
  ///
  /// # Panics
  ///
  /// If the given `SystemTime` value is earlier than the [`UNIX_EPOCH`](SystemTime::UNIX_EPOCH).
  pub fn compare_exchange_weak(
    &self,
    current: SystemTime,
    new: SystemTime,
    success: Ordering,
    failure: Ordering,
  ) -> Result<SystemTime, SystemTime> {
    match self.0.compare_exchange_weak(
      current.duration_since(SystemTime::UNIX_EPOCH).unwrap(),
      new.duration_since(SystemTime::UNIX_EPOCH).unwrap(),
      success,
      failure,
    ) {
      Ok(duration) => Ok(SystemTime::UNIX_EPOCH + duration),
      Err(duration) => Err(SystemTime::UNIX_EPOCH + duration),
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
  /// # Panics
  ///
  /// If the given `SystemTime` value is earlier than the [`UNIX_EPOCH`](SystemTime::UNIX_EPOCH).
  ///
  /// # Examples
  ///
  /// ```rust
  /// use atomic_time::AtomicSystemTime;
  /// use std::{time::{Duration, SystemTime}, sync::atomic::Ordering};
  ///
  /// let now = SystemTime::now();
  /// let x = AtomicSystemTime::new(now);
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
  ) -> Result<SystemTime, SystemTime>
  where
    F: FnMut(SystemTime) -> Option<SystemTime>,
  {
    self
      .0
      .fetch_update(set_order, fetch_order, |duration| {
        f(SystemTime::UNIX_EPOCH + duration)
          .map(|system_time| system_time.duration_since(SystemTime::UNIX_EPOCH).unwrap())
      })
      .map(|duration| SystemTime::UNIX_EPOCH + duration)
      .map_err(|duration| SystemTime::UNIX_EPOCH + duration)
  }

  /// Returns `true` if operations on values of this type are lock-free.
  /// If the compiler or the platform doesn't support the necessary
  /// atomic instructions, global locks for every potentially
  /// concurrent atomic operation will be used.
  ///
  /// # Examples
  /// ```
  /// use atomic_time::AtomicSystemTime;
  ///
  /// let is_lock_free = AtomicSystemTime::is_lock_free();
  /// ```
  #[inline]
  pub fn is_lock_free() -> bool {
    AtomicDuration::is_lock_free()
  }
}

#[cfg(feature = "serde")]
const _: () = {
  use serde::{Deserialize, Serialize};

  impl Serialize for AtomicSystemTime {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
      self.load(Ordering::SeqCst).serialize(serializer)
    }
  }

  impl<'de> Deserialize<'de> for AtomicSystemTime {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
      Ok(Self::new(SystemTime::deserialize(deserializer)?))
    }
  }
};

#[cfg(test)]
mod tests {
  use super::*;
  use std::time::Duration;

  #[test]
  fn test_atomic_system_time_now() {
    let atomic_time = AtomicSystemTime::now();
    // As SystemTime::now is very precise, this test needs to be lenient with timing.
    // Just check that the time is not the UNIX_EPOCH.
    assert_ne!(atomic_time.load(Ordering::SeqCst), SystemTime::UNIX_EPOCH);
  }

  #[test]
  fn test_atomic_system_time_new_and_load() {
    let now = SystemTime::now();
    let atomic_time = AtomicSystemTime::new(now);
    assert_eq!(atomic_time.load(Ordering::SeqCst), now);
  }

  #[test]
  fn test_atomic_system_time_store_and_load() {
    let now = SystemTime::now();
    let after_one_sec = now + Duration::from_secs(1);
    let atomic_time = AtomicSystemTime::new(now);
    atomic_time.store(after_one_sec, Ordering::SeqCst);
    assert_eq!(atomic_time.load(Ordering::SeqCst), after_one_sec);
  }

  #[test]
  fn test_atomic_system_time_swap() {
    let now = SystemTime::now();
    let after_one_sec = now + Duration::from_secs(1);
    let atomic_time = AtomicSystemTime::new(now);
    let prev_time = atomic_time.swap(after_one_sec, Ordering::SeqCst);
    assert_eq!(prev_time, now);
    assert_eq!(atomic_time.load(Ordering::SeqCst), after_one_sec);
  }

  #[test]
  fn test_atomic_system_time_compare_exchange_weak() {
    let now = SystemTime::now();
    let after_one_sec = now + Duration::from_secs(1);
    let after_two_secs = now + Duration::from_secs(2);
    let atomic_time = AtomicSystemTime::new(now);

    // Successful compare_exchange_weak
    let mut result;
    loop {
      result =
        atomic_time.compare_exchange_weak(now, after_one_sec, Ordering::SeqCst, Ordering::SeqCst);
      if result.is_ok() {
        break;
      }
    }
    assert!(result.is_ok());
    assert_eq!(atomic_time.load(Ordering::SeqCst), after_one_sec);

    // Failed compare_exchange_weak
    let result =
      atomic_time.compare_exchange_weak(now, after_two_secs, Ordering::SeqCst, Ordering::SeqCst);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), after_one_sec);
  }

  #[test]
  fn test_atomic_system_time_compare_exchange() {
    let now = SystemTime::now();
    let after_one_sec = now + Duration::from_secs(1);
    let atomic_time = AtomicSystemTime::new(now);
    let result =
      atomic_time.compare_exchange(now, after_one_sec, Ordering::SeqCst, Ordering::SeqCst);
    assert!(result.is_ok());
    assert_eq!(atomic_time.load(Ordering::SeqCst), after_one_sec);
  }

  #[test]
  fn test_atomic_system_time_fetch_update() {
    let now = SystemTime::now();
    let atomic_time = AtomicSystemTime::new(now);

    // Update the time by adding 1 second
    let result = atomic_time.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |prev| {
      Some(prev + Duration::from_secs(1))
    });
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), now);
    assert_eq!(
      atomic_time.load(Ordering::SeqCst),
      now + Duration::from_secs(1)
    );

    // Trying an update that returns None, should not change the value
    let result = atomic_time.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |_| None);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), now + Duration::from_secs(1));
    assert_eq!(
      atomic_time.load(Ordering::SeqCst),
      now + Duration::from_secs(1)
    );
  }

  #[test]
  #[cfg(feature = "std")]
  fn test_atomic_system_time_thread_safety() {
    use std::sync::Arc;
    use std::thread;

    let atomic_time = Arc::new(AtomicSystemTime::now());
    let mut handles = vec![];

    for _ in 0..4 {
      let atomic_clone = atomic_time.clone();
      let handle = thread::spawn(move || {
        let current = atomic_clone.load(Ordering::SeqCst);
        let new = current + Duration::from_secs(1);
        atomic_clone.store(new, Ordering::SeqCst);
      });
      handles.push(handle);
    }

    for handle in handles {
      handle.join().unwrap();
    }

    // This checks that the time has advanced, but it's not precise about how much,
    // due to the non-deterministic nature of thread execution order and timing.
    assert!(atomic_time.load(Ordering::SeqCst) > SystemTime::now());
  }

  #[cfg(feature = "serde")]
  #[test]
  fn test_atomic_system_time_serde() {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct Test {
      time: AtomicSystemTime,
    }

    let now = SystemTime::now();
    let test = Test {
      time: AtomicSystemTime::new(now),
    };
    let serialized = serde_json::to_string(&test).unwrap();
    let deserialized: Test = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.time.load(Ordering::SeqCst), now,);
  }
}
