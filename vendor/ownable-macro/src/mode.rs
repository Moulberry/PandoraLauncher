use proc_macro2::TokenStream;
use quote::quote;

#[derive(Clone, Copy)]
pub(crate) enum Mode {
    ToBorrowed,
    ToOwned,
    IntoOwned,
}

impl Mode {
    pub(crate) fn name(self) -> TokenStream {
        match self {
            Mode::ToBorrowed => quote!(::ownable::traits::ToBorrowed),
            Mode::ToOwned => quote!(::ownable::traits::ToOwned),
            Mode::IntoOwned => quote!(::ownable::traits::IntoOwned),
        }
    }
    pub(crate) fn function(self) -> TokenStream {
        match self {
            Mode::ToBorrowed => quote!(to_borrowed),
            Mode::ToOwned => quote!(to_owned),
            Mode::IntoOwned => quote!(into_owned),
        }
    }
    pub(crate) fn as_ref(self) -> TokenStream {
        if let Mode::IntoOwned = self {
            quote!()
        } else {
            quote!(&)
        }
    }
    pub(crate) fn doc(self) -> &'static str {
        match self {
            Mode::ToBorrowed => {
                " Copy the structure and reference the original values.\n\
                \n\
                This is always a deep copy of the structure."
            }
            Mode::ToOwned => {
                " Copy the structure and clone the original values.\n\
                \n\
                This is always a deep copy."
            }
            Mode::IntoOwned => {
                " Copy the structure and clone the original values if it's not owned.\n\
                \n\
                This is always a deep copy of the structure."
            }
        }
    }
}
