<h1 align="center">formatx</h1>

<p align="center">
  <a href="https://crates.io/crates/formatx">
    <img src="https://img.shields.io/crates/d/formatx?style=flat-square">
  </a>
  <a href="https://crates.io/crates/formatx">
    <img src="https://img.shields.io/crates/v/formatx?style=flat-square">
  </a>
  <a href="https://github.com/clitic/formatx">
    <img src="https://img.shields.io/github/actions/workflow/status/clitic/formatx/ci.yml?logo=github&style=flat-square">
  </a>
  <a href="https://docs.rs/formatx/latest/formatx">
    <img src="https://img.shields.io/docsrs/formatx?logo=docsdotrs&style=flat-square">
  </a>
  <a href="https://github.com/clitic/formatx#license">
    <img src="https://img.shields.io/crates/l/formatx?style=flat-square">
  </a>
  <a href="https://github.com/clitic/formatx">
    <img src="https://img.shields.io/github/repo-size/clitic/formatx?style=flat-square">
  </a>
</p>

<p align="center">A macro for formatting non literal strings at runtime in Rust.</p>

`formatx` is a dependency free string templating library with syntax derived from [std::fmt](https://doc.rust-lang.org/std/fmt/#syntax). formatx exports [formatx!](https://docs.rs/formatx/latest/formatx/macro.formatx.html) macro which is similar to [format!](https://doc.rust-lang.org/std/macro.format.html) macro. formatx works by first parsing the template string and then it uses `format!` macro internally to replicate it's behaviour. formatx aims for formatting strings and numbers although an generic type can also be formatted like an [struct](https://github.com/clitic/formatx/blob/main/examples/struct.rs).

## Getting Started

Add this to your Cargo.toml file.

```toml
[dependencies]
formatx = "0.2.4"
```

Or add from command line.

```bash
$ cargo add formatx
```

See [docs](https://docs.rs/formatx/latest/formatx) and [examples](https://github.com/clitic/formatx.rs/tree/main/examples) to 
know how to use it.

## Example

SOURCE: [format! with non literal string](https://users.rust-lang.org/t/format-with-non-literal-string/2057)

```rust
use formatx::formatx;

fn message(language: &str, name: &str, number: i32) -> String {
    let s = match language {
        "french" => "Bonjour {}, le nombre est {}",
        "spanish" => "Hola {}, el numero es {}",
        _ => "Hi {}, the number is {}",
    };
    formatx!(s, name, number).unwrap()
}

fn main() {
    println!("{}", message("french", "Léa", 1));
    println!("{}", message("spanish", "Sofia", 2));
    println!("{}", message("english", "Ashley", 3));
}
```

OUTPUT

```
Bonjour Léa, le nombre est 1
Hola Sofia, el numero es 2
Hi Ashley, the number is 3
```

## Syntax

The main goal of `formatx` is to provide a almost identical syntax as of rust's format syntax. The syntax of `formatx` is identical to [std::fmt](https://doc.rust-lang.org/std/fmt/#syntax). You can also see gettext [documentation](https://www.gnu.org/software/gettext/manual/html_node/rust_002dformat.html), the rust's format syntax specifications is mentioned there too.

## Limitations

> **Warning**
> Examples given below will always panic.

1. Only types which implements [Display](https://doc.rust-lang.org/std/fmt/trait.Display.html) + [Debug](https://doc.rust-lang.org/std/fmt/trait.Debug.html) traits are supported. Other [formatting-traits](https://doc.rust-lang.org/std/fmt/#formatting-traits) aren't supported.

2. Local variable interpolation isn't supported.

```rust
let people = "Rustaceans";
formatx!("Hello {people}!").unwrap();
```

3. Intermingling the two types of [positional](https://doc.rust-lang.org/std/fmt/#positional-parameters) specifiers isn't supported.

```rust
formatx!("{1} {} {0} {}", 1, 2).unwrap();
```

4. Parameter setting through `$` sign argument isn't supported.

```rust
formatx!("{:width$}!", "x", width = 5).unwrap();
```

5. An asterisk `.*` can't be used to set [precision](https://doc.rust-lang.org/std/fmt/#precision).

```rust
formatx!("{:.*}", 5, 0.01).unwrap();
```

## Handling Of Positional Arguments

Positional arguments are handled by an internal key which increments itself whenever an postional argument is passed through the macro. So, the behaviour is very different when compared to `format` macro.

By default this internal key is set to `0` and when an positional argument is passed. `formatx` resolves the replacement by first checking if there is `0` key present in format string (eg. `Hello {0}`) or not, if the `0` is present then it replaces it, else if the `0` key is not present in the format string (eg.`Hello {}`) it replaces the first blank placeholder. After a replacement is made this internal key is increased by `1`.

```rust
asserteq!(
  format!("{} {2} {} {1} {4} {} {3} {5} {6}", "zero", "one", "two", "three", "four", "five", "six"),
  formatx!("{} {2} {} {1} {4} {} {3} {5} {6}", "zero", "one", "two", "three", "four", "five", "six", "seven", "eight").unwrap() // This line won't panic
); // This line will panic
```

An error cannot be raised in such cases because `formatx` works at runtime and it doesn't know the future upcoming postitional arguments. So while designing your app keep this point in mind or simply use key value arguments. See issue [#7](https://github.com/clitic/formatx/issues/7) for more info.

## Handling Of Unused Arguments

Unlike rust's built-in `format!` macro, which reports an error if any provided arguments are not used in the format string, `formatx!` allows unused arguments. This can be particularly useful in localization scenarios, where translations may or may not require certain arguments depending on grammatical rules.

## Alternatives

1. [strfmt](https://github.com/vitiral/strfmt)
2. [runtime-fmt](https://github.com/SpaceManiac/runtime-fmt)

## License

Dual Licensed

- [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0) ([LICENSE-APACHE](LICENSE-APACHE))
- [MIT license](https://opensource.org/licenses/MIT) ([LICENSE-MIT](LICENSE-MIT))
