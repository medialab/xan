#!/bin/bash
set -uoe pipefail
shopt -s lastpipe

cargo build --release

XAN=./target/release/xan

echo $($XAN count $1)

for i in $(seq 1 128);
do
  declare -i total=0

  $XAN split --chunks $i --segments $1 | $XAN select -e 'fmt("-B {} -E {}", from, to)' | $XAN behead | \
  while read -r segment;
  do
    count=$($XAN slice $segment $1 | $XAN count)
    ((total += $count))
  done

  echo "--chunks=$i -> $total"
done
