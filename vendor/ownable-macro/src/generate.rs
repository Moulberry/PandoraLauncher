use crate::derive::Derive;
use crate::mode::Mode;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::iter::once;
use syn::token::{Colon, Where};
use syn::{
    GenericParam, Generics, Lifetime, LifetimeParam, Path, PathSegment, PredicateType, Type,
    TypeParam, TypeParamBound, TypePath, WhereClause, WherePredicate,
};

impl Derive<'_> {
    pub(crate) fn generate(&mut self, inner: &TokenStream) -> TokenStream {
        match self.mode {
            Mode::ToBorrowed => self.generate_mode_to_borrowed(inner),
            Mode::ToOwned | Mode::IntoOwned => self.generate_mode_in_to_owned(inner),
        }
    }

    fn generate_mode_to_borrowed(&mut self, inner: &TokenStream) -> TokenStream {
        if self.generics.lifetimes().count() == 0 {
            self.error(
                self.ident,
                "ToBorrowed can be only derived for a struct/enum with a lifetime",
            );
        }

        let lifetime_our = &self.attribute.new_lifetime(self);
        let lifetime_placeholder = &Lifetime::new("'_", Span::call_site());
        let mut generics_definition = self.generate_generics(None);
        generics_definition
            .params
            .insert(0, LifetimeParam::new(lifetime_our.clone()).into());
        let generics_our = self.generate_generics(Some(lifetime_our));
        let generics_placeholder = self.generate_generics(Some(lifetime_placeholder));
        let generics_where = self.generate_where(lifetime_our);

        let name = self.ident;
        let trait_name = Mode::ToBorrowed.name();
        let doc = Mode::ToBorrowed.doc();
        let function = if self.attribute.function.unwrap_or(true) {
            quote! {
                impl #generics_definition #name #generics_our #generics_where
                {
                    #[doc=#doc]
                    #[inline(always)]
                    pub fn to_borrowed(&self) -> #name #generics_placeholder {
                        #trait_name::to_borrowed(self)
                    }
                }
            }
        } else {
            TokenStream::default()
        };

        quote! {
            impl #generics_definition #trait_name <#lifetime_our> for #name #generics_our #generics_where
            {
                fn to_borrowed(&#lifetime_our self) -> Self {
                    #inner
                }
            }

            #function
        }
    }

    fn generate_mode_in_to_owned(&mut self, inner: &TokenStream) -> TokenStream {
        let lifetime_placeholder = &Lifetime::new("'_", Span::call_site());
        let lifetime_static = &Lifetime::new("'static", Span::call_site());
        let generics_definition = self.generate_generics(None);
        let generics_placeholder = self.generate_generics(Some(lifetime_placeholder));
        let generics_static = self.generate_generics(Some(lifetime_static));
        let generics_where = self.generate_where(lifetime_static);

        let name = self.ident;
        let trait_name = self.mode.name();
        let trait_function = self.mode.function();
        let as_ref = self.mode.as_ref();
        let doc = self.mode.doc();
        let function = if self.attribute.function.unwrap_or(true) {
            quote! {
                impl #generics_definition #name #generics_placeholder #generics_where
                {
                    #[doc=#doc]
                    #[inline(always)]
                    pub fn #trait_function(#as_ref self) -> #name #generics_static {
                        #trait_name::#trait_function(self)
                    }
                }
            }
        } else {
            TokenStream::default()
        };

        quote! {
            impl #generics_definition #trait_name for #name #generics_placeholder #generics_where
            {
                type Owned = #name #generics_static;
                fn #trait_function(#as_ref self) -> Self::Owned {
                    #inner
                }
            }

            #function
        }
    }

    fn generate_generics(&self, lt: Option<&Lifetime>) -> Generics {
        let mut gen = Generics::default();

        for gp in &self.generics.params {
            match gp {
                GenericParam::Lifetime(l) => {
                    if self.attribute.is_reference_lifetime(&l.lifetime.ident) {
                        gen.params.push(l.clone().into());
                    } else if let Some(lt) = lt {
                        gen.params.push(LifetimeParam::new(lt.clone()).into());
                    }
                }
                GenericParam::Type(t) => {
                    gen.params.push(TypeParam::from(t.ident.clone()).into());
                }
                GenericParam::Const(c) => {
                    if lt.is_none() {
                        // This happens exactly for the definitions of the generics (e.g. `N`)
                        gen.params.push(c.clone().into());
                    } else {
                        // This happens for all uses of the generics (e.g. `const N: usize`)
                        gen.params.push(TypeParam::from(c.ident.clone()).into());
                    }
                }
            }
        }

        gen
    }

    fn generate_where(&mut self, lt: &Lifetime) -> WhereClause {
        let mut w = Vec::new();

        for gp in &self.generics.params {
            match gp {
                GenericParam::Type(t) => {
                    w.push(WherePredicate::Type(PredicateType {
                        lifetimes: None,
                        bounded_ty: Type::Path(TypePath {
                            qself: None,
                            path: Path {
                                leading_colon: None,
                                segments: once(PathSegment::from(t.ident.clone())).collect(),
                            },
                        }),
                        colon_token: Colon::default(),
                        bounds: t.bounds.iter().cloned().collect(),
                    }));
                }
                GenericParam::Lifetime(_) | GenericParam::Const(_) => {}
            }
        }

        if let Some(wc) = &self.generics.where_clause {
            w.extend(wc.predicates.iter().cloned());
        }

        WhereClause {
            where_token: Where::default(),
            predicates: w
                .into_iter()
                .filter_map(|wp| self.set_lifetime(lt, wp))
                .collect(),
        }
    }

    fn set_lifetime(&mut self, lt: &Lifetime, wp: WherePredicate) -> Option<WherePredicate> {
        match wp {
            WherePredicate::Lifetime(mut l) => {
                if self.attribute.is_reference_lifetime(&l.lifetime.ident) {
                    l.bounds = l
                        .bounds
                        .into_iter()
                        .map(|l| self.reference_lifetime_or_clone(l, lt))
                        .collect();
                    Some(WherePredicate::Lifetime(l))
                } else {
                    None
                }
            }
            WherePredicate::Type(mut pt) => {
                pt.lifetimes = None;
                pt.bounds = pt
                    .bounds
                    .into_iter()
                    .map(|tpb| match tpb {
                        TypeParamBound::Lifetime(l) => {
                            TypeParamBound::Lifetime(self.reference_lifetime_or_clone(l, lt))
                        }
                        t => t,
                    })
                    .collect();
                Some(WherePredicate::Type(pt))
            }
            // When stable, enable: #[cfg_attr(test, deny(non_exhaustive_omitted_patterns))]
            _ => self.error_with(&wp, "Unsupported WherePredicate", None),
        }
    }

    fn reference_lifetime_or_clone(&self, l: Lifetime, lt: &Lifetime) -> Lifetime {
        if self.attribute.is_reference_lifetime(&l.ident) {
            l
        } else {
            lt.clone()
        }
    }
}
