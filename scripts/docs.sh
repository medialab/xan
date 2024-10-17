#!/bin/bash
set -uoe pipefail

cargo build

XAN=./target/debug/xan

# Templating README.md
XAN_MOONBLADE_CHEATSHEET=$($XAN map --cheatsheet | tail -n +2) \
XAN_MOONBLADE_FUNCTIONS=$($XAN map --functions | tail -n +2) \
XAN_MOONBLADE_AGGS=$($XAN agg --aggs | tail -n +2) \
  envsubst < README.template.md > README.md

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