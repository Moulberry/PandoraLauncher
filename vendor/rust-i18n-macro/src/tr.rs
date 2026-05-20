use quote::{quote, ToTokens};
use rust_i18n_support::{
    MinifyKey, DEFAULT_MINIFY_KEY_LEN, DEFAULT_MINIFY_KEY_PREFIX, DEFAULT_MINIFY_KEY_THRESH,
};
use syn::{parse::discouraged::Speculative, token::Brace, Expr, Ident, LitStr, Token};

#[derive(Clone, Debug, Default)]
pub enum Value {
    #[default]
    Empty,
    Expr(Expr),
    Ident(Ident),
}

impl Value {
    fn is_expr_lit_str(&self) -> bool {
        if let Self::Expr(Expr::Lit(expr_lit)) = self {
            if let syn::Lit::Str(_) = &expr_lit.lit {
                return true;
            }
        }
        false
    }

    fn is_expr_tuple(&self) -> bool {
        if let Self::Expr(Expr::Tuple(_)) = self {
            return true;
        }
        false
    }

    fn to_string(&self) -> Option<String> {
        if let Self::Expr(Expr::Lit(expr_lit)) = self {
            if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                return Some(lit_str.value());
            }
        }
        None
    }

    fn to_tupled_token_streams(
        &self,
    ) -> syn::parse::Result<(proc_macro2::TokenStream, proc_macro2::TokenStream)> {
        if let Self::Expr(Expr::Tuple(expr_tuple)) = self {
            if expr_tuple.elems.len() == 2 {
                let first = expr_tuple.elems.first().map(|v| quote! { #v }).unwrap();
                let last = expr_tuple.elems.last().map(|v| quote! { #v }).unwrap();
                return Ok((first, last));
            }
        }
        Err(syn::Error::new_spanned(
            self,
            "Expected a tuple with two elements",
        ))
    }
}

impl From<Expr> for Value {
    fn from(expr: Expr) -> Self {
        Self::Expr(expr)
    }
}

impl From<Ident> for Value {
    fn from(ident: Ident) -> Self {
        Self::Ident(ident)
    }
}

impl quote::ToTokens for Value {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Self::Empty => {}
            Self::Expr(expr) => match expr {
                Expr::Path(path) => quote! { &#path }.to_tokens(tokens),
                expr => expr.to_tokens(tokens),
            },
            Self::Ident(ident) => quote! { &#ident }.to_tokens(tokens),
        }
    }
}

impl syn::parse::Parse for Value {
    fn parse(input: syn::parse::ParseStream) -> syn::parse::Result<Self> {
        let fork = input.fork();
        if let Ok(expr) = fork.parse::<Expr>() {
            input.advance_to(&fork);
            return Ok(expr.into());
        }
        let fork = input.fork();
        if let Ok(expr) = fork.parse::<Ident>() {
            input.advance_to(&fork);
            return Ok(expr.into());
        }
        Err(input.error("Expected a expression or an identifier"))
    }
}

#[derive(Clone, Default)]
pub struct Argument {
    pub name: String,
    pub value: Value,
    pub specifiers: Option<String>,
}

impl Argument {
    #[allow(dead_code)]
    pub fn value_string(&self) -> String {
        match &self.value {
            Value::Expr(Expr::Lit(expr_lit)) => match &expr_lit.lit {
                syn::Lit::Str(lit_str) => lit_str.value(),
                _ => self.value.to_token_stream().to_string(),
            },
            _ => self.value.to_token_stream().to_string(),
        }
    }

    fn try_ident(input: syn::parse::ParseStream) -> syn::parse::Result<String> {
        let fork = input.fork();
        let ident = fork.parse::<Ident>()?;
        input.advance_to(&fork);
        Ok(ident.to_string())
    }

    fn try_literal(input: syn::parse::ParseStream) -> syn::parse::Result<String> {
        let fork = input.fork();
        let lit = fork.parse::<LitStr>()?;
        input.advance_to(&fork);
        Ok(lit.value())
    }
}

impl syn::parse::Parse for Argument {
    fn parse(input: syn::parse::ParseStream) -> syn::parse::Result<Self> {
        // Ignore leading commas.
        while input.peek(Token![,]) {
            let _ = input.parse::<Token![,]>()?;
        }
        // Parse the argument name.
        let name = Self::try_ident(input)
            .or_else(|_| Self::try_literal(input))
            .map_err(|_| input.error("Expected a `string` literal or an identifier"))?;
        // Parse the separator between the name and the value.
        if input.peek(Token![=>]) {
            let _ = input.parse::<Token![=>]>()?;
        } else if input.peek(Token![=]) {
            let _ = input.parse::<Token![=]>()?;
        } else {
            return Err(input.error("Expected `=>` or `=`"));
        }
        // Parse the argument value.
        let value = input.parse()?;
        // Parse the specifiers [optinal].
        let specifiers = if input.peek(Token![:]) {
            let _ = input.parse::<Token![:]>()?;
            if input.peek(Brace) {
                let content;
                let _ = syn::braced!(content in input);
                let mut specifiers = String::new();
                while let Ok(s) = content.parse::<proc_macro2::TokenTree>() {
                    specifiers.push_str(&s.to_string());
                }
                Some(specifiers)
            } else {
                None
            }
        } else {
            None
        };
        Ok(Self {
            name,
            value,
            specifiers,
        })
    }
}

#[derive(Default)]
pub struct Arguments {
    pub args: Vec<Argument>,
}

impl Arguments {
    pub fn is_empty(&self) -> bool {
        self.args.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Argument> {
        self.args.iter()
    }

    pub fn keys(&self) -> Vec<String> {
        self.args.iter().map(|arg| arg.name.clone()).collect()
    }

    #[allow(dead_code)]
    pub fn values(&self) -> Vec<Value> {
        self.args.iter().map(|arg| arg.value.clone()).collect()
    }
}

impl AsRef<Vec<Argument>> for Arguments {
    fn as_ref(&self) -> &Vec<Argument> {
        &self.args
    }
}

impl AsMut<Vec<Argument>> for Arguments {
    fn as_mut(&mut self) -> &mut Vec<Argument> {
        &mut self.args
    }
}

impl syn::parse::Parse for Arguments {
    fn parse(input: syn::parse::ParseStream) -> syn::parse::Result<Self> {
        let args = input
            .parse_terminated(Argument::parse, Token![,])?
            .into_iter()
            .collect();
        Ok(Self { args })
    }
}

#[derive(Default)]
pub struct Messsage {
    #[allow(dead_code)]
    key: proc_macro2::TokenStream,
    val: Value,
}

impl Messsage {
    fn try_exp(input: syn::parse::ParseStream) -> syn::parse::Result<Self> {
        let fork = input.fork();
        let expr = fork.parse::<Expr>()?;
        input.advance_to(&fork);

        Ok(Self {
            key: Default::default(),
            val: Value::Expr(expr),
        })
    }

    fn try_ident(input: syn::parse::ParseStream) -> syn::parse::Result<Self> {
        let fork = input.fork();
        let ident = fork.parse::<Ident>()?;
        input.advance_to(&fork);
        Ok(Self {
            key: Default::default(),
            val: Value::Ident(ident),
        })
    }
}

impl syn::parse::Parse for Messsage {
    fn parse(input: syn::parse::ParseStream) -> syn::parse::Result<Self> {
        let result = Self::try_exp(input).or_else(|_| Self::try_ident(input))?;
        Ok(result)
    }
}

/// A type representing the `tr!` proc macro.
pub(crate) struct Tr {
    pub msg: Messsage,
    pub args: Arguments,
    pub locale: Option<Value>,
    pub minify_key: bool,
    pub minify_key_len: usize,
    pub minify_key_prefix: String,
    pub minify_key_thresh: usize,
}

impl Tr {
    fn new() -> Self {
        Self {
            msg: Messsage::default(),
            args: Arguments::default(),
            locale: None,
            minify_key: false,
            minify_key_len: DEFAULT_MINIFY_KEY_LEN,
            minify_key_prefix: DEFAULT_MINIFY_KEY_PREFIX.into(),
            minify_key_thresh: DEFAULT_MINIFY_KEY_THRESH,
        }
    }

    fn parse_minify_key(value: &Value) -> syn::parse::Result<bool> {
        if let Value::Expr(Expr::Lit(expr_lit)) = value {
            match &expr_lit.lit {
                syn::Lit::Bool(lit_bool) => {
                    return Ok(lit_bool.value);
                }
                syn::Lit::Str(lit_str) => {
                    let value = lit_str.value();
                    if ["true", "false", "yes", "no"].contains(&value.as_str()) {
                        return Ok(["true", "yes"].contains(&value.as_str()));
                    }
                }
                _ => {}
            }
        }
        Err(syn::Error::new_spanned(
            value,
            "`_minify_key` Expected a string literal in `true`, `false`, `yes`, `no`",
        ))
    }

    fn parse_minify_key_len(value: &Value) -> syn::parse::Result<usize> {
        if let Value::Expr(Expr::Lit(expr_lit)) = value {
            if let syn::Lit::Int(lit_int) = &expr_lit.lit {
                return Ok(lit_int.base10_parse().unwrap());
            }
        }
        Err(syn::Error::new_spanned(
            value,
            "`_minify_key_len` Expected a integer literal",
        ))
    }

    fn parse_minify_key_prefix(value: &Value) -> syn::parse::Result<String> {
        if let Value::Expr(Expr::Lit(expr_lit)) = value {
            if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                return Ok(lit_str.value());
            }
        }
        Err(syn::Error::new_spanned(
            value,
            "`_minify_key_prefix` Expected a string literal",
        ))
    }

    fn parse_minify_key_thresh(value: &Value) -> syn::parse::Result<usize> {
        if let Value::Expr(Expr::Lit(expr_lit)) = value {
            if let syn::Lit::Int(lit_int) = &expr_lit.lit {
                return Ok(lit_int.base10_parse().unwrap());
            }
        }
        Err(syn::Error::new_spanned(
            value,
            "`_minify_key_threshold` Expected a integer literal",
        ))
    }

    fn filter_arguments(&mut self) -> syn::parse::Result<()> {
        for arg in self.args.iter() {
            match arg.name.as_str() {
                "locale" => {
                    self.locale = Some(arg.value.clone());
                }
                "_minify_key" => {
                    self.minify_key = Self::parse_minify_key(&arg.value)?;
                }
                "_minify_key_len" => {
                    self.minify_key_len = Self::parse_minify_key_len(&arg.value)?;
                }
                "_minify_key_prefix" => {
                    self.minify_key_prefix = Self::parse_minify_key_prefix(&arg.value)?;
                }
                "_minify_key_thresh" => {
                    self.minify_key_thresh = Self::parse_minify_key_thresh(&arg.value)?;
                }
                _ => {}
            }
        }

        self.args.as_mut().retain(|v| {
            ![
                "locale",
                "_minify_key",
                "_minify_key_len",
                "_minify_key_prefix",
                "_minify_key_thresh",
            ]
            .contains(&v.name.as_str())
        });

        Ok(())
    }

    #[cfg(feature = "log-miss-tr")]
    fn log_missing() -> proc_macro2::TokenStream {
        quote! {
            log::log!(target: "rust-i18n", log::Level::Warn, "missing: {} => {:?} @ {}:{}", msg_key, msg_val, file!(), line!());
        }
    }

    #[cfg(not(feature = "log-miss-tr"))]
    fn log_missing() -> proc_macro2::TokenStream {
        quote! {}
    }

    fn into_token_stream(self) -> proc_macro2::TokenStream {
        let (msg_key, msg_val) = if self.minify_key && self.msg.val.is_expr_lit_str() {
            let msg_val = self.msg.val.to_string().unwrap();
            let msg_key = MinifyKey::minify_key(
                &msg_val,
                self.minify_key_len,
                self.minify_key_prefix.as_str(),
                self.minify_key_thresh,
            );
            (quote! { #msg_key }, quote! { #msg_val })
        } else if self.minify_key && self.msg.val.is_expr_tuple() {
            self.msg.val.to_tupled_token_streams().unwrap()
        } else if self.minify_key {
            let minify_key_len = self.minify_key_len;
            let minify_key_prefix = self.minify_key_prefix;
            let minify_key_thresh = self.minify_key_thresh;
            let msg_val = self.msg.val.to_token_stream();
            let msg_key = quote! { rust_i18n::MinifyKey::minify_key(&msg_val, #minify_key_len, #minify_key_prefix, #minify_key_thresh) };
            (msg_key, msg_val)
        } else {
            let msg_val = self.msg.val.to_token_stream();
            let msg_key = quote! { &msg_val };
            (msg_key, msg_val)
        };
        let locale = self.locale.map_or_else(
            || quote! { &rust_i18n::locale() },
            |locale| quote! { #locale },
        );
        let keys: Vec<_> = self.args.keys().iter().map(|v| quote! { #v }).collect();
        let values: Vec<_> = self
            .args
            .as_ref()
            .iter()
            .map(|v| {
                let value = &v.value;
                let sepecifiers = v
                    .specifiers
                    .as_ref()
                    .map_or("{}".to_owned(), |s| format!("{{{}}}", s));
                quote! { format!(#sepecifiers, #value) }
            })
            .collect();
        let logging = Self::log_missing();
        if self.args.is_empty() {
            quote! {
                {
                    let msg_val = #msg_val;
                    let msg_key = #msg_key;
                    if let Some(translated) = crate::_rust_i18n_try_translate(#locale, &msg_key) {
                        translated.into()
                    } else {
                        #logging
                        rust_i18n::CowStr::from(msg_val).into_inner()
                    }
                }
            }
        } else {
            quote! {
                {
                    let msg_val = #msg_val;
                    let msg_key = #msg_key;
                    let keys = &[#(#keys),*];
                    let values = &[#(#values),*];
                    {
                    if let Some(translated) = crate::_rust_i18n_try_translate(#locale, &msg_key) {
                        let replaced = rust_i18n::replace_patterns(&translated, keys, values);
                        std::borrow::Cow::from(replaced)
                    } else {
                        #logging
                        let replaced = rust_i18n::replace_patterns(rust_i18n::CowStr::from(msg_val).as_str(), keys, values);
                        std::borrow::Cow::from(replaced)
                    }
                }
                }
            }
        }
    }
}

impl Default for Tr {
    fn default() -> Self {
        Self::new()
    }
}

impl syn::parse::Parse for Tr {
    fn parse(input: syn::parse::ParseStream) -> syn::parse::Result<Self> {
        let msg = input.parse::<Messsage>()?;
        let comma = input.parse::<Option<Token![,]>>()?;
        let args = if comma.is_some() {
            input.parse::<Arguments>()?
        } else {
            Arguments::default()
        };

        let mut result = Self {
            msg,
            args,
            ..Self::new()
        };

        result.filter_arguments()?;

        Ok(result)
    }
}

impl From<Tr> for proc_macro::TokenStream {
    fn from(args: Tr) -> Self {
        args.into_token_stream().into()
    }
}

impl From<Tr> for proc_macro2::TokenStream {
    fn from(args: Tr) -> Self {
        args.into_token_stream()
    }
}
