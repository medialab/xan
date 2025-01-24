#!/bin/bash
set -uoe pipefail

git tag "$1"
git push --tags origin master

# TODO: cargo publish