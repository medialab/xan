#!/bin/bash
set -uoe pipefail

cargo build

XAN=./target/debug/xan

# Stubbing per-command help
for cmd in $($XAN 2>&1 | grep -Eo "\s{4}[a-z][a-z-]+\s" | sed 's/ //g')
do
  path=docs/cmd/$cmd.md
  echo $path

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
mkdir -p docs/moonblade
$XAN help cheatsheet --md > docs/moonblade/cheatsheet.md
$XAN help functions --md > docs/moonblade/functions.md
$XAN help aggs --md > docs/moonblade/aggs.md
