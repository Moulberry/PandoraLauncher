# Rust I18n

[![CI](https://github.com/longbridge/rust-i18n/actions/workflows/ci.yml/badge.svg)](https://github.com/longbridge/rust-i18n/actions/workflows/ci.yml) [![Docs](https://docs.rs/rust-i18n/badge.svg)](https://docs.rs/rust-i18n/) [![Crates.io](https://img.shields.io/crates/v/rust-i18n.svg)](https://crates.io/crates/rust-i18n)

> 🎯 Let's make I18n things to easy!

Rust I18n is a crate for loading localized text from a set of (YAML, JSON or TOML) mapping files. The mappings are converted into data readable by Rust programs at compile time, and then localized text can be loaded by simply calling the provided [`t!`] macro.

Unlike other I18n libraries, Rust I18n's goal is to provide a simple and easy-to-use API.

The API of this crate is inspired by [ruby-i18n](https://github.com/ruby-i18n/i18n) and [Rails I18n](https://guides.rubyonrails.org/i18n.html).

## Features

- Codegen on compile time for includes translations into binary.
- Global [`t!`] macro for loading localized text in everywhere.
- Use YAML (default), JSON or TOML format for mapping localized text, and support mutiple files merging.
- `cargo i18n` Command line tool for checking and extract untranslated texts into YAML files.
- Support all localized texts in one file, or split into difference files by locale.
- Supports specifying a chain of fallback locales for missing translations.
- Supports automatic lookup of language territory for fallback locale. For instance, if `zh-CN` is not available, it will fallback to `zh`. (Since v2.4.0)
- Support short hashed keys for optimize memory usage and lookup speed. (Since v3.1.0)
- Support format variables in [`t!`], and support format variables with [`std::fmt`](https://doc.rust-lang.org/std/fmt/) syntax. (Since v3.1.0)
- Support for log missing translations at the warning level with `log-miss-tr` feature, the feature requires the `log` crate. (Since v3.1.0)

## Usage

Add crate dependencies in your Cargo.toml and setup I18n config:

```toml
[dependencies]
rust-i18n = "3"
```

Load macro and init translations in `lib.rs` or `main.rs`:

```rust,compile_fail,no_run
// Load I18n macro, for allow you use `t!` macro in anywhere.
#[macro_use]
extern crate rust_i18n;

// Init translations for current crate.
// This will load Configuration using the `[package.metadata.i18n]` section in `Cargo.toml` if exists.
// Or you can pass arguments by `i18n!` to override it.
i18n!("locales");

// Config fallback missing translations to "en" locale.
// Use `fallback` option to set fallback locale.
//
i18n!("locales", fallback = "en");

// Or more than one fallback with priority.
//
i18n!("locales", fallback = ["en", "es"]);

// Use a short hashed key as an identifier for long string literals
// to optimize memory usage and lookup speed.
// The key generation algorithm is `${Prefix}${Base62(SipHash13("msg"))}`.
i18n!("locales", minify_key = true);
//
// Alternatively, you can customize the key length, prefix,
// and threshold for the short hashed key.
i18n!("locales",
      minify_key = true,
      minify_key_len = 12,
      minify_key_prefix = "t_",
      minify_key_thresh = 64
);
// Now, if the message length exceeds 64, the `t!` macro will automatically generate
// a 12-byte short hashed key with a "t_" prefix for it, if not, it will use the original.

// If no any argument, use config from Cargo.toml or default.
i18n!();
```

Or you can import by use directly:

```rust,no_run
// You must import in each files when you wants use `t!` macro.
use rust_i18n::t;

rust_i18n::i18n!("locales");

fn main() {
    // Find the translation for the string literal `Hello` using the manually provided key `hello`.
    println!("{}", t!("hello"));

    // Use `available_locales!` method to get all available locales.
    println!("{:?}", rust_i18n::available_locales!());
}
```

## Locale file

You can use `_version` key to specify the version (This version is the locale file version, not the rust-i18n version) of the locale file, and the default value is `1`.

rust-i18n supports two style of config file, and those versions will always be keeping.

- `_version: 1` - Split each locale into difference files, it is useful when your project wants to split to translate work.
- `_version: 2` - Put all localized text into same file, it is easy to translate quickly by AI (e.g.: GitHub Copilot). When you write original text, just press Enter key, then AI will suggest you the translation text for other languages.

You can choose as you like.

### Split Localized Texts into Difference Files

> \_version: 1

You can also split the each language into difference files, and you can choose (YAML, JSON, TOML), for example: `en.json`:

```bash
.
├── Cargo.lock
├── Cargo.toml
├── locales
│   ├── zh-CN.yml
│   ├── en.yml
└── src
│   └── main.rs
```

```yml
_version: 1
hello: "Hello world"
messages.hello: "Hello, %{name}"
t_4Cct6Q289b12SkvF47dXIx: "Hello, %{name}"
```

Or use JSON or TOML format, just rename the file to `en.json` or `en.toml`, and the content is like this:

```json
{
  "_version": 1,
  "hello": "Hello world",
  "messages.hello": "Hello, %{name}",
  "t_4Cct6Q289b12SkvF47dXIx": "Hello, %{name}"
}
```

```toml
hello = "Hello world"
t_4Cct6Q289b12SkvF47dXIx = "Hello, %{name}"

[messages]
hello = "Hello, %{name}"
```

### All Localized Texts in One File

> \_version: 2

Make sure all localized files (containing the localized mappings) are located in the `locales/` folder of the project root directory:

```bash
.
├── Cargo.lock
├── Cargo.toml
├── locales
│   ├── app.yml
│   ├── some-module.yml
└── src
│   └── main.rs
└── sub_app
│   └── locales
│   │   └── app.yml
│   └── src
│   │   └── main.rs
│   └── Cargo.toml
```

In the localized files, specify the localization keys and their corresponding values, for example, in `app.yml`:

```yml
_version: 2
hello:
  en: Hello world
  zh-CN: 你好世界
messages.hello:
  en: Hello, %{name}
  zh-CN: 你好，%{name}
# Generate short hashed keys using `minify_key=true, minify_key_thresh=10`
t_4Cct6Q289b12SkvF47dXIx:
  en: Hello, %{name}
  zh-CN: 你好，%{name}
```

This is useful when you use [GitHub Copilot](https://github.com/features/copilot), after you write a first translated text, then Copilot will auto generate other locale's translations for you.

<img src="https://user-images.githubusercontent.com/5518/262332592-7b6cf058-7ef4-4ec7-8dea-0aa3619ce6eb.gif" width="446" />

### Get Localized Strings in Rust

Import the [`t!`] macro from this crate into your current scope:

```rust,no_run
use rust_i18n::t;
```

Then, simply use it wherever a localized string is needed:

```rust,no_run
# macro_rules! t {
#    ($($all_tokens:tt)*) => {}
# }
# fn main() {
// use rust_i18n::t;
t!("hello");
// => "Hello world"

t!("hello", locale = "zh-CN");
// => "你好世界"

t!("messages.hello", name = "world");
// => "Hello, world"

t!("messages.hello", "name" => "world");
// => "Hello, world"

t!("messages.hello", locale = "zh-CN", name = "Jason", count = 2);
// => "你好，Jason (2)"

t!("messages.hello", locale = "zh-CN", "name" => "Jason", "count" => 3 + 2);
// => "你好，Jason (5)"

t!("Hello, %{name}, you serial number is: %{sn}", name = "Jason", sn = 123 : {:08});
// => "Hello, Jason, you serial number is: 000000123"
# }
```

### Current Locale

You can use [`rust_i18n::set_locale()`](<set_locale()>) to set the global locale at runtime, so that you don't have to specify the locale on each [`t!`] invocation.

```rust
rust_i18n::set_locale("zh-CN");

let locale = rust_i18n::locale();
assert_eq!(&*locale, "zh-CN");
```

### Extend Backend

Since v2.0.0 rust-i18n support extend backend for cusomize your translation implementation.

For example, you can use HTTP API for load translations from remote server:

```rust,no_run
# pub mod reqwest {
#  pub mod blocking {
#    pub struct Response;
#    impl Response {
#       pub fn text(&self) -> Result<String, Box<dyn std::error::Error>> { todo!() }
#    }
#    pub fn get(_url: &str) -> Result<Response, Box<dyn std::error::Error>> { todo!() }
#  }
# }
# use std::collections::HashMap;
use rust_i18n::Backend;

pub struct RemoteI18n {
    trs: HashMap<String, HashMap<String, String>>,
}

impl RemoteI18n {
    fn new() -> Self {
        // fetch translations from remote URL
        let response = reqwest::blocking::get("https://your-host.com/assets/locales.yml").unwrap();
        let trs = serde_yaml::from_str::<HashMap<String, HashMap<String, String>>>(&response.text().unwrap()).unwrap();

        return Self {
            trs
        };
    }
}

impl Backend for RemoteI18n {
    fn available_locales(&self) -> Vec<&str> {
        return self.trs.keys().map(|k| k.as_str()).collect();
    }

    fn translate(&self, locale: &str, key: &str) -> Option<&str> {
        // Write your own lookup logic here.
        // For example load from database
        return self.trs.get(locale)?.get(key).map(|k| k.as_str());
    }
}
```

Now you can init rust_i18n by extend your own backend:

```rust,no_run
# struct RemoteI18n;
# impl RemoteI18n {
#   fn new() -> Self { todo!() }
# }
# impl rust_i18n::Backend for RemoteI18n {
#   fn available_locales(&self) -> Vec<&str> { todo!() }
#   fn translate(&self, locale: &str, key: &str) -> Option<&str> { todo!() }
# }
rust_i18n::i18n!("locales", backend = RemoteI18n::new());
```

This also will load local translates from ./locales path, but your own `RemoteI18n` will priority than it.

Now you call [`t!`] will lookup translates from your own backend first, if not found, will lookup from local files.

## Example

A minimal example of using rust-i18n can be found [here](https://github.com/longbridge/rust-i18n/tree/main/examples).

## I18n Ally

I18n Ally is a VS Code extension for helping you translate your Rust project.

You can add [i18n-ally-custom-framework.yml](https://github.com/longbridge/rust-i18n/blob/main/.vscode/i18n-ally-custom-framework.yml) to your project `.vscode` directory, and then use I18n Ally can parse `t!` marco to show translate text in VS Code editor.

## Extractor

> **Experimental**

We provided a `cargo i18n` command line tool for help you extract the untranslated texts from the source code and then write into YAML file.

> In current only output YAML, and use `_version: 2` format.

You can install it via `cargo install rust-i18n-cli`, then you get `cargo i18n` command.

```bash
$ cargo install rust-i18n-cli
```

### Extractor Config

💡 NOTE: `package.metadata.i18n` config section in Cargo.toml is just work for `cargo i18n` command, if you don't use that, you don't need this config.

```toml
[package.metadata.i18n]
# The available locales for your application, default: ["en"].
# available-locales = ["en", "zh-CN"]

# The default locale, default: "en".
# default-locale = "en"

# Path for your translations YAML file, default: "locales".
# This config for let `cargo i18n` command line tool know where to find your translations.
# You must keep this path same as the one you pass to method `rust_i18n::i18n!`.
# load-path = "locales"
```

Rust I18n providered a `i18n` bin for help you extract the untranslated texts from the source code and then write into YAML file.

```bash
$ cargo install rust-i18n-cli
# Now you have `cargo i18n` command
```

After that the untranslated texts will be extracted and saved into `locales/TODO.en.yml` file.

You also can special the locale by use `--locale` option:

```bash
$ cd your_project_root_directory
$ cargo i18n

Checking [en] and generating untranslated texts...
Found 1 new texts need to translate.
----------------------------------------
Writing to TODO.en.yml

Checking [fr] and generating untranslated texts...
Found 11 new texts need to translate.
----------------------------------------
Writing to TODO.fr.yml

Checking [zh-CN] and generating untranslated texts...
All thing done.

Checking [zh-HK] and generating untranslated texts...
Found 11 new texts need to translate.
----------------------------------------
Writing to TODO.zh-HK.yml
```

Run `cargo i18n -h` to see details.

```bash
$ cargo i18n -h
cargo-i18n 3.1.0
---------------------------------------
Rust I18n command to help you extract all untranslated texts from source code.

It will iterate all Rust files in the source directory and extract all untranslated texts that used `t!` macro. Then it will generate a YAML file and merge with the existing translations.

https://github.com/longbridge/rust-i18n

Usage: cargo i18n [OPTIONS] [-- <SOURCE>]

Arguments:
  [SOURCE]
          Extract all untranslated I18n texts from source code

          [default: ./]

Options:
  -t, --translate <TEXT>...
          Manually add a translation to the localization file.

          This is useful for non-literal values in the `t!` macro.

          For example, if you have `t!(format!("Hello, {}!", "world"))` in your code,
          you can add a translation for it using `-t "Hello, world!"`,
          or provide a translated message using `-t "Hello, world! => Hola, world!"`.

          NOTE: The whitespace before and after the key and value will be trimmed.

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## Debugging the Codegen Process

The `RUST_I18N_DEBUG` environment variable can be used to print out some debugging infos when code is being generated at compile time.

```bash
$ RUST_I18N_DEBUG=1 cargo build
```

## Benchmark

Benchmark [`t!`] method, result on MacBook Pro (2023, Apple M3):

```bash
t                       time:   [32.637 ns 33.139 ns 33.613 ns]
t_with_locale           time:   [24.616 ns 24.812 ns 25.071 ns]
t_with_args             time:   [128.70 ns 128.97 ns 129.24 ns]
t_with_args (str)       time:   [129.48 ns 130.08 ns 130.76 ns]
t_with_args (many)      time:   [370.28 ns 374.46 ns 380.56 ns]
t_with_threads          time:   [38.619 ns 39.506 ns 40.419 ns]
t_lorem_ipsum           time:   [33.867 ns 34.286 ns 34.751 ns]
```

The result `101 ns (0.0001 ms)` means if there have **10K** translate texts, it will cost `1ms`.

## Use Cases

- [longbridge-terminal](https://github.com/longbridge/longbridge-terminal)
- [topgrade](https://github.com/topgrade-rs/topgrade)
- [trippy](https://github.com/fujiapple852/trippy)
- [hyperswitch](https://github.com/juspay/hyperswitch)
- [MirrorX](https://github.com/MirrorX-Desktop/MirrorX)

## License

MIT
