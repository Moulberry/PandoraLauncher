//! Procedural macro implementation for the `stacksafe` crate.
//!
//! This crate provides the `#[stacksafe]` attribute macro that transforms functions
//! to use automatic stack growth, preventing stack overflow in deeply recursive scenarios.

use proc_macro::TokenStream;
use proc_macro_error2::abort;
use proc_macro_error2::abort_call_site;
use proc_macro_error2::proc_macro_error;
use quote::ToTokens;
use quote::quote;
use syn::ItemFn;
use syn::Path;
use syn::ReturnType;
use syn::Type;
use syn::parse_macro_input;
use syn::parse_quote;

#[proc_macro_attribute]
#[proc_macro_error]
pub fn stacksafe(args: TokenStream, item: TokenStream) -> TokenStream {
    let mut crate_path: Option<Path> = None;

    let arg_parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("crate") {
            crate_path = Some(meta.value()?.parse()?);
            Ok(())
        } else {
            Err(meta.error(format!(
                "unknown attribute parameter `{}`",
                meta.path
                    .get_ident()
                    .map_or("unknown".to_string(), |i| i.to_string())
            )))
        }
    });
    parse_macro_input!(args with arg_parser);

    let item_fn: ItemFn = match syn::parse(item.clone()) {
        Ok(item) => item,
        Err(_) => abort_call_site!("#[stacksafe] can only be applied to functions"),
    };

    if item_fn.sig.asyncness.is_some() {
        abort!(
            item_fn.sig.asyncness,
            "#[stacksafe] does not support async functions"
        );
    }

    let mut item_fn = item_fn;
    let block = item_fn.block;
    let ret = match &item_fn.sig.output {
        // impl trait is not supported in closure return type, override with
        // default, which is inferring.
        ReturnType::Type(_, ty) if matches!(**ty, Type::ImplTrait(_)) => ReturnType::Default,
        _ => item_fn.sig.output.clone(),
    };

    let stacksafe_crate = crate_path.unwrap_or_else(|| parse_quote!(::stacksafe));

    let wrapped_block = quote! {
        {
            #stacksafe_crate::internal::stacker::maybe_grow(
                #stacksafe_crate::get_minimum_stack_size(),
                #stacksafe_crate::get_stack_allocation_size(),
                #stacksafe_crate::internal::with_protected(move || #ret { #block })
            )
        }
    };
    item_fn.block = Box::new(syn::parse(wrapped_block.into()).unwrap());
    item_fn.into_token_stream().into()
}
