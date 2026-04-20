# Contributing to `xan`

## May I contribute to xan?

Please open an issue describing your planned contribution first, so that we can discuss the relevance of your proposal and the details of the implementation.

## Modifying a xan command

1. Add your tests to `./tests/test_<command>.rs`
2. Add your changes to `./src/cmd/<command>.rs` and edit the [docopt](http://docopt.org/) help description
3. Use `cargo fmt`, `cargo test <command>` and `cargo clippy`
4. Run `./scripts/docs.sh` to generate the documentation
5. Make a pull request

## Adding a moonblade function

1. Add your tests at the end of `./src/moonblade/interpreter.rs`
2. Add your function somewhere in `./src/moonblade/functions/`
3. Edit documentation in `./src/moonblade/doc/`
4. Use `cargo fmt`, `cargo test` and `cargo clippy`
5. Run `./scripts/docs.sh` to generate the documentation
6. Make a pull request

## How to release

1. Bump the version in `Cargo.toml`.
2. Drop `(provisional)` in `CHANGELOG.md`.
3. Commit `Bump <version>`.
4. Run `./scripts/release.sh`.
