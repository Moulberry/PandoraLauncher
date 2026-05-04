use quote::quote;
use rust_i18n_support::{
    is_debug, load_locales, I18nConfig, DEFAULT_MINIFY_KEY, DEFAULT_MINIFY_KEY_LEN,
    DEFAULT_MINIFY_KEY_PREFIX, DEFAULT_MINIFY_KEY_THRESH,
};
use std::collections::BTreeMap;
use syn::{parse_macro_input, Expr, Ident, LitBool, LitStr, Token};

mod minify_key;
mod tr;

struct Args {
    locales_path: String,
    default_locale: Option<String>,
    fallback: Option<Vec<String>>,
    extend: Option<Expr>,
    minify_key: bool,
    minify_key_len: usize,
    minify_key_prefix: String,
    minify_key_thresh: usize,
}

impl Args {
    fn consume_path(&mut self, input: syn::parse::ParseStream) -> syn::parse::Result<()> {
        let path = input.parse::<LitStr>()?;
        self.locales_path = path.value();

        Ok(())
    }

    fn consume_fallback(&mut self, input: syn::parse::ParseStream) -> syn::parse::Result<()> {
        if let Ok(val) = input.parse::<LitStr>() {
            self.fallback = Some(vec![val.value()]);
            return Ok(());
        }
        let val = input.parse::<syn::ExprArray>()?;
        let fallback = val
            .elems
            .into_iter()
            .map(|expr| {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(lit_str),
                    ..
                }) = expr
                {
                    Ok(lit_str.value())
                } else {
                    Err(input.error(
                        "`fallback` must be a string literal or an array of string literals",
                    ))
                }
            })
            .collect::<syn::parse::Result<Vec<String>>>()?;
        self.fallback = Some(fallback);
        Ok(())
    }

    fn consume_minify_key(&mut self, input: syn::parse::ParseStream) -> syn::parse::Result<()> {
        let lit_bool = input.parse::<LitBool>()?;
        self.minify_key = lit_bool.value;
        Ok(())
    }

    fn consume_minify_key_len(&mut self, input: syn::parse::ParseStream) -> syn::parse::Result<()> {
        let lit_int = input.parse::<syn::LitInt>()?;
        self.minify_key_len = lit_int.base10_parse()?;
        Ok(())
    }

    fn consume_minify_key_prefix(
        &mut self,
        input: syn::parse::ParseStream,
    ) -> syn::parse::Result<()> {
        let lit_str = input.parse::<syn::LitStr>()?;
        self.minify_key_prefix = lit_str.value();
        Ok(())
    }

    fn consume_minify_key_thresh(
        &mut self,
        input: syn::parse::ParseStream,
    ) -> syn::parse::Result<()> {
        let lit_int = input.parse::<syn::LitInt>()?;
        self.minify_key_thresh = lit_int.base10_parse()?;
        Ok(())
    }

    fn consume_options(&mut self, input: syn::parse::ParseStream) -> syn::parse::Result<()> {
        let ident = input.parse::<Ident>()?.to_string();
        input.parse::<Token![=]>()?;

        // If there have any option in arguments, it will override the config from metadata
        match ident.as_str() {
            "fallback" => {
                self.consume_fallback(input)?;
            }
            "backend" => {
                let val = input.parse::<Expr>()?;
                self.extend = Some(val);
            }
            "minify_key" => {
                self.consume_minify_key(input)?;
            }
            "minify_key_len" => {
                self.consume_minify_key_len(input)?;
            }
            "minify_key_prefix" => {
                self.consume_minify_key_prefix(input)?;
            }
            "minify_key_thresh" => {
                self.consume_minify_key_thresh(input)?;
            }
            _ => {}
        }

        // Continue to consume reset of options
        if input.parse::<Token![,]>().is_ok() {
            self.consume_options(input)?;
        }

        Ok(())
    }

    /// Load the config from Cargo.toml. This can be overridden by subsequent options.
    fn load_metadata(&mut self, input: syn::parse::ParseStream) -> syn::parse::Result<()> {
        // CARGO_MANIFEST_DIR is current build directory
        if let Ok(cargo_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let current_dir = std::path::PathBuf::from(cargo_dir);
            let cfg = I18nConfig::load(&current_dir)
                .map_err(|_| input.error("Failed to load config from Cargo.toml for `metadata`"))?;

            self.locales_path = cfg.load_path;
            self.default_locale = Some(cfg.default_locale.clone());
            if !cfg.fallback.is_empty() {
                self.fallback = Some(cfg.fallback);
            }
            self.minify_key = cfg.minify_key;
            self.minify_key_len = cfg.minify_key_len;
            self.minify_key_prefix = cfg.minify_key_prefix;
            self.minify_key_thresh = cfg.minify_key_thresh;
        } else if rust_i18n_support::is_debug() {
            return Err(input.error("The CARGO_MANIFEST_DIR is required fo `metadata`"));
        }

        Ok(())
    }
}

impl syn::parse::Parse for Args {
    /// Parse macro arguments.
    ///
    /// ```no_run
    /// # use rust_i18n::i18n;
    /// # fn v1() {
    /// i18n!();
    /// # }
    /// # fn v2() {
    /// i18n!("locales");
    /// # }
    /// # fn v3() {
    /// i18n!("locales", fallback = "en");
    /// # }
    /// # fn v4() {
    /// i18n!("locales", fallback = ["en", "es"]);
    /// # }
    /// # fn v5() {
    /// i18n!("locales", fallback = ["en", "es"],
    ///       minify_key = true,
    ///       minify_key_len = 12,
    ///       minify_key_prefix = "t_",
    ///       minify_key_thresh = 64);
    /// # }
    /// ```
    ///
    /// Ref: https://docs.rs/syn/latest/syn/parse/index.html
    fn parse(input: syn::parse::ParseStream) -> syn::parse::Result<Self> {
        let lookahead = input.lookahead1();
        // The config from metadata is lower priority

        let mut result = Self {
            locales_path: String::from("locales"),
            default_locale: None,
            fallback: None,
            extend: None,
            minify_key: DEFAULT_MINIFY_KEY,
            minify_key_len: DEFAULT_MINIFY_KEY_LEN,
            minify_key_prefix: DEFAULT_MINIFY_KEY_PREFIX.to_owned(),
            minify_key_thresh: DEFAULT_MINIFY_KEY_THRESH,
        };

        result.load_metadata(input)?;

        if lookahead.peek(LitStr) {
            result.consume_path(input)?;

            if input.parse::<Token![,]>().is_ok() {
                result.consume_options(input)?;
            }
        } else if lookahead.peek(Ident) {
            result.consume_options(input)?;
        }

        Ok(result)
    }
}

/// Init I18n translations.
///
/// This will load all translations by glob `**/*.yml` from the given path, default: `${CARGO_MANIFEST_DIR}/locales`.
///
/// # Attributes
///
/// - `fallback` for set the fallback locale, if present [`t!`](macro.t.html) macro will use it as the fallback locale.
/// - `backend` for set the backend, if present [`t!`](macro.t.html) macro will use it as the backend.
/// - `metadata` to enable/disable loading of the [package.metadata.i18n] config from Cargo.toml, default: `true`.
/// - `minify_key` for enable/disable minify key, default: [`DEFAULT_MINIFY_KEY`](constant.DEFAULT_MINIFY_KEY.html).
/// - `minify_key_len` for set the minify key length, default: [`DEFAULT_MINIFY_KEY_LEN`](constant.DEFAULT_MINIFY_KEY_LEN.html),
///   * The range of available values is from `0` to `24`.
/// - `minify_key_prefix` for set the minify key prefix, default: [`DEFAULT_MINIFY_KEY_PREFIX`](constant.DEFAULT_MINIFY_KEY_PREFIX.html).
/// - `minify_key_thresh` for set the minify key threshold, default: [`DEFAULT_MINIFY_KEY_THRESH`](constant.DEFAULT_MINIFY_KEY_THRESH.html).
///   * If the length of the value is less than or equal to this value, the value will not be minified.
///
/// ```no_run
/// # use rust_i18n::i18n;
/// # fn v1() {
/// i18n!();
/// # }
/// # fn v2() {
/// i18n!("locales");
/// # }
/// # fn v3() {
/// i18n!("locales", fallback = "en");
/// # }
/// # fn v4() {
/// i18n!("locales", fallback = ["en", "es"]);
/// # }
/// # fn v5() {
/// i18n!("locales", fallback = ["en", "es"],
///       minify_key = true,
///       minify_key_len = 12,
///       minify_key_prefix = "t_",
///       minify_key_thresh = 64);
/// # }
/// # fn v6() {
/// i18n!();
/// # }
/// ```
#[proc_macro]
pub fn i18n(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let args = parse_macro_input!(input as Args);

    // CARGO_MANIFEST_DIR is current build directory
    let cargo_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is empty");
    let current_dir = std::path::PathBuf::from(cargo_dir);
    let locales_path = current_dir.join(&args.locales_path);

    let data = load_locales(&locales_path.display().to_string(), |_| false);
    let code = generate_code(data, args);

    if is_debug() {
        println!(
            "\n\n-------------- code --------------\n{}\n----------------------------------\n\n",
            code
        );
    }

    code.into()
}

fn generate_code(
    translations: BTreeMap<String, BTreeMap<String, String>>,
    args: Args,
) -> proc_macro2::TokenStream {
    let mut all_translations = Vec::<proc_macro2::TokenStream>::new();

    translations.iter().for_each(|(locale, trs)| {
        trs.iter().for_each(|(k, v)| {
            all_translations.push(quote! {
                backend.add_translations(#locale, &std::collections::HashMap::from([(#k, #v)]));
            });
        });
    });

    let default_locale = if let Some(default_locale) = args.default_locale {
        quote! {
            use std::ops::Deref;
            if #default_locale != rust_i18n::locale().deref() {
                rust_i18n::set_locale(rust_i18n::locale().deref());
            } else {
                rust_i18n::set_locale(#default_locale);
            }
        }
    } else {
        quote! {}
    };

    let fallback = if let Some(fallback) = args.fallback {
        quote! {
            Some(&[#(#fallback),*])
        }
    } else {
        quote! {
            None
        }
    };

    let extend_code = if let Some(extend) = args.extend {
        quote! {
            let backend = backend.extend(#extend);
        }
    } else {
        quote! {}
    };

    let minify_key = args.minify_key;
    let minify_key_len = args.minify_key_len;
    let minify_key_prefix = args.minify_key_prefix;
    let minify_key_thresh = args.minify_key_thresh;

    quote! {
        use rust_i18n::{BackendExt, CowStr, MinifyKey};
        use std::borrow::Cow;

        /// I18n backend instance
        ///
        /// [PUBLIC] This is a public API, and as an example in examples/
        #[allow(missing_docs)]
        static _RUST_I18N_BACKEND: rust_i18n::once_cell::sync::Lazy<Box<dyn rust_i18n::Backend>> = rust_i18n::once_cell::sync::Lazy::new(|| {
            let mut backend = rust_i18n::SimpleBackend::new();
            #(#all_translations)*
            #extend_code
            #default_locale

            Box::new(backend)
        });

        static _RUST_I18N_FALLBACK_LOCALE: Option<&[&'static str]> = #fallback;
        static _RUST_I18N_MINIFY_KEY: bool = #minify_key;
        static _RUST_I18N_MINIFY_KEY_LEN: usize = #minify_key_len;
        static _RUST_I18N_MINIFY_KEY_PREFIX: &str = #minify_key_prefix;
        static _RUST_I18N_MINIFY_KEY_THRESH: usize = #minify_key_thresh;

        /// Lookup fallback locales
        ///
        /// For example: `"zh-Hant-CN-x-private1-private2"` -> `"zh-Hant-CN-x-private1"` -> `"zh-Hant-CN"` -> `"zh-Hant"` -> `"zh"`.
        ///
        /// https://datatracker.ietf.org/doc/html/rfc4647#section-3.4
        #[inline]
        #[doc(hidden)]
        #[allow(missing_docs)]
        pub fn _rust_i18n_lookup_fallback(locale: &str) -> Option<&str> {
            locale.rfind('-').map(|n| locale[..n].trim_end_matches("-x"))
        }

        /// Get I18n text by locale and key
        #[inline]
        #[allow(missing_docs)]
        #[doc(hidden)]
        pub fn _rust_i18n_translate<'r>(locale: &str, key: &'r str) -> Cow<'r, str> {
            _rust_i18n_try_translate(locale, key).unwrap_or_else(|| {
                if locale.is_empty() {
                    key.into()
                } else {
                    format!("{}.{}", locale, key).into()
                }
            })
        }

        /// Try to get I18n text by locale and key
        #[inline]
        #[doc(hidden)]
        #[allow(missing_docs)]
        pub fn _rust_i18n_try_translate<'r>(locale: &str, key: impl AsRef<str>) -> Option<Cow<'r, str>> {
            _RUST_I18N_BACKEND.translate(locale, key.as_ref())
                .map(Cow::from)
                .or_else(|| {
                    let mut current_locale = locale;
                    while let Some(fallback_locale) = _rust_i18n_lookup_fallback(current_locale) {
                        if let Some(value) = _RUST_I18N_BACKEND.translate(fallback_locale, key.as_ref()) {
                            return Some(Cow::from(value));
                        }
                        current_locale = fallback_locale;
                    }

                    _RUST_I18N_FALLBACK_LOCALE.and_then(|fallback| {
                        fallback.iter().find_map(|locale| _RUST_I18N_BACKEND.translate(locale, key.as_ref()).map(Cow::from))
                    })
                })
        }

        #[inline]
        #[doc(hidden)]
        #[allow(missing_docs)]
        pub fn _rust_i18n_available_locales() -> Vec<&'static str> {
            let mut locales = _RUST_I18N_BACKEND.available_locales();
            locales.sort();
            locales
        }

        #[doc(hidden)]
        #[allow(unused_macros)]
        macro_rules! __rust_i18n_t {
            ($($all_tokens:tt)*) => {
                rust_i18n::_tr!($($all_tokens)*, _minify_key = #minify_key, _minify_key_len = #minify_key_len, _minify_key_prefix = #minify_key_prefix, _minify_key_thresh = #minify_key_thresh)
            }
        }

        #[doc(hidden)]
        #[allow(unused_macros)]
        macro_rules! __rust_i18n_tkv {
            ($msg:literal) => {
                {
                    let val = $msg;
                    let key = rust_i18n::_minify_key!($msg, #minify_key_len, #minify_key_prefix, #minify_key_thresh);
                    (key, val)
                }
            }
        }

        pub(crate) use __rust_i18n_t as _rust_i18n_t;
        pub(crate) use __rust_i18n_tkv as _rust_i18n_tkv;
    }
}

/// A procedural macro that generates a translation key from a value.
#[proc_macro]
#[doc(hidden)]
pub fn _minify_key(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    parse_macro_input!(input as minify_key::MinifyKey).into()
}

/// Export the `_tr!` macro for rust_i18n crate.
///
/// This macro first checks if a translation exists for the input string.
/// If it does, it returns the translated string.
/// If it does not, it returns the input value.
#[proc_macro]
#[doc(hidden)]
pub fn _tr(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    parse_macro_input!(input as tr::Tr).into()
}
