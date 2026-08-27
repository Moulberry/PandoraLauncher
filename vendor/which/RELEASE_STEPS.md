# Release steps

This crate is released infrequently so it is easy to forget things that need to be done with a new release.
Here's a list.

1. Is the crate formatted correctly?
2. Do the unit tests pass on all platforms? (CI runs to check this are fine)
    - Windows
    - Linux
    - macOS
    - WebAssembly targets
3. Is clippy linter reporting any issues?
4. Does the crate build with an MSRV compiler on all platforms?
5. Bump the version number
6. Make sure CHANGELOG.md is up to date
7. Tag the latest commit to master with the version number, format x.y.z
8. Generate a release on GitHub
9. Make sure the crate doesn't have Windows-style newlines when publishing, Linux distro maintainers seem to care about this.
10. cargo publish 

