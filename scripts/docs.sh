#!/bin/bash
set -uoe pipefail

cargo build

XAN=./target/debug/xan

# Stubbing per-command help
for cmd in $($XAN 2>&1 | grep -Eo "\s{4}[a-z-]+\s" | sed 's/ //g')
do
  path=docs/cmd/$cmd.md

  if [ ! -f $path ] || grep -qF '<!-- Generated -->' $path; then
    cat << EOF > $path
<!-- Generated -->
# xan $cmd

\`\`\`txt
$($XAN $cmd --help 2>&1)
\`\`\`
EOF
  fi
done

# Moonblade reference
    cat << EOF > docs/moonblade.md
# xan expression language reference

* [Cheatsheet](#cheatsheet)
* [Functions & Operators](#functions--operators)
* [Aggregation functions](#aggregation-functions)

## Cheatsheet

\`\`\`txt
$($XAN map --cheatsheet | tail -n +2)
\`\`\`

## Functions & Operators

\`\`\`txt
$($XAN map --functions | tail -n +2)
\`\`\`

## Aggregation functions

\`\`\`txt
$($XAN agg --aggs | tail -n +2)
\`\`\`

EOF
