#!/usr/bin/bash
IMG_DIR="$(dirname $0)"

FILES=(
    "$IMG_DIR/view.png"
    "$IMG_DIR/stats-report.png"
)

convert -delay 80 -loop 0 ${FILES[@]} "$IMG_DIR/dataviz.gif"
