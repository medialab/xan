#!/usr/bin/bash

# Generating the data from the "raw" sample:
#   $ xan select -e 'Category as category, Format as format, col("year-date") as date, Units as units, col(-2) as revenues, col(-1) as adjusted_revenues' series.csv

# Installing correct version of `ansi2png-rs`:
#   $ cargo +nightly install --git https://github.com/yomguithereal/ansi2png-rs --locked --rev 9c3ccd6d

export CLICOLOR_FORCE=1

RESOURCES_DIR="$(dirname $0)/../../resources"
IMG_DIR="$(dirname $0)"
SERIES="$RESOURCES_DIR/series.csv"
SOTU="$RESOURCES_DIR/sotu.csv"

save() {
    ansi2png-rs -o "$IMG_DIR/$1.png"
}

# view
echo "xan view snapshots"

xan v "$SERIES" -l 10 --name series.csv --repeat-headers never | \
save "view"

xan v "$SERIES" -l 10 --name series.csv --repeat-headers never --rainbow | \
save "view-rainbow"

xan v "$SOTU" -l 10 --name sotu.csv --repeat-headers never --cols 50 | \
save "view-sotu"

xan sample 3 -g category --seed 1 "$SERIES" | \
xan v -A --repeat-headers never -g category | \
save "view-grouped"

xan v "$SERIES" -l 10 --name sotu.csv --repeat-headers never -HIMS 5 | \
save "view-custom"

xan v "$SERIES" -l 10 -MI --name series.csv --repeat-headers never --theme borderless | \
save "view-borderless"

xan v "$SERIES" -l 10 -MI --name series.csv --repeat-headers never --theme striped | \
save "view-striped"

# flatten
echo "xan flatten snapshots"

xan f "$SERIES" -l 5 --cols 50 | \
save "flatten"

# stats -R
echo "xan stats -R snapshots"

xan stats -s 0,2,3 "$SERIES" | xan f -N --row-separator " " | \
save "stats-flat"

xan stats -s 0,2,3 -R "$SERIES" | save "stats-report"
