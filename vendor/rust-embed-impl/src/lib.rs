#![recursion_limit = "1024"]
#![forbid(unsafe_code)]
#[macro_use]
extern crate quote;
extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use rust_embed_utils::PathMatcher;
use std::{
  collections::BTreeMap,
  env,
  io::ErrorKind,
  iter::FromIterator,
  path::{Path, PathBuf},
};
use syn::{parse_macro_input, Data, DeriveInput, Expr, ExprLit, Fields, Lit, Meta, MetaNameValue};

#[allow(clippy::too_many_arguments)]
fn embedded(
  ident: &syn::Ident, relative_folder_path: Option<&str>, absolute_folder_path: Option<String>, prefix: Option<&str>, includes: &[String], excludes: &[String],
  metadata_only: bool, crate_path: &syn::Path, compression: &str,
) -> syn::Result<TokenStream2> {
  extern crate rust_embed_utils;

  let mut match_values = BTreeMap::new();
  let mut list_values = Vec::<String>::new();

  let includes: Vec<&str> = includes.iter().map(AsRef::as_ref).collect();
  let excludes: Vec<&str> = excludes.iter().map(AsRef::as_ref).collect();
  let matcher = PathMatcher::new(&includes, &excludes);
  let entries = absolute_folder_path.clone()
    .map(|v| rust_embed_utils::get_files(v, matcher))
    .into_iter()
    .flatten();
  for rust_embed_utils::FileEntry { rel_path, full_canonical_path } in entries {
    match_values.insert(
      rel_path.clone(),
      embed_file(relative_folder_path, ident, &rel_path, &full_canonical_path, metadata_only, crate_path, compression)?,
    );

    list_values.push(if let Some(prefix) = prefix {
      format!("{}{}", prefix, rel_path)
    } else {
      rel_path
    });
  }

  let array_len = list_values.len();

  // If debug-embed is on, unconditionally include the code below. Otherwise,
  // make it conditional on cfg(not(debug_assertions)).
  let not_debug_attr = if cfg!(feature = "debug-embed") {
    quote! {}
  } else {
    quote! { #[cfg(not(debug_assertions))]}
  };

  let handle_prefix = if let Some(prefix) = prefix {
    quote! {
      let file_path = file_path.strip_prefix(#prefix)?;
    }
  } else {
    TokenStream2::new()
  };
  let match_values = match_values.into_iter().map(|(path, bytes)| {
    quote! {
        (#path, #bytes),
    }
  });
  let use_compression = cfg!(feature = "compression") && !metadata_only;
  let (entry_type, return_type) = if use_compression {
    (
      quote! { fn() -> #crate_path::EmbeddedCompressedFile },
      quote! { #crate_path::EmbeddedCompressedFile },
    )
  } else {
    (quote! { #crate_path::EmbeddedFile }, quote! { #crate_path::EmbeddedFile })
  };
  let get_value = if use_compression {
    quote! {|idx| (ENTRIES[idx].1)()}
  } else {
    quote! {|idx| ENTRIES[idx].1.clone()}
  };
  let impls = if cfg!(feature = "compression") {
    if metadata_only {
      quote! {
          pub fn compressed(_file_path: &str) -> ::std::option::Option<#crate_path::EmbeddedCompressedFile> {
            ::std::option::Option::None
          }
          pub fn get(file_path: &str) -> ::std::option::Option<#crate_path::EmbeddedFile> {
            #ident::__file(file_path)
          }
      }
    } else {
      quote! {
          pub fn compressed(file_path: &str) -> ::std::option::Option<#crate_path::EmbeddedCompressedFile> {
            #ident::__file(file_path)
          }
          pub fn get(file_path: &str) -> ::std::option::Option<#crate_path::EmbeddedFile> {
            #ident::__file(file_path).map(|f| #crate_path::EmbeddedFile {
                data: f.data.decoded().into(),
                metadata: f.metadata,
            })
          }
      }
    }
  } else {
    quote! {
        /// Get an embedded file and its metadata.
        pub fn get(file_path: &str) -> ::std::option::Option<#crate_path::EmbeddedFile> {
          #ident::__file(file_path)
        }
    }
  };
  let compressed_trait_impl = if cfg!(feature = "compression") {
    quote! {
      fn compressed(file_path: &str) -> ::std::option::Option<#crate_path::EmbeddedCompressedFile> {
        #ident::compressed(file_path)
      }
    }
  } else {
    TokenStream2::new()
  };
  Ok(quote! {
      #not_debug_attr
      impl #ident {
          fn __file(file_path: &str) -> ::std::option::Option<#return_type> {
              #handle_prefix
              let key = file_path.replace("\\", "/");
              static ENTRIES: &'static [(&'static str, #entry_type)] = &[
                  #(#match_values)*];
              let position = ENTRIES.binary_search_by_key(&key.as_str(), |entry| entry.0);
              position.ok().map(#get_value)
          }

          #impls

          fn names() -> ::std::slice::Iter<'static, &'static str> {
              const ITEMS: [&str; #array_len] = [#(#list_values),*];
              ITEMS.iter()
          }

          /// Iterates over the file paths in the folder.
          pub fn iter() -> impl ::std::iter::Iterator<Item = ::std::borrow::Cow<'static, str>> + 'static {
              Self::names().map(|x| ::std::borrow::Cow::from(*x))
          }
      }

      #not_debug_attr
      impl #crate_path::RustEmbed for #ident {
        #compressed_trait_impl
        fn get(file_path: &str) -> ::std::option::Option<#crate_path::EmbeddedFile> {
          #ident::get(file_path)
        }
        fn iter() -> #crate_path::Filenames {
          #crate_path::Filenames::Embedded(#ident::names())
        }
      }
  })
}

fn dynamic(
  ident: &syn::Ident, folder_path: Option<String>, prefix: Option<&str>, includes: &[String], excludes: &[String], metadata_only: bool, crate_path: &syn::Path,
) -> TokenStream2 {
  let dynamic_compressed = if cfg!(feature = "compression") {
    quote! {
      fn compressed(_file_path: &str) -> ::std::option::Option<#crate_path::EmbeddedCompressedFile> {
        ::std::option::Option::None
      }
    }
  } else {
    TokenStream2::new()
  };
  let (handle_prefix, map_iter) = if let ::std::option::Option::Some(prefix) = prefix {
    (
      quote! { let file_path = file_path.strip_prefix(#prefix)?; },
      quote! { ::std::borrow::Cow::Owned(format!("{}{}", #prefix, e.rel_path)) },
    )
  } else {
    (TokenStream2::new(), quote! { ::std::borrow::Cow::from(e.rel_path) })
  };

  let declare_includes = quote! {
    const INCLUDES: &[&str] = &[#(#includes),*];
  };

  let declare_excludes = quote! {
    const EXCLUDES: &[&str] = &[#(#excludes),*];
  };

  // In metadata_only mode, we still need to read file contents to generate the
  // file hash, but then we drop the file data.
  let strip_contents = metadata_only.then_some(quote! {
      .map(|mut file| { file.data = ::std::default::Default::default(); file })
  });

  let methods = if let Some(ref folder_path) = folder_path {
    let non_canonical_folder_path = Path::new(&folder_path);
    let canonical_folder_path = non_canonical_folder_path
      .canonicalize()
      .or_else(|err| match err {
        err if err.kind() == ErrorKind::NotFound => Ok(non_canonical_folder_path.to_owned()),
        err => Err(err),
      })
      .expect("folder path must resolve to an absolute path");
    let canonical_folder_path = canonical_folder_path.to_str().expect("absolute folder path must be valid unicode");
    quote! {
          /// Get an embedded file and its metadata.
          pub fn get(file_path: &str) -> ::std::option::Option<#crate_path::EmbeddedFile> {
              #handle_prefix

              let rel_file_path = file_path.replace("\\", "/");
              let file_path = ::std::path::Path::new(#folder_path).join(&rel_file_path);

              // Make sure the path requested does not escape the folder path
              let canonical_file_path = file_path.canonicalize().ok()?;
              if !canonical_file_path.starts_with(#canonical_folder_path) {
                  // Tried to request a path that is not in the embedded folder

                  // TODO: Currently it allows "path_traversal_attack" for the symlink files
                  // For it to be working properly we need to get absolute path first
                  // and check that instead if it starts with `canonical_folder_path`
                  // https://doc.rust-lang.org/std/path/fn.absolute.html (currently nightly)
                  // Should be allowed only if it was a symlink
                  let metadata = ::std::fs::symlink_metadata(&file_path).ok()?;
                  if !metadata.is_symlink() {
                    return ::std::option::Option::None;
                  }
              }
              let path_matcher = Self::matcher();
              if path_matcher.is_path_included(&rel_file_path) {
                #crate_path::utils::read_file_from_fs(&canonical_file_path).ok() #strip_contents
              } else {
                ::std::option::Option::None
              }
          }

          /// Iterates over the file paths in the folder.
          pub fn iter() -> impl ::std::iter::Iterator<Item = ::std::borrow::Cow<'static, str>> + 'static {
              use ::std::path::Path;

              #crate_path::utils::get_files(::std::string::String::from(#folder_path), Self::matcher())
                  .map(|e| #map_iter)
          }
    }
  } else {
    quote! {
          /// Get an embedded file and its metadata.
          pub fn get(file_path: &str) -> ::std::option::Option<#crate_path::EmbeddedFile> {
              // No folder_path defined so nothing can be returned.
              None
          }

          /// Iterates over the file paths in the folder.
          pub fn iter() -> impl ::std::iter::Iterator<Item = ::std::borrow::Cow<'static, str>> + 'static {
              None.into_iter()
          }
    }
  };

  quote! {
      #[cfg(debug_assertions)]
      impl #ident {


        fn matcher() -> #crate_path::utils::PathMatcher {
            #declare_includes
            #declare_excludes
            static PATH_MATCHER: ::std::sync::OnceLock<#crate_path::utils::PathMatcher> = ::std::sync::OnceLock::new();
            PATH_MATCHER.get_or_init(|| #crate_path::utils::PathMatcher::new(INCLUDES, EXCLUDES)).clone()
        }

          #methods
      }

      #[cfg(debug_assertions)]
      impl #crate_path::RustEmbed for #ident {
        #dynamic_compressed
        fn get(file_path: &str) -> ::std::option::Option<#crate_path::EmbeddedFile> {
          #ident::get(file_path)
        }
        fn iter() -> #crate_path::Filenames {
          // the return type of iter() is unnamable, so we have to box it
          #crate_path::Filenames::Dynamic(::std::boxed::Box::new(#ident::iter()))
        }
      }
  }
}

fn generate_assets(
  ident: &syn::Ident, relative_folder_path: Option<&str>, absolute_folder_path: Option<String>, prefix: Option<String>, includes: Vec<String>, excludes: Vec<String>,
  metadata_only: bool, crate_path: &syn::Path, compression: &str,
) -> syn::Result<TokenStream2> {
  let embedded_impl = embedded(
    ident,
    relative_folder_path,
    absolute_folder_path.clone(),
    prefix.as_deref(),
    &includes,
    &excludes,
    metadata_only,
    crate_path,
    compression,
  );
  if cfg!(feature = "debug-embed") {
    return embedded_impl;
  }
  let embedded_impl = embedded_impl?;
  let dynamic_impl = dynamic(ident, absolute_folder_path, prefix.as_deref(), &includes, &excludes, metadata_only, crate_path);

  Ok(quote! {
      #embedded_impl
      #dynamic_impl
  })
}

fn embed_file(
  folder_path: Option<&str>, ident: &syn::Ident, rel_path: &str, full_canonical_path: &str, metadata_only: bool, crate_path: &syn::Path, compression: &str,
) -> syn::Result<TokenStream2> {
  let file = rust_embed_utils::read_file_from_fs(Path::new(full_canonical_path)).expect("File should be readable");
  let hash = file.metadata.sha256_hash();
  let (last_modified, created) = if cfg!(feature = "deterministic-timestamps") {
    (quote! { ::std::option::Option::Some(0u64) }, quote! { ::std::option::Option::Some(0u64) })
  } else {
    let last_modified = match file.metadata.last_modified() {
      Some(last_modified) => quote! { ::std::option::Option::Some(#last_modified) },
      None => quote! { ::std::option::Option::None },
    };
    let created = match file.metadata.created() {
      Some(created) => quote! { ::std::option::Option::Some(#created) },
      None => quote! { ::std::option::Option::None },
    };
    (last_modified, created)
  };
  let embedding_code = if metadata_only {
    quote! {
        const BYTES: &'static [u8] = &[];
    }
  } else if cfg!(feature = "compression") {
    let folder_path = folder_path.ok_or(syn::Error::new(ident.span(), "`folder` must be provided under `compression` feature."))?;
    let full_relative_path = PathBuf::from_iter([folder_path, rel_path]);
    let full_relative_path = full_relative_path.to_string_lossy();
    let compression_ident = format_ident!("{}", compression);
    quote! {
      #crate_path::flate!(static BYTES: IFlate from #full_relative_path with #compression_ident);
    }
  } else {
    quote! {
      const BYTES: &'static [u8] = include_bytes!(#full_canonical_path);
    }
  };
  Ok(if cfg!(feature = "compression") && !metadata_only {
    quote! { || {
        #embedding_code

        #crate_path::EmbeddedCompressedFile {
            data: &*BYTES,
            metadata: #crate_path::utils::__rust_embed_metadata!([#(#hash),*], #last_modified, #created, #crate_path::__mimetype_of!(#full_canonical_path))
        }
    } }
  } else {
    quote! {
      {
        #embedding_code

        #crate_path::EmbeddedFile {
            data: ::std::borrow::Cow::Borrowed(BYTES),
            metadata: #crate_path::utils::__rust_embed_metadata!([#(#hash),*], #last_modified, #created, #crate_path::__mimetype_of!(#full_canonical_path))
        }
      }
    }
  })
}

/// Find all pairs of the `name = "value"` attribute from the derive input
fn find_attribute_values(ast: &syn::DeriveInput, attr_name: &str) -> Vec<String> {
  ast
    .attrs
    .iter()
    .filter(|value| value.path().is_ident(attr_name))
    .filter_map(|attr| match &attr.meta {
      Meta::NameValue(MetaNameValue {
        value: Expr::Lit(ExprLit { lit: Lit::Str(val), .. }),
        ..
      }) => Some(val.value()),
      _ => None,
    })
    .collect()
}

fn find_bool_attribute(ast: &syn::DeriveInput, attr_name: &str) -> Option<bool> {
  ast
    .attrs
    .iter()
    .find(|value| value.path().is_ident(attr_name))
    .and_then(|attr| match &attr.meta {
      Meta::NameValue(MetaNameValue {
        value: Expr::Lit(ExprLit { lit: Lit::Bool(val), .. }),
        ..
      }) => Some(val.value()),
      _ => None,
    })
}

fn impl_rust_embed(ast: &syn::DeriveInput) -> syn::Result<TokenStream2> {
  match ast.data {
    Data::Struct(ref data) => match data.fields {
      Fields::Unit => {}
      _ => return Err(syn::Error::new_spanned(ast, "RustEmbed can only be derived for unit structs")),
    },
    _ => return Err(syn::Error::new_spanned(ast, "RustEmbed can only be derived for unit structs")),
  };

  let crate_path: syn::Path = find_attribute_values(ast, "crate_path")
    .last()
    .map_or_else(|| syn::parse_str("rust_embed").unwrap(), |v| syn::parse_str(v).unwrap());

  let mut folder_paths = find_attribute_values(ast, "folder");
  if folder_paths.len() != 1 {
    return Err(syn::Error::new_spanned(
      ast,
      "#[derive(RustEmbed)] must contain one attribute like this #[folder = \"examples/public/\"]",
    ));
  }
  let folder_path = folder_paths.remove(0);

  const COMPRESSION_ALGOS: &[&str] = &["deflate", "zstd"];
  let mut compression = find_attribute_values(ast, "compression");
  if cfg!(feature = "compression") && (compression.len() > 1 || !compression.first().is_none_or(|v| COMPRESSION_ALGOS.contains(&v.as_str()))) {
    return Err(syn::Error::new_spanned(
      ast,
      format!(
        "#[derive(RustEmbed)] must contain one attribute like this #[compression = \"deflate\"]\navailable compression algorithm are {COMPRESSION_ALGOS:?}"
      ),
    ));
  }
  let compression = compression.pop().unwrap_or_else(|| "deflate".to_owned());

  let prefix = find_attribute_values(ast, "prefix").into_iter().next();
  let includes = find_attribute_values(ast, "include");
  let excludes = find_attribute_values(ast, "exclude");
  let metadata_only = find_bool_attribute(ast, "metadata_only").unwrap_or(false);
  let allow_missing = find_bool_attribute(ast, "allow_missing").unwrap_or(false);

  #[cfg(not(feature = "include-exclude"))]
  if !includes.is_empty() || !excludes.is_empty() {
    return Err(syn::Error::new_spanned(
      ast,
      "Please turn on the `include-exclude` feature to use the `include` and `exclude` attributes",
    ));
  }

  #[cfg(feature = "interpolate-folder-path")]
  let folder_path = match shellexpand::full(&folder_path) {
    Ok(v) => Ok(Some(v.to_string())),
    Err(v) if v.cause == std::env::VarError::NotPresent && allow_missing => Ok(None),
    Err(v) => Err(syn::Error::new_spanned(ast, v.to_string())),
  }?;
  #[cfg(not(feature = "interpolate-folder-path"))]
  let folder_path = Some(folder_path);

  // Base relative paths on the Cargo.toml location
  let (relative_path, absolute_folder_path) = if let Some(folder_path) = folder_path {
    let (relative_path, absolute_folder_path) = if Path::new(&folder_path).is_relative() {
      let absolute_path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap())
        .join(&folder_path)
        .to_str()
        .unwrap()
        .to_owned();
      (Some(folder_path.clone()), absolute_path)
    } else {
      if cfg!(feature = "compression") {
        let cargo_manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        match Path::new(&folder_path).strip_prefix(&cargo_manifest_dir) {
          Ok(rel) => {
            let rel_str = rel.to_str().unwrap().to_owned();
            (Some(rel_str), folder_path)
          }
          Err(_) => return Err(syn::Error::new_spanned(ast, "`folder` must be a relative path under `compression` feature.")),
        }
      } else {
        (None, folder_path)
      }
    };

    if !Path::new(&absolute_folder_path).exists() && !allow_missing {
      let mut message = format!(
        "#[derive(RustEmbed)] folder '{}' does not exist. cwd: '{}'",
        absolute_folder_path,
        std::env::current_dir().unwrap().to_str().unwrap()
      );

      // Add a message about the interpolate-folder-path feature if the path may
      // include a variable
      if absolute_folder_path.contains('$') && cfg!(not(feature = "interpolate-folder-path")) {
        message += "\nA variable has been detected. RustEmbed can expand variables \
                    when the `interpolate-folder-path` feature is enabled.";
      }

      return Err(syn::Error::new_spanned(ast, message));
    };
    (relative_path, Some(absolute_folder_path))
  } else {
    (None, None)
  };

  generate_assets(
    &ast.ident,
    relative_path.as_deref(),
    absolute_folder_path,
    prefix,
    includes,
    excludes,
    metadata_only,
    &crate_path,
    &compression,
  )
}

#[proc_macro_derive(RustEmbed, attributes(folder, prefix, include, exclude, allow_missing, metadata_only, crate_path, compression))]
pub fn derive_input_object(input: TokenStream) -> TokenStream {
  let ast = parse_macro_input!(input as DeriveInput);
  match impl_rust_embed(&ast) {
    Ok(ok) => ok.into(),
    Err(e) => e.to_compile_error().into(),
  }
}

/// Expands to a string literal containing the mimetype of the file at the given
/// path, guessed from its extension.
///
/// This lives in the proc-macro crate (which depends on `mime_guess`
/// unconditionally) rather than being computed inside the derive's host-side
/// logic: the derive must emit tokens that do not depend on the *host*'s
/// `mime-guess` feature, and this proc macro is invoked only from the
/// target-side `__rust_embed_metadata!` macro. When the target has `mime-guess`
/// disabled, the invocation generated by the derive is discarded by that macro
/// without ever being expanded.
#[doc(hidden)]
#[proc_macro]
pub fn __mimetype_of(input: TokenStream) -> TokenStream {
  let path = parse_macro_input!(input as syn::LitStr).value();
  let mimetype = mime_guess::from_path(&path).first_or_octet_stream().to_string();
  quote! { #mimetype }.into()
}
