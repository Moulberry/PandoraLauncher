[![cargo version](https://img.shields.io/crates/v/atomic_enum.svg)](https://crates.io/crates/atomic_enum) 
[![docs.rs version](https://img.shields.io/docsrs/atomic_enum)](https://docs.rs/atomic_enum/latest/atomic_enum/)
# atomic_enum

An attribute to create an atomic wrapper around a C-style enum.

Internally, the generated wrapper uses an `AtomicUsize` to store the value.
The atomic operations have the same semantics as the equivalent operations
of `AtomicUsize`.

# Example
```rust
# use atomic_enum::atomic_enum;
# use std::sync::atomic::Ordering;
#[atomic_enum]
#[derive(PartialEq)]
enum CatState {
    Dead = 0,
    BothDeadAndAlive,
    Alive,
}

let state = AtomicCatState::new(CatState::Dead);
state.store(CatState::Alive, Ordering::Relaxed);
assert_eq!(state.load(Ordering::Relaxed), CatState::Alive);
```

This attribute does not use or generate any unsafe code.

The crate can be used in a `#[no_std]` environment.

# Maintenance Note
This crate is passively maintained.