use crate::attribute::{FieldAttribute, OrAssign};
use crate::derive::Derive;
use darling::FromAttributes;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{FieldsNamed, FieldsUnnamed, LitInt, Type, TypeReference, Variant};

impl Derive<'_> {
    pub(crate) fn derive_named(
        &mut self,
        variant: Option<Variant>,
        data: &FieldsNamed,
    ) -> TokenStream {
        let mut fields = Vec::new();
        for d in &data.named {
            let mut field_attribute = self.handle(FieldAttribute::from_attributes(&d.attrs));
            if let Some(ref variant) = variant {
                field_attribute
                    .or_assign(&self.handle(FieldAttribute::from_attributes(&variant.attrs)));
            }
            field_attribute.or_assign(self.attribute);
            let name = d.ident.as_ref().unwrap();
            let call = self.create_call(
                &field_attribute,
                &name.into_token_stream(),
                variant.is_none(),
                &d.ty,
            );
            fields.push(quote! {#name: #call});
        }

        let name = self.ident;
        if let Some(variant) = variant {
            let variant_name = &variant.ident;
            quote! {#name :: #variant_name { #(#fields),* }}
        } else {
            quote! {#name { #(#fields),* }}
        }
    }

    pub(crate) fn derive_unnamed(
        &mut self,
        variant: Option<&Variant>,
        data: &FieldsUnnamed,
    ) -> TokenStream {
        let mut fields = Vec::new();
        for (i, d) in data.unnamed.iter().enumerate() {
            let mut field_attribute = self.handle(FieldAttribute::from_attributes(&d.attrs));
            if let Some(variant) = variant {
                field_attribute
                    .or_assign(&self.handle(FieldAttribute::from_attributes(&variant.attrs)));
                field_attribute.or_assign(self.attribute);
                fields.push(self.create_call(
                    &field_attribute,
                    &Ident::new(&format!("arg{i}"), Span::call_site()).to_token_stream(),
                    false,
                    &d.ty,
                ));
            } else {
                field_attribute.or_assign(self.attribute);
                fields.push(self.create_call(
                    &field_attribute,
                    &LitInt::new(&i.to_string(), Span::call_site()).to_token_stream(),
                    true,
                    &d.ty,
                ));
            }
        }

        let name = self.ident;
        if let Some(variant) = variant {
            let variant_name = &variant.ident;
            quote! {#name :: #variant_name ( #(#fields),* )}
        } else {
            quote! {#name ( #(#fields),* )}
        }
    }

    fn create_call(
        &mut self,
        field_attribute: &FieldAttribute,
        index: &TokenStream,
        with_self: bool,
        ty: &Type,
    ) -> TokenStream {
        if let Type::Reference(TypeReference {
            lifetime: Some(l), ..
        }) = ty
        {
            if self.attribute.is_reference_lifetime(&l.ident) {
                if with_self {
                    quote! { self . #index}
                } else {
                    quote! { #index }
                }
            } else {
                self.error(ty,"References are not supported out of the box, see: https://docs.rs/ownable/*/ownable/#references");
                quote! { todo!() }
            }
        } else if field_attribute.clone.unwrap_or(false) {
            if with_self {
                quote! {::core::clone::Clone::clone(& self . #index)}
            } else {
                quote! {::core::clone::Clone::clone(& #index)}
            }
        } else {
            let trait_name = self.mode.name();
            let trait_function = self.mode.function();
            if with_self {
                let as_ref = self.mode.as_ref();
                quote! {#trait_name::#trait_function(#as_ref self . #index)}
            } else {
                quote! {#trait_name::#trait_function(#index)}
            }
        }
    }
}
