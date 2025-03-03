# Contributing to `xan`

## How to release

1. Bump the version in `Cargo.toml`
2. `cargo publish`
3. Publish a release on github with a tag aligned with the version in `Cargo.toml` and a release name prefixed with `v`.
4. Drop `(provisional)` in `CHANGELOG.md`.
