use crate::derive::Derive;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DataStruct, Fields};

impl Derive<'_> {
    pub(crate) fn derive_struct(&mut self, data: &DataStruct) -> TokenStream {
        let inner = match &data.fields {
            Fields::Named(data) => self.derive_named(None, data),
            Fields::Unnamed(data) => self.derive_unnamed(None, data),
            Fields::Unit => self.derive_struct_unit(),
        };
        self.generate(&inner)
    }

    fn derive_struct_unit(&self) -> TokenStream {
        let name = self.ident;
        quote! {#name}
    }
}
