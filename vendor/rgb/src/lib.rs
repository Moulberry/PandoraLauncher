//! Basic struct for `RGB` and `RGBA` pixels. Packed, with red first, alpha last.
//!
//! This crate is intended to be the lowest common denominator for sharing `RGB`/`RGBA` bitmaps between other crates.
//!
//! The crate includes convenience functions for converting between the struct and bytes,
//! and overloaded operators that work on all channels at once.
//!
//! This crate intentionally doesn't implement color management (due to complexity of the problem),
//! but the structs can be parametrized to implement this if necessary. Other colorspaces are out of scope.
//!
#![cfg_attr(feature = "as-bytes", doc = "```rust")]
#![cfg_attr(not(feature = "as-bytes"), doc = "```ignore")]
//! # use rgb::*;
//! let pixel = RGB8 {r:0, g:100, b:255};
//!
//! let pixel_rgba = pixel.alpha(255);
//! let pixel = pixel_rgba.rgb();
//!
//! let pixels = vec![pixel; 100];
//! /// Can be converted to a type-less slice without copying
//! let bytes: &[u8] = rgb::bytemuck::cast_slice(&pixels);
//!
//! use rgb::prelude::*; // Import pixel map trait
//! let half_bright = pixel.map(|channel| channel / 2);
//! let doubled = half_bright * 2;
//! # let _ = doubled;
//! ```
#![doc(html_logo_url = "https://kornel.ski/rgb-logo.png")]
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]

// std is required to run unit tests
#[cfg(test)]
#[macro_use] extern crate std;
/// Re-export of the [`bytemuck` crate](https://lib.rs/bytemuck). [See docs](https://docs.rs/bytemuck).
///
/// Use [`::bytemuck::cast_slice()`] or [`::bytemuck::from_bytes()`] to convert
/// pixels to/from `&[u8]`.
#[cfg(feature = "bytemuck")]
#[doc(alias = "ComponentSlice")]
#[doc(alias = "as_bytes")]
#[doc(alias = "Pod")]
pub use ::bytemuck;

pub(crate) mod formats {
    pub mod abgr;
    pub mod argb;
    pub mod bgr;
    pub mod bgra;
    pub mod gray;
    pub mod gray_a;
    pub mod gray_alpha;
    pub mod grb;
    pub mod rgb;
    pub mod rgba;
}

/// Use as `use rgb::prelude::*`
///
/// The prelude will contain commonly used traits, and will be expanded in the future.
///
/// Using a glob import is recommended for forward compatibility with the next major version of the crate.
pub mod prelude {
    pub use crate::legacy::internal::pixel::ComponentMap;
    pub use crate::legacy::internal::pixel::ColorComponentMap;
}

pub use formats::abgr::Abgr;
pub use formats::argb::Argb;
pub use formats::bgr::Bgr;
pub use formats::bgra::Bgra;
#[cfg(not(feature = "unstable-experimental"))]
pub use formats::gray_alpha::GrayAlpha_v08 as GrayAlpha;
#[cfg(feature = "unstable-experimental")]
#[deprecated(note = "Renamed to GrayA")]
pub use formats::gray_a::GrayA as GrayAlpha;
#[cfg(not(feature = "unstable-experimental"))]
pub use formats::gray::Gray_v08 as Gray;
#[cfg(feature = "unstable-experimental")]
pub use formats::gray::Gray_v09 as Gray;
pub use formats::grb::Grb;
pub use formats::rgb::Rgb;
pub use formats::rgba::Rgba;

mod inherent_impls;

pub(crate) mod legacy {
    pub(crate) mod internal {
        pub mod convert;
        pub mod ops;
        pub mod pixel;
        pub mod rgb;
        pub mod rgba;
    }
    /// BGR/BGRA alernative layouts & grayscale
    ///
    /// BGR might be useful for some Windows or OpenGL APIs.
    pub mod alt;
}

pub use legacy::alt;

#[cfg(all(feature = "bytemuck", not(feature = "as-bytes")))]
mod bytemuck_impl;
#[cfg(feature = "as-bytes")]
mod as_bytes;

/// Re-export from `bytemuck` crate
#[cfg(feature = "as-bytes")]
pub use ::bytemuck::Pod;
/// Re-export from `bytemuck` crate
#[cfg(feature = "as-bytes")]
pub use ::bytemuck::Zeroable;

pub use crate::legacy::internal::convert::*;
pub use crate::legacy::internal::pixel::*;

#[doc(hidden)]
/// Renamed to `Rgb`
pub use formats::rgb::Rgb as RGB;
#[doc(hidden)]
/// Renamed to `Rgba`
pub use formats::rgba::Rgba as RGBA;

#[doc(hidden)]
/// Incompatible replacement for the `GrayAlpha` type
pub use formats::gray_a::GrayA;

/// 8-bit RGB
///
/// The colorspace is technically undefined, but generally sRGB is assumed.
pub type RGB8 = RGB<u8>;

/// 16-bit RGB in machine's native endian
///
/// Be careful to perform byte-swapping when reading from files.
pub type RGB16 = RGB<u16>;

/// 8-bit RGBA, alpha is last. 0 = transparent, 255 = opaque.
pub type RGBA8 = RGBA<u8>;

/// 16-bit RGB in machine's native endian. 0 = transparent, 65535 = opaque.
///
/// Alpha is last.
pub type RGBA16 = RGBA<u16>;

