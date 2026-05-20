use quote::quote;
use rust_i18n_support::minify_key;
use syn::Token;

/// A type representing the `minify_key!` proc macro.
#[derive(Clone, Debug, Default)]
pub struct MinifyKey {
    msg: String,
    len: usize,
    prefix: String,
    threshold: usize,
}

impl MinifyKey {
    fn into_token_stream(self) -> proc_macro2::TokenStream {
        let key = minify_key(&self.msg, self.len, &self.prefix, self.threshold);
        quote! { #key }
    }
}

impl syn::parse::Parse for MinifyKey {
    /// minify_key!("This is message", len = 24, prefix = "t_", threshold = 4)
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let msg = input.parse::<syn::LitStr>()?.value();
        let _comma = input.parse::<Token![,]>()?;
        let len: usize = input.parse::<syn::LitInt>()?.base10_parse()?;
        let _comma = input.parse::<Token![,]>()?;
        let prefix = input.parse::<syn::LitStr>()?.value();
        let _comma = input.parse::<Token![,]>()?;
        let threshold: usize = input.parse::<syn::LitInt>()?.base10_parse()?;
        Ok(Self {
            msg,
            len,
            prefix,
            threshold,
        })
    }
}

impl From<MinifyKey> for proc_macro::TokenStream {
    fn from(val: MinifyKey) -> Self {
        val.into_token_stream().into()
    }
}

impl From<MinifyKey> for proc_macro2::TokenStream {
    fn from(val: MinifyKey) -> Self {
        val.into_token_stream()
    }
}
