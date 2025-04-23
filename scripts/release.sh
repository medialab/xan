#!/bin/bash
set -uoe pipefail

VERSION=$(cargo run -- --version)

cargo publish
git tag "$VERSION"
git push --tags