## Run I18n extract in dev

When we want test `cargo i18n` in local dev, we can:

```bash
$ cargo run -- i18n ~/work/some-rust-project
```

## How to release

1. Update `Cargo.toml` version
2. Run `make release` to build and publish to crates.io
