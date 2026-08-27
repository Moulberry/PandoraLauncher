use crate::{case, def_attrs};

/// Parses the `crate` attribute value into a path.
pub fn parse_crate_path(crate_attr: Option<&str>) -> Result<Option<syn::Path>, syn::Error> {
    crate_attr.map(syn::parse_str).transpose()
}

/// Renames `ident` per the `rename`/`rename_all` attribute values, `rename` taking precedence.
pub fn rename_identifier(
    ident: String,
    span: proc_macro2::Span,
    rename_attr: Option<String>,
    rename_all_attr: Option<&str>,
) -> Result<String, syn::Error> {
    if let Some(name) = rename_attr {
        Ok(name)
    } else {
        match rename_all_attr {
            Some("lowercase") => Ok(ident.to_ascii_lowercase()),
            Some("UPPERCASE") => Ok(ident.to_ascii_uppercase()),
            Some("PascalCase") => Ok(case::pascal_or_camel_case(&ident, true)),
            Some("camelCase") => Ok(case::pascal_or_camel_case(&ident, false)),
            Some("snake_case") => Ok(case::snake_or_kebab_case(&ident, true)),
            Some("kebab-case") => Ok(case::snake_or_kebab_case(&ident, false)),
            None => Ok(ident),
            Some(other) => Err(syn::Error::new(
                span,
                format!("invalid `rename_all` attribute value {other}"),
            )),
        }
    }
}

// The generated `parse()` is hardwired to the `zbus`/`zvariant` lists named below, so shared
// codegen that may run under another namespace (e.g. a future `#[zgvariant(...)]`) must parse
// via `parse_with_lists(attrs, config.attr_lists)` instead, or it will silently ignore that
// namespace's attributes.
def_attrs! {
    crate zbus, zvariant;

    /// Attributes defined on structures.
    pub StructAttributes("struct") { signature str, rename_all str, deny_unknown_fields none, crate_path str };
    /// Attributes defined on fields.
    pub FieldAttributes("field") { rename str };
    /// Attributes defined on enumerations.
    pub EnumAttributes("enum") { signature str, rename_all str, crate_path str };
    /// Attributes defined on variants.
    pub VariantAttributes("variant") { rename str };
}
