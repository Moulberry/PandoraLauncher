use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Error, Field, punctuated::Punctuated, spanned::Spanned};
use zvariant_utils::macros;

use crate::utils::*;

fn dict_name_for_field(
    f: &Field,
    rename_attr: Option<String>,
    rename_all_attr: Option<&str>,
) -> Result<String, Error> {
    let ident = f.ident.as_ref().unwrap().to_string();
    rename_identifier(ident, f.span(), rename_attr, rename_all_attr)
}

/// Implements `Serialize` for structs as D-Bus dictionaries via a serde helper.
pub fn expand_serialize_derive(input: DeriveInput) -> Result<TokenStream, Error> {
    let StructAttributes {
        rename_all,
        crate_path: crate_attr,
        ..
    } = StructAttributes::parse(&input.attrs)?;
    let crate_path = parse_crate_path(crate_attr.as_deref())?;
    let rename_all_str = rename_all.as_deref().unwrap_or("snake_case");
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = &input.ident;
    let helper = format_ident!("__SerializeDict{}", name);
    let zv = zvariant_path(crate_path.as_ref());

    let mut field_defs = Vec::new();
    let mut field_inits = Vec::new();
    if let Data::Struct(data) = &input.data {
        for field in &data.fields {
            let ident = field.ident.as_ref().unwrap();
            let ty = &field.ty;
            let FieldAttributes { rename, .. } = FieldAttributes::parse(&field.attrs)?;
            let dict_name = dict_name_for_field(field, rename, rename_all.as_deref())?;
            let is_opt = macros::ty_is_option(ty);
            if is_opt {
                let as_value_opt_path = quote! { #zv::as_value::optional };
                let as_value_opt_str = format!("{as_value_opt_path}");
                field_defs.push(quote! {
                    #[serde(
                        rename = #dict_name,
                        with = #as_value_opt_str,
                        skip_serializing_if = "Option::is_none",
                    )]
                    #ident: &'a #ty
                });
            } else {
                let as_value_path = quote! { #zv::as_value };
                let as_value_str = format!("{as_value_path}");
                field_defs.push(quote! {
                    #[serde(rename = #dict_name, with = #as_value_str)]
                    #ident: &'a #ty
                });
            }
            field_inits.push(quote! { #ident: &self.#ident });
        }
    } else {
        return Err(Error::new(input.span(), "only structs supported"));
    }

    Ok(quote! {
        #[allow(deprecated)]
        impl #impl_generics #zv::export::serde::ser::Serialize for #name #ty_generics #where_clause {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: #zv::export::serde::ser::Serializer,
            {
                use #zv::export::serde::Serialize;

                #[derive(Serialize)]
                #[serde(rename_all = #rename_all_str)]
                struct #helper<'a> {
                    #[serde(skip)]
                    phantom: ::std::marker::PhantomData<&'a ()>,
                    #(#field_defs,)*
                }

                let helper = #helper {
                    phantom: ::std::marker::PhantomData,
                    #(#field_inits,)*
                };

                helper.serialize(serializer)
            }
        }
    })
}

/// Implements `Deserialize` for structs from D-Bus dictionaries via a serde helper.
pub fn expand_deserialize_derive(input: DeriveInput) -> Result<TokenStream, Error> {
    let StructAttributes {
        rename_all,
        deny_unknown_fields,
        crate_path: crate_attr,
        ..
    } = StructAttributes::parse(&input.attrs)?;
    let crate_path = parse_crate_path(crate_attr.as_deref())?;
    let rename_all_str = rename_all.as_deref().unwrap_or("snake_case");
    let zv = zvariant_path(crate_path.as_ref());

    // Create a new generics with a 'de lifetime
    let mut generics = input.generics.clone();
    let lifetime_param = syn::LifetimeParam {
        attrs: Vec::new(),
        lifetime: syn::Lifetime::new("'de", Span::call_site()),
        colon_token: None,
        bounds: Punctuated::new(),
    };
    generics
        .params
        .insert(0, syn::GenericParam::Lifetime(lifetime_param));

    let (impl_generics, _ty_generics, where_clause) = generics.split_for_impl();
    let (_, orig_ty_generics, _) = input.generics.split_for_impl();
    let name = &input.ident;
    let helper = format_ident!("__DeserializeDict{}", name);

    let mut field_defs = Vec::new();
    let mut field_assignments = Vec::new();
    let mut non_optional_field_checks = Vec::new();
    if let Data::Struct(data) = &input.data {
        for field in &data.fields {
            let ident = field.ident.as_ref().unwrap();
            let ty = &field.ty;
            let FieldAttributes { rename, .. } = FieldAttributes::parse(&field.attrs)?;
            let dict_name = dict_name_for_field(field, rename, rename_all.as_deref())?;
            let is_opt = macros::ty_is_option(ty);

            if is_opt {
                let as_value_opt_path = quote! { #zv::as_value::optional };
                let as_value_opt_str = format!("{as_value_opt_path}");
                field_defs.push(quote! {
                    #[serde(rename = #dict_name, with = #as_value_opt_str, default)]
                    #ident: #ty
                });
                field_assignments.push(quote! { #ident: helper.#ident });
            } else {
                // For non-optional fields, use Option<T> in helper for default support
                let as_value_opt_path = quote! { #zv::as_value::optional };
                let as_value_opt_str = format!("{as_value_opt_path}");
                field_defs.push(quote! {
                    #[serde(rename = #dict_name, with = #as_value_opt_str, default)]
                    #ident: Option<#ty>
                });

                // Add a check to make sure this field was provided
                non_optional_field_checks.push(quote! {
                    if helper.#ident.is_none() {
                        return Err(<D::Error as #zv::export::serde::de::Error>::missing_field(#dict_name));
                    }
                });

                // Unwrap the option for field assignment
                field_assignments.push(quote! { #ident: helper.#ident.unwrap() });
            }
        }
    } else {
        return Err(Error::new(input.span(), "only structs supported"));
    }

    let deny_attr = if deny_unknown_fields {
        quote! { , deny_unknown_fields }
    } else {
        quote! {}
    };

    Ok(quote! {
        #[allow(deprecated)]
        impl #impl_generics #zv::export::serde::de::Deserialize<'de> for #name #orig_ty_generics
        #where_clause
        {
            fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
            where
                D: #zv::export::serde::de::Deserializer<'de>,
            {
                use #zv::export::serde::Deserialize;

                #[derive(Deserialize, Default)]
                #[serde(default, rename_all = #rename_all_str #deny_attr)]
                struct #helper {
                    #(#field_defs,)*
                }

                let helper = #helper::deserialize(deserializer)?;

                // Check for missing non-optional fields
                #(#non_optional_field_checks)*

                Ok(Self {
                    #(#field_assignments,)*
                })
            }
        }
    })
}
