#!/usr/bin/bash

# Generating the data from the "raw" sample:
#   $ xan select -e 'Category as category, Format as format, col("year-date") as date, Units as units, col(-2) as revenues, col(-1) as adjusted_revenues' series.csv

# Installing correct version of `ansi2png-rs`:
#   $ cargo +nightly install --git https://github.com/yomguithereal/ansi2png-rs --locked --rev 71ae8a92

export CLICOLOR_FORCE=1

RESOURCES_DIR="$(dirname $0)/../../resources"
IMG_DIR="$(dirname $0)"
SERIES="$RESOURCES_DIR/series.csv"

# view
echo "xan view snapshots"

xan v "$SERIES" -l 10 --name series.csv --repeat-headers never | \
ansi2png-rs -o "$IMG_DIR/view.png"

# stats -R
echo "xan stats -R snapshots"

xan stats -R "$SERIES" | ansi2png-rs -o "$IMG_DIR/stats-report.png"
