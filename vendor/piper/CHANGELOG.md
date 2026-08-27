# Version 0.2.5

- Add functions to allow for buffered reading and writing. (#27)
- Fix a bug where closing the `Writer` after writing can cause the `Reader` to
  lose bytes. (#31)

# Version 0.2.4

- Update doctests to be more reliable. (#19)

# Version 0.2.3

- Relax MSRV to v1.36. (#15)
- Fix the repository URL in Cargo.toml. (#16)

# Version 0.2.2

- Update `portable-atomic-util` to v0.2.0. (#12)

# Version 0.2.1

- Update `fastrand` to v2.0.0. (#2)

# Version 0.2.0

- **Breaking:** Replace the uploaded version on `crates.io` with a new, maintained version of `piper` that only contains the pipe code.
