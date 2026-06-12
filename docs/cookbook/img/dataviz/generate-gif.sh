#!/usr/bin/bash
IMG_DIR="$(dirname $0)"

FILES=(
    "$IMG_DIR/view.png"
    "$IMG_DIR/view-grouped.png"
    "$IMG_DIR/flatten.png"
    "$IMG_DIR/stats-report.png"
)

read MAX_W MAX_H < <(
  identify -format "%w %h\n" "${FILES[@]}" | awk '
  {
    if ($1 > w) w = $1
    if ($2 > h) h = $2
  }
  END {
    print w, h
  }'
)

echo "Max canvas: ${MAX_W}x${MAX_H}"

rm -rf /tmp/xan-gif
mkdir -p /tmp/xan-gif

convert "${FILES[@]}" \
  -resize "${MAX_W}x${MAX_H}" \
  -background '#171421' \
  -gravity North \
  -extent "${MAX_W}x${MAX_H}" \
  -set delay 80 \
  -loop 0 \
  /tmp/xan-gif/dataviz.png

gifski -r 0.8 /tmp/xan-gif/dataviz-*.png -o "$IMG_DIR/dataviz.gif"

rm -rf /tmp/xan-gif
