## Run I18n extract in dev

When we want test `cargo i18n` in local dev, we can:

```bash
$ cargo run -- i18n ~/work/some-rust-project
```

## How to release

1. Use `cargo set-version` to set the new version for all crates.

   ```bash
   cargo set-version x.y.z
   ```

2. Git add and commit the changes with message `Bump vx.y.z`.
3. Create a new git tag with the version `vx.y.z` and push `main` branch and the tag to remote.
4. Then GitHub Actions will automatically publish the crates to crates.io and create a new release in GitHub.
