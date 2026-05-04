use crate::attribute::DeriveAttribute;
use crate::mode::Mode;
use darling::error::Accumulator;
use darling::{Error, FromDeriveInput};
use proc_macro2::{Ident, TokenStream};
use std::fmt::Display;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Generics};

// a struct to just hold all global data together (rather than passing them always around)
pub(crate) struct Derive<'a> {
    errors: Accumulator,
    pub(crate) ident: &'a Ident,
    pub(crate) generics: &'a Generics,
    pub(crate) attribute: &'a DeriveAttribute,
    pub(crate) mode: Mode,
}

impl Derive<'_> {
    #[inline]
    pub(crate) fn handle<T: Default>(&mut self, result: Result<T, Error>) -> T {
        self.errors.handle(result).unwrap_or_default()
    }

    pub(crate) fn error<S: Spanned, T: Display>(&mut self, s: &S, t: T) {
        self.error_with(s, t, ());
    }

    pub(crate) fn error_with<S: Spanned, T: Display, E>(&mut self, s: &S, t: T, e: E) -> E {
        self.errors.push(Error::custom(t).with_span(s));
        e
    }
}

pub(crate) fn derive(input: &DeriveInput, mode: Mode) -> TokenStream {
    let mut errors = Error::accumulator();
    let attribute = &mut errors
        .handle_in(|| DeriveAttribute::from_derive_input(input))
        .unwrap_or_default();
    let mut derive = Derive {
        errors,
        ident: &input.ident,
        generics: &input.generics,
        attribute,
        mode,
    };
    derive.verify_lifetimes();

    let mut result = match &input.data {
        Data::Struct(data) => derive.derive_struct(data),
        Data::Enum(data) => derive.derive_enum(data),
        Data::Union(_data) => {
            derive.error_with(input, "union is not supported", TokenStream::new())
        }
    };
    if let Err(errors) = derive.errors.finish() {
        result.extend(errors.write_errors());
    }
    result
}
