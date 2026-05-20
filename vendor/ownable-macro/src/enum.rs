use crate::derive::Derive;
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{DataEnum, Fields, FieldsNamed, FieldsUnnamed, Variant};

impl Derive<'_> {
    pub(crate) fn derive_enum(&mut self, data: &DataEnum) -> TokenStream {
        let mut matches = Vec::new();
        for v in &data.variants {
            matches.push(match &v.fields {
                Fields::Named(data) => self.match_named(v, data),
                Fields::Unnamed(data) => self.match_unnamed(v, data),
                Fields::Unit => self.match_unit(v),
            });
        }
        self.generate(&quote! {
            match self {
                #(#matches),*
            }
        })
    }

    fn match_named(&mut self, variant: &Variant, data: &FieldsNamed) -> TokenStream {
        let name = self.ident;
        let variant_name = &variant.ident;
        let mut pattern = Vec::new();
        for field in &data.named {
            pattern.push(field.ident.as_ref().unwrap());
        }
        let inner = self.derive_named(Some(variant.clone()), data);
        quote! {#name :: #variant_name {#(#pattern),* } => #inner}
    }

    fn match_unnamed(&mut self, variant: &Variant, data: &FieldsUnnamed) -> TokenStream {
        let name = self.ident;
        let variant_name = &variant.ident;
        let mut pattern = Vec::new();
        for i in 0..data.unnamed.len() {
            pattern.push(Ident::new(&format!("arg{i}"), Span::call_site()));
        }
        let inner = self.derive_unnamed(Some(variant), data);
        quote! {#name :: #variant_name ( #(#pattern),* ) => #inner}
    }

    fn match_unit(&self, variant: &Variant) -> TokenStream {
        let name = self.ident;
        let variant_name = &variant.ident;
        quote! {#name :: #variant_name => #name :: #variant_name}
    }
}
