#!/usr/bin/bash
IMG_DIR="$(dirname $0)"

FILES=(
    "$IMG_DIR/view.png"
    "$IMG_DIR/heatmap-custom-decades.png"
    "$IMG_DIR/view-grouped.png"
    "$IMG_DIR/plot-scatter-small-multiples-horizontal.png"
    "$IMG_DIR/flatten.png"
    "$IMG_DIR/hist-categorical1.png"
    "$IMG_DIR/stats-report.png"
    "$IMG_DIR/plot-layout-gradient-unzoomed.png"
    "$IMG_DIR/heatmap-conditional-formatting.png"
    "$IMG_DIR/plot-scatter-categorical.png"
    "$IMG_DIR/hist-freq-multiple.png"
    "$IMG_DIR/heatmap-adj.png"
    "$IMG_DIR/plot-time-small-multiples.png"
    "$IMG_DIR/flatten-sotu-highlight.png"
    "$IMG_DIR/plot-layout-clusters-colors.png"
    "$IMG_DIR/heatmap-corr-renamed.png"
)

# NOTE: current is 2800x2240
# read MAX_W MAX_H < <(
#     identify -format "%w %h\n" "${FILES[@]}" | awk '
#     {
#         if ($1 > w) w = $1
#         if ($2 > h) h = $2
#     }
#     END {
#         print w, h
#     }'
# )

# echo "Max canvas: ${MAX_W}x${MAX_H}"

MAX_W=2800
MAX_H=2240

rm -rf /tmp/xan-gif
mkdir -p /tmp/xan-gif

convert "${FILES[@]}" \
    -resize "${MAX_W}x${MAX_H}" \
    -background '#171421' \
    -gravity Center \
    -extent "${MAX_W}x${MAX_H}" \
    -set delay 80 \
    -loop 0 \
    /tmp/xan-gif/dataviz.png

gifski -r 0.8 /tmp/xan-gif/dataviz-*.png -o "$IMG_DIR/dataviz.gif"

rm -rf /tmp/xan-gif
