#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]

//! This crate is not to be used on it's own, please see <https://docs.rs/ownable>.

mod attribute;
mod common;
mod derive;
mod r#enum;
mod generate;
mod mode;
mod r#struct;

use crate::derive::derive;
use crate::mode::Mode;
use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

// This mod uses proc_macro::TokenStream while all others use proc_macro2::TokenStream!

/// Derive `to_borrowed`.
#[proc_macro_derive(ToBorrowed, attributes(ownable))]
pub fn to_borrowed(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);
    derive(&input, Mode::ToBorrowed).into()
}

/// Derive `to_owned`.
#[proc_macro_derive(ToOwned, attributes(ownable))]
pub fn to_owned(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);
    derive(&input, Mode::ToOwned).into()
}

/// Derive `into_owned`.
#[proc_macro_derive(IntoOwned, attributes(ownable))]
pub fn into_owned(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);
    derive(&input, Mode::IntoOwned).into()
}
