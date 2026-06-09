# Dataviz from the comfort of your terminal

This document is a showcase & guide to data visualisation in the terminal using the [`xan`](https://github.com/medialab/xan) command line tool.

It is often overlooked because `xan` is first and foremost a very performant tabular data processing tool, but it can also render a large variety of typical data visualizations directly in your terminal. This ultimately means you never have to leave it to explore the data you mangle.

I say "comfort" and I mean it ;). `xan` will have processed and rendered your data in the terminal long before you are able to spin up your Jupyter instance and import `pandas` & `matplotlib`. No cruft. No distraction. Just raw insights. Like it's still 1970 and all you have is ASCII art, but with ✨colors✨ and Unicode ([braille](https://en.wikipedia.org/wiki/Braille_ASCII) characters are a godsend).

<!-- TODO: grid view, automagic using imagemacick convert, or gif? -->
<!-- TODO: how to save -->
<!-- TODO: layout gif -->
<!-- TODO: mention you can always zoom out -->

## Summary

- [Downloading the datasets used in this guide](#downloading-the-datasets-used-in-this-guide)
- [`xan view` to display tables](#xan-view-to-display-tables)
    - [Fitting the screen](#fitting-the-screen)
    - [Dealing with emojis](#dealing-with-emojis)
    - [Grouping rows](#grouping-rows)
    - [Customizing the view](#customizing-the-view)
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

*iris.csv*

The fampus "Iris" dataset, used in a lot of machine learning examples.

```bash
curl -LO https://github.com/medialab/xan/raw/refs/heads/master/docs/cookbook/resources/iris.csv
```

*pulsar.csv*

Data from the pulsar plot of the following article:

> Radio Observations of the Pulse Profiles and Dispersion Measures of Twelve Pulsars by Harold D. Carft, Jr. 1970

famously used on the cover of Joy Division's "Unknown Pleasures" album.

```bash
curl -LO https://gist.githubusercontent.com/borgar/31c1e476b8e92a11d7e9/raw/0fae97dab6830ecee185a63c1cee0008f6778ff6/pulsar.csv
```

*layout.csv.gz*

x and y positions of a sample of accounts from a French defunct social network, as inferred by the ForceAltas2 layout algorithm.

```bash
curl -LO https://github.com/medialab/xan/raw/refs/heads/master/docs/cookbook/resources/layout.csv.gz
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

### Grouping rows

Sometimes, you might want to group rows visually based on the value of some of their columns. You can do so with `xan v -g/--groupby` thusly:

```bash
xan sample 3 -g category series.csv | \
xan view -A -g category
```

<p align="center">
    <img alt="view-grouped.png" src="./img/dataviz/view-grouped.png" width="80%" />
</p>

### Customizing the view

If you call `xan view --help` you will see that the command offers a lot of customization options (some of which you can set as default through the `XAN_VIEW_ARGS` env variable).

For instance, let's hide headers, the index colum, the info text, and force numbers to be formatted using a maximum of 5 significant numbers:

```bash
xan view -S 5 --hide-index --hide-headers --hide-info series.csv
```

<p align="center">
    <img alt="view-custom.png" src="./img/dataviz/view-custom.png" width="80%" />
</p>

The command even offers a variety of different "themes" that can be used to stylize the table:

```bash
# -M stands for --hide-info & -I for --hide-index
xan view -MI --theme borderless series.csv
```

<p align="center">
    <img alt="view-borderless.png" src="./img/dataviz/view-borderless.png" width="80%" />
</p>

Now, the tabular view is a staple for a reason, but it becomes somewhat limited when your file has many columns or if cell values are very long, for instance if they contain full text.

Fortunately `xan` has another command catering to those use-cases, so you can easily read the full contents of a CSV row: `flatten`.

## `xan flatten` for close reading

`xan flatten` is a command that lets you read full row data more comfortably than `xan view` by "flattening" the representation. That is to say we will let each column take at least a line so the full content of their cells can be read:

```
xan flatten series.csv
```

<p align="center">
    <img alt="flatten.png" src="./img/dataviz/flatten.png" width="40%" />
</p>

<!-- TODO: wrap, condense, flatter, highlighting -->

## `xan stats -R/--report` for automatic statistical reports

`xan` has a `stats` command that can easily compute descriptive statistics about all or a selection of columns of your CSV file.

The result of the command is another CSV file, so people would usually feed to `xan flatten` for better readability:

```bash
# Some columns in the output correspond to numerical vs. text columns
# so people use the -N/--non-empty flag of flatten to hide irrelevant information
xan stats series.csv | xan flatten -N --row-separator " "
```

<p align="center">
    <img alt="stats-flat.png" src="./img/dataviz/stats-flat.png" width="40%" />
</p>

But since this was a prominent use-case and since it would be nice to have inline dataviz such as bar charts, time series and distributions, the command gained a `-R/--report` flag to do just that:

```bash
xan stats -R series.csv
```

<p align="center">
    <img alt="stats-report.png" src="./img/dataviz/stats-report.png" width="80%" />
</p>

## `xan hist` for detailed bar plots

<!-- TODO: freq, -c, bins -->

## `xan plot` for scatter plots, line plots and time series

## `xan heatmap` for heatmaps and conditional formatting

## `xan spark` for sparklines and aggregated bar plots

<!-- TODO: joydiv -->

## `xan progress` for progress bars

<!-- TODO: asciinema gif -->
