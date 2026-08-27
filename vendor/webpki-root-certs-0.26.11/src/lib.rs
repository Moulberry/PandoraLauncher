//! A compiled-in copy of the full X.509 root certificates trusted by Mozilla.
//!
//! You should generally prefer to use [`webpki-roots`] when using [`rustls`] or [`webpki`] as it is
//! more space efficient and convenient for that use.
//!
//! This library is suitable for use in applications that can always be recompiled and instantly deployed.
//! For applications that are deployed to end-users and cannot be recompiled, or which need certification
//! before deployment, consider a library that uses the platform native certificate verifier such as
//! [`rustls-platform-verifier`]. This has the additional benefit of supporting OS provided CA constraints
//! and revocation data.
//!
//! This crate version is a semver trick to webpki-root-certs v1.  Users of it should
//! migrate their dependency to webpki-root-certs v1 at their leisure.
//!
//! [`webpki-roots`]: https://docs.rs/webpki-roots
//! [`webpki`]: https://docs.rs/rustls-webpki
//! [`rustls`]: https://docs.rs/rustls
//! [`rustls-platform-verifier`]: https://docs.rs/rustls-platform-verifier

#![no_std]
#![forbid(unsafe_code, unstable_features)]
#![deny(
    elided_lifetimes_in_paths,
    trivial_casts,
    trivial_numeric_casts,
    unused_import_braces,
    unused_extern_crates,
    unused_qualifications
)]

pub use parent::TLS_SERVER_ROOT_CERTS;
