// to silence an clippy from within FromDeriveInput
#![allow(clippy::needless_continue)]

use crate::derive::Derive;
use darling::util::SpannedValue;
use darling::{Error, FromAttributes, FromDeriveInput, FromMeta};
use proc_macro2::{Ident, Span};
use syn::{GenericParam, Generics, Lifetime};

//
// DeriveAttribute
//

#[derive(Default, Debug, FromDeriveInput)]
#[darling(default, attributes(ownable), supports(struct_any, enum_any))]
pub(crate) struct DeriveAttribute {
    #[darling(default, rename = "reference")]
    reference_lifetime: SpannedValue<LifetimesAttribute>,
    pub(crate) clone: Option<bool>,
    pub(crate) function: Option<bool>,
}

impl DeriveAttribute {
    pub(crate) fn is_reference_lifetime(&self, ident: &Ident) -> bool {
        self.reference_lifetime.0.iter().any(|kl| ident == kl)
    }

    pub(crate) fn new_lifetime(&self, derive: &mut Derive) -> Lifetime {
        const TRY_LIFETIME: &str = "ownable";

        for l in TRY_LIFETIME
            .char_indices()
            .map(move |(pos, _)| &TRY_LIFETIME[..=pos])
        {
            if !self.reference_lifetime.0.iter().any(|kl| l == kl) {
                return Lifetime::new(&format!("'{l}"), Span::call_site());
            }
        }

        derive.error_with(
            &Span::call_site(),
            "all of the following lifetimes are already used: 'o, 'ow, .. 'ownable",
            Lifetime::new("'error", Span::call_site()),
        )
    }

    fn contains_lifetime(generics: &Generics, lifetime: &str) -> bool {
        generics.params.iter().any(|p| match p {
            GenericParam::Lifetime(l) => l.lifetime.ident == lifetime,
            GenericParam::Type(_) | GenericParam::Const(_) => false,
        })
    }
}

//
// FieldAttribute
//

#[derive(Default, FromAttributes)]
#[darling(attributes(ownable))]
pub(crate) struct FieldAttribute {
    pub(crate) clone: Option<bool>,
}

pub(crate) trait OrAssign<Rhs> {
    fn or_assign(&mut self, rhs: Rhs);
}

impl OrAssign<&FieldAttribute> for FieldAttribute {
    fn or_assign(&mut self, rhs: &FieldAttribute) {
        self.clone = self.clone.or(rhs.clone);
    }
}

impl OrAssign<&DeriveAttribute> for FieldAttribute {
    fn or_assign(&mut self, rhs: &DeriveAttribute) {
        self.clone = self.clone.or(rhs.clone);
    }
}

//
// LifetimesAttribute
//

#[derive(Default, Debug)]
pub(crate) struct LifetimesAttribute(pub(crate) Vec<String>);

impl FromMeta for LifetimesAttribute {
    fn from_string(value: &str) -> Result<Self, Error> {
        Ok(Self(
            value
                .split(',')
                .map(|l| match l.trim().strip_prefix('\'') {
                    None => Err(Error::custom("not a lifetime")),
                    Some(lt) => Ok(lt.to_string()),
                })
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
}

impl Derive<'_> {
    pub(crate) fn verify_lifetimes(&mut self) {
        for lt in &self.attribute.reference_lifetime.0 {
            if !DeriveAttribute::contains_lifetime(self.generics, lt) {
                self.error(
                    &self.attribute.reference_lifetime.span(),
                    format!("lifetime \"'{lt}\" is not used"),
                );
            }
        }
    }
}
