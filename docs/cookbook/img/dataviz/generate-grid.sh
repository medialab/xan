#!/usr/bin/bash
IMG_DIR="$(dirname $0)"

FILES=(
    "$IMG_DIR/view-grouped.png"
    "$IMG_DIR/flatten-split.png"
    "$IMG_DIR/hist-categorical1.png"
    "$IMG_DIR/stats-report.png"
    "$IMG_DIR/plot-scatter-categorical.png"
    "$IMG_DIR/plot-time-small-multiples.png"
    "$IMG_DIR/heatmap-custom-decades.png"
    "$IMG_DIR/heatmap-conditional-formatting.png"
    "$IMG_DIR/spark-gradient.png"
)

MAX_W=2800
MAX_H=2240

mkdir -p "$IMG_DIR/grid"
rm -rf "$IMG_DIR/grid/"*

for file in ${FILES[@]};
do
    echo $file
    convert $file \
        -resize "${MAX_W}x${MAX_H}" \
        -background '#171421' \
        -gravity Center \
        -extent "${MAX_W}x${MAX_H}" \
        "$IMG_DIR/grid/$(basename $file)"
done
