# Dataviz from the comfort of your terminal

This document is a showcase & guide to data visualisation in the terminal using the [`xan`](https://github.com/medialab/xan) command line tool.

<!-- TODO: grid view, automagic using imagemacick convert, or gif? -->
<!-- TODO: how to save -->
<!-- TODO: rainbow -->
<!-- TODO: layout gif -->

## Summary

- [Downloading the datasets used in this guide](#downloading-the-datasets-used-in-this-guide)
- [`xan view` to display tables](#xan-view-to-display-tables)
    - [Fitting the screen](#fitting-the-screen)
    - [Dealing with emojis](#dealing-with-emojis)
- [`xan flatten` for close reading](#xan-flatten-for-close-reading)
- [`xan stats -R/--report` for automatic statistical reports](#xan-stats--r--report-for-automatic-statistical-reports)
- [`xan hist` for detailed bar plots](#xan-hist-for-detailed-bar-plots)
- [`xan plot` for scatter plots, line plots and time series](#xan-plot-for-scatter-plots-line-plots-and-time-series)
- [`xan heatmap` for heatmaps and conditional formatting](#xan-heatmap-for-heatmaps-and-conditional-formatting)
- [`xan spark` for sparklines and aggregated bar plots](#xan-spark-for-sparklines-and-aggregated-bar-plots)
- [`xan progress` for progress bars](#xan-progress-for-progress-bars)


## Downloading the datasets used in this guide

*series.csv*

Time series related from RIAA about music distribution formats in time and their associated gross revenues.

```bash
curl -LO https://github.com/medialab/xan/raw/refs/heads/master/docs/cookbook/resources/series.csv
```

*sotu.csv*

Retranscription of U.S. state of the union speeches across time (1790 to 2018):

```bash
curl -L https://github.com/BrianWeinstein/state-of-the-union/raw/refs/heads/master/transcripts.csv > sotu.csv
```

*pulsar.csv*

Data from the pulsar plot of the following article:

> Radio Observations of the Pulse Profiles and Dispersion Measures of Twelve Pulsars by Harold D. Carft, Jr. 1970

famously used on the cover of Joy Division's "Unknown Pleasures" album.

```bash
curl -LO https://gist.githubusercontent.com/borgar/31c1e476b8e92a11d7e9/raw/0fae97dab6830ecee185a63c1cee0008f6778ff6/pulsar.csv
```

## `xan view` to display tables

[`xan view`](../cmd/view.md) is usually one of the first learned and most used commands of `xan` since it lets you take a glance at your CSV files directly in the terminal, using the very familiar tabular representation. You can thus forego using `LibreOffice` or (god forbids!) `Excel` and never ever have to leave the terminal again!

Here is how to use it:

```bash
xan view series.csv
```

<p align="center">
    <img alt="view.png" src="./img/dataviz/view.png" width="80%" />
</p>

See how different data types are colored differently, like in a code editor, to help you figure things out? `xan view` knows how to recognize numbers, strings, time-related information, urls, null values and booleans.

If you fancy rainbows and are not much of a data type kind of person you can also use the `-R/--rainbow` flag to use alternating color per column instead:

```bash
xan view --rainbow series.csv
```

<p align="center">
    <img alt="view-rainbow.png" src="./img/dataviz/view-rainbow.png" width="80%" />
</p>

### Fitting the screen

In `series.csv`, the data is quite concise, so it is easy to print all columns losslessly in the terminal. But see what happens when we use the command, in a small terminal, on `sotu.csv`, containing urls and full text for whole speeches:

```bash
xan view sotu.csv
```

<p align="center">
    <img alt="view-sotu.png" src="./img/dataviz/view-sotu.png" width="60%" />
</p>

First, see how some values get truncated to fit on screen?

Then the command tells you we could only display 3 out of 5 columns, which is why there is a dummy column in the middle full of ellipsis `…` characters, lest we forget it. When space is tight, the `view` command will always try to print a mix of columns from the beginning and from the end.

Then, see how the first cell of the `transcript` column contains a highlighted leading newline character? The `view` command will highlight a lot of those patterns to easily spot irregularities about your data, such as empty cells (displayed as a greyed out `<empty>`), leading/trailing whitespace etc.

Finally, see how last row is also a dummy one full of ellipsis `…` characters? That's because `xan view`, like most `xan` commands, follow a streaming approach and only displays the first rows of your data by default (my screenshots use 10, but the command's default is 100).

The command works thusly because you usually don't need to consume all rows of a file to be able to preview it efficiently and because, as a human, you won't be able to read more than some hundreds of rows by yourself anyway ;).

What's more `xan view` is usually the last step of a complex `xan pipeline` yielding a stream you should not need to consume entirely to make sure it spits out the required data, which is the reason why you used `xan view` in the first place instead of piping the result to a file.

*Printing more rows*

If you want more or less rows on screen, you can always use the `-l/--limit` flag. Or you can also use the `-A/--all` flag to print everything if you feel like you can take it.

*Printing more columns*

However this only takes care of the rows not being printed, not the columns. For this particular problem, people usually rely on pagers such as `less` or `more`:

```bash
xan view --expand --color=always file.csv | less -SR
```

But the above command is quite a mouthful and (if you are not on legacy Windows shell) you can also use the `-p/--pager` flag that will do the same:

```bash
xan view -p file.csv
```

### Dealing with emojis

Funnily enough, there is no way to predict, even when using a monospace font (which is customary in a terminal), the width an emoji will take on screen once rendered.

This is unfortunate because terminal rendering is character-based and layout computations work by knowing what width a character will have on screen (yes some characters can span 2 columns or sometimes do not appear on screen at all).

So if you spot this kind of artifacts when using `xan view`:

<p align="center">
    <img alt="view-emojis.png" src="./img/dataviz/view-emojis.png" width="40%" />
</p>

Just use the `-E/--sanitize-emojis` flag to print their shortcodes instead:

```bash
xan view -E data-with-emojis.csv
```

<p align="center">
    <img alt="view-emojis-sanitized.png" src="./img/dataviz/view-emojis-sanitized.png" width="40%" />
</p>

<!--

groupby

passthrough

flatten -->

## `xan flatten` for close reading

## `xan stats -R/--report` for automatic statistical reports

<!-- TODO: highlighting, -F -->

## `xan hist` for detailed bar plots

<!-- TODO: freq, -c, bins -->

## `xan plot` for scatter plots, line plots and time series

## `xan heatmap` for heatmaps and conditional formatting

## `xan spark` for sparklines and aggregated bar plots

<!-- TODO: joydiv -->

## `xan progress` for progress bars

<!-- TODO: asciinema gif -->
