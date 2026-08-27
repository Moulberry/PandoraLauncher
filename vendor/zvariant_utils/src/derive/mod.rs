//! Code generation shared by the `zvariant_derive` and `zgvariant_derive` proc-macro crates.

use proc_macro2::TokenStream;
use quote::quote;

pub mod attrs;
pub use attrs::*;

mod dict;
pub use dict::{expand_deserialize_dict_derive, expand_serialize_dict_derive};

mod signature;
pub use signature::*;

mod r#type;
pub use r#type::expand_type_derive;

mod value;
pub use value::{ValueType, expand_value_derive};

/// Configuration for a derive crate using this code generation.
pub struct Config {
    /// Attribute list names to parse, e.g. `["zvariant", "zbus"]`.
    pub attr_lists: &'static [&'static str],
    /// Crate path used when the user doesn't override it via the `crate` attribute.
    pub default_path: TokenStream,
}

impl Config {
    /// The crate path to emit, honoring the user's `crate` attribute override.
    pub fn resolve_path(&self, crate_attr: Option<&str>) -> syn::Result<TokenStream> {
        Ok(match attrs::parse_crate_path(crate_attr)? {
            Some(path) => quote! { ::#path },
            None => self.default_path.clone(),
        })
    }
}
