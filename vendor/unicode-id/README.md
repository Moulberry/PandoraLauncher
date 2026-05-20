> [!IMPORTANT]
> Try the optimized version [unicode-id-start](https://github.com/Boshen/unicode-id-start).

# unicode-id

Determine if a `char` is a valid identifier for a parser and/or lexer according to
[Unicode Standard Annex #31](http://www.unicode.org/reports/tr31/) rules.

This is a clone of [unicode-xid](https://github.com/unicode-rs/unicode-xid).

```rust
use unicode_id::UnicodeID;

fn main() {
    let ch = 'a';
    println!("Is {} a valid start of an identifier? {}", ch, UnicodeID::is_id_start(ch));
}
```

## features

unicode-id supports a `no_std` feature. This eliminates dependence
on std, and instead uses equivalent functions from core.

## changelog

### 0.3.6

- Update to Unicode 17.0.0

### 0.3.5

- Update to Unicode 16.0.0

### 0.3.4

- Update to Unicode 15.1.0

### 0.3.3

- Update to Unicode 15.0.0

### 0.3.2

- Fix clippy warnings

### 0.3.0

- Fork repo for unicode-id
- Update to Unicode 14.0.0

### 0.2.2

- Add an ASCII fast-path

### 0.2.1

- Update to Unicode 13.0.0
- Speed up lookup

### 0.2.0

- Update to Unicode 12.1.0.

### 0.1.0

- Initial release.
