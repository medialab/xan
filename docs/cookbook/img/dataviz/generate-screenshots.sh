#!/usr/bin/bash

# Generating the data from the density design "Raw" sample:
#   $ xan select -e 'Category as category, Format as format, col("year-date") as date, Units as units, col(-2) as revenues, col(-1) as adjusted_revenues' series.csv

# Installing correct version of `ansi2png-rs`:
#   $ cargo +nightly install --git https://github.com/yomguithereal/ansi2png-rs --locked --rev 70dbf53e

export CLICOLOR_FORCE=1

RESOURCES_DIR="$(dirname $0)/../../resources"
IMG_DIR="$(dirname $0)"
SERIES="$RESOURCES_DIR/series.csv"
SOTU="$RESOURCES_DIR/sotu.csv"
MEDIAS="$RESOURCES_DIR/medias.csv"
IRIS="$RESOURCES_DIR/iris.csv"
MISERABLES="$RESOURCES_DIR/les-miserables.csv"
LAYOUT="$RESOURCES_DIR/layout.csv.gz"
CLUSTERS="$RESOURCES_DIR/clusters.csv.gz"

save() {
    ansi2png-rs -o "$IMG_DIR/$1.png"
}

save_with_width() {
    ansi2png-rs -o "$IMG_DIR/$1.png" --png-width "$2"
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

xan f "$SERIES" -R -l 5 --cols 50 | \
save "flatten-rainbow"

xan search -s president Obama "$SOTU" | \
xan tokenize sentences transcript | \
xan f -l 5 --cols 59 | \
save_with_width "flatten-sotu" 1432

xan search -s president Obama "$SOTU" | \
xan tokenize sentences transcript | \
xan f -c -l 5 --cols 59 | \
save_with_width "flatten-sotu-condense" 1432

xan search -s president Obama "$SOTU" | \
xan tokenize sentences transcript | \
xan f -w -l 5 --cols 59 | \
save_with_width "flatten-sotu-wrap" 1432

xan search -s president Obama "$SOTU" | \
xan tokenize sentences transcript | \
xan f -F -l 2 --cols 59 | \
save_with_width "flatten-sotu-flatter" 1432

xan tokenize sentences transcript "$SOTU" | \
xan search -s sentence -i conspicuous | \
xan f -F -l 2 --cols 59 -i -H conspicuous | \
save_with_width "flatten-sotu-highlight" 1432

xan tail -l 2 "$MEDIAS" | \
xan f -N --split prefixes | \
save "flatten-split"

# stats -R
echo "xan stats -R snapshots"

xan stats -s 0,2,3 "$SERIES" | xan f -N --row-separator " " | \
save "stats-flat"

xan stats -s 0,2,3 -R "$SERIES" | save "stats-report"

# hist
echo "xan hist snapshots"

xan freq -s category "$SERIES" | \
xan hist --cols 60 | \
save "hist-freq"

xan freq -s category "$SERIES" | \
xan hist --cols 60 -B large | \
save "hist-freq-large"

xan freq -s category "$SERIES" | \
xan hist --cols 60 -R | \
save "hist-freq-rainbow"

xan freq -s category,format "$SERIES" | \
xan hist --cols 60 | \
save "hist-freq-multiple"

xan bins -s revenues "$SERIES" | \
xan hist --cols 60 | \
save "hist-bins"

xan bins -s revenues "$SERIES" | \
xan hist --cols 60 --log | \
save "hist-bins-log"

xan freq -N -g edito -s wheel_category "$MEDIAS" | \
xan sort -s value | \
xan hist --cols 60 -c edito | \
save "hist-categorical1"

xan freq -N -g edito -s wheel_category "$MEDIAS" | \
xan hist --cols 60 -c edito | \
save "hist-categorical2"

xan freq -AN -s foundation_year "$MEDIAS" | \
xan filter 'value > 1980' | \
xan hist -D | \
save "hist-date"

xan freq -AN -s foundation_year "$MEDIAS" | \
xan filter 'value >= 1910 && value <= 1960' | \
xan hist -D -G 2 | \
save "hist-gaps"

xan groupby category 'sum(floor(revenues)) as total' "$SERIES" | \
xan hist --name 'total revenues by category' --label category --value total --cols 60 | \
save "hist-custom"

xan groupby category 'sum(floor(revenues)) as total' "$SERIES" | \
xan sort -s total -N | \
xan hist -R --name 'total revenues by category' --label category --value total --cols 60 | \
save "hist-custom-sorted"

# plot
echo "xan plot snapshots"

xan plot sepal_length petal_width "$IRIS" -M dot | \
save "plot-scatter"

xan plot sepal_length petal_width "$IRIS" -GM dot | \
save "plot-scatter-grid"

xan plot 0 1,2,3 "$IRIS" -M dot | \
save "plot-scatter-ys"

xan plot sepal_length petal_width -c species "$IRIS" -M dot | \
save "plot-scatter-categorical"

xan plot sepal_length petal_width -c species "$IRIS" -M dot -S 1 | \
save "plot-scatter-small-multiples-vertical"

xan plot sepal_length petal_width -c species "$IRIS" -M dot -S 3 -G | \
save "plot-scatter-small-multiples-horizontal"

xan plot sepal_length petal_width -c species "$IRIS" -M dot -S 2 --share-x-scale no --share-y-scale no | \
save "plot-scatter-small-multiples-unshared"

xan plot -LT date --count "$SERIES" | \
save "plot-time"

xan plot -LT date revenues "$SERIES" | \
save "plot-time-y"

xan plot -LT date revenues -c category "$SERIES" | \
save "plot-time-categorical"

xan plot -LT date revenues -c category -S 3 -G "$SERIES" | \
save "plot-time-small-multiples"

xan plot revenues adjusted_revenues -R "$SERIES" | \
save "plot-regression"

xan tokenize words transcript -k word "$SOTU" | \
xan vocab token | \
xan sort -s gf -RN | \
xan enum -c rank -S 1 | \
xan plot rank gf --y-scale log10 --x-scale log10 | \
save "plot-zipf"

xan plot x y -Q --hide-all "$LAYOUT" | \
save "plot-layout"

xan plot x y -D or_rd --density-scale log -Q --hide-all "$LAYOUT" | \
save "plot-layout-gradient"

xan plot x y -D or_rd --density-scale log -Q --hide-all "$LAYOUT" --cols 204 --rows 55 | \
save "plot-layout-gradient-unzoomed"

xan plot x y -Q --hide-all "$CLUSTERS" | \
save "plot-layout-clusters"

xan plot x y -Q --hide-all "$CLUSTERS" -c cluster | \
save "plot-layout-clusters-colors"

# heatmap
echo "xan heatmap snapshots"

xan matrix corr -s :3 "$IRIS" | \
xan heatmap -DU | \
save "heatmap-corr"

xan matrix corr -s :3 "$IRIS" | \
xan heatmap -DUS 3 | \
save "heatmap-corr-size"

xan matrix corr -s :3 "$IRIS" | \
xan heatmap -DUNS 3 | \
save "heatmap-corr-show-numbers"

xan rename -s :3 sl,sw,pl,pw "$IRIS" | \
xan matrix corr -s :3 | \
xan heatmap -DUNS 3 | \
save "heatmap-corr-renamed"

xan matrix count edito wheel_subcategory "$MEDIAS" | \
xan heatmap --gradient viridis -F -S2 -N --normalize col | \
save "heatmap-count"

xan matrix adj source target -U -w weight "$MISERABLES" | \
xan heatmap -F | \
save "heatmap-adj"

xan sample 5 -g species --seed 1 "$IRIS" | \
xan heatmap -l species --normalize col | \
save "heatmap-custom-iris"

xan map 'date.year().round(10) as decade' "$SERIES" | \
xan matrix count decade category | \
xan heatmap -F -G viridis -S3 -N | \
save "heatmap-custom-decades"

xan search -s category Disc "$SERIES" | \
xan slice -l 20 | \
xan heatmap -l date -v revenues,adjusted_revenues -W 17 -N --align right -G yl_gn_bu --normalize col | \
save "heatmap-conditional-formatting"

# progress

# asciinema rec -c 'xan progress sample.csv > /dev/null' progress.cast --overwrite
# agg --rows 1 progress.cast progress.gif

# asciinema rec -c 'xan progress sample.csv --total 10000000 > /dev/null' progress.cast --overwrite
# agg --rows 1 progress.cast progress-total.gif

# asciinema rec -c 'xan progress sample.csv --title "Processing tweets" > /dev/null' progress.cast --overwrite
# agg --rows 1 progress.cast progress-title.gif

# asciinema rec -c 'xan progress sample.csv -B > /dev/null' progress.cast --overwrite
# agg --rows 1 progress.cast progress-bytes.gif

# asciinema rec -c 'xan p count mathilde/**/ocr.csv.gz -P "slice -l 1000000" --progress > /dev/null' progress.cast --overwrite
# agg progress.cast progress-parallel.gif

# misc
echo "misc snapshots"

COLORTERM="" xan plot x y -Q --hide-all -D or_rd "$LAYOUT" --density-scale log | \
save "layout-bad-colors"

