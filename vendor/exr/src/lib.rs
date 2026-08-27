//! Read and write OpenEXR images.
//! This library uses no foreign code or unsafe Rust.
//!
//! See the [README.md](https://github.com/johannesvollmer/exrs/blob/master/README.md) for crate information.
//! Read __the [GUIDE.md](https://github.com/johannesvollmer/exrs/blob/master/GUIDE.md) for a API introduction__.
//! Check out the [examples](https://github.com/johannesvollmer/exrs/tree/master/examples) for a first impression.

#![warn(
    rust_2018_idioms,
    future_incompatible,
    unused_extern_crates,
    unused,
    missing_copy_implementations,
    missing_debug_implementations,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]
#![deny(
    unused_variables,
    unused_assignments,
    dead_code,
    unused_must_use,
    missing_copy_implementations,
    trivial_numeric_casts,
    redundant_semicolons
)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod io; // public to allow for custom attribute byte parsing

pub mod compression;
pub mod image;
pub mod math;
pub mod meta;

pub mod block;
pub mod error;

#[macro_use]
extern crate smallvec;

/// Export the most important items from `exrs`.
/// _Note: This includes a type called `Result`, possibly overwriting the
/// default `std::Result` type usage._
pub mod prelude {

    /// Import this specifically if you want to be explicit but still use the
    /// extension traits.
    pub mod traits {
        pub use crate::image::{
            crop::{ApplyCroppedView, Crop, CropResult, CropWhere, CroppedChannels, InspectSample},
            read::{
                any_channels::ReadSamples,
                image::{ReadImage, ReadLayers},
                layers::ReadChannels,
                read,
                specific_channels::ReadSpecificChannel,
            },
            write::{channels::GetPixel, WritableImage},
        };
    }

    // re-export external stuff
    pub use half::f16;
    pub use smallvec::SmallVec;
    pub use traits::*;

    // error handling
    pub use crate::error::{Error, Result};
    pub use crate::image::{
        read::{
            read_all_data_from_file, read_all_flat_layers_from_file,
            read_all_rgba_layers_from_file, read_first_flat_layer_from_file,
            read_first_rgba_layer_from_file,
        },
        write::{write_rgb_file, write_rgba_file},
    };
    // common math
    pub use crate::math::Vec2;
    // image data structures
    pub use crate::{
        block::samples::Sample,
        image::*,
        meta::{
            attribute,
            attribute::{
                AttributeValue, ChannelDescription, Compression, IntegerBounds, LineOrder,
                SampleType, Text, TileDescription,
            },
            header::{ImageAttributes, LayerAttributes},
            MetaData,
        },
    };
}
