#!/bin/bash
set -uoe pipefail

cargo build

XAN=./target/debug/xan

# Templating README.md
XAN_MOONBLADE_CHEATSHEET=$($XAN map --cheatsheet | tail -n +2) \
XAN_MOONBLADE_FUNCTIONS=$($XAN map --functions | tail -n +2) \
XAN_MOONBLADE_AGGS=$($XAN agg --aggs | tail -n +2) \
  envsubst < README.template.md > README.md