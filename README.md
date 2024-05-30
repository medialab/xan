# xan

`xan` is a command line tool that can be used to process CSV files directly from the shell.

It has been written in Rust to be as performant as possible and can easily handle very large CSV files (Gigabytes). It is also able to leverage parallelism (through multithreading) to make some tasks complete as fast as your computer can allow.

It can easily preview, filter, slice, aggregate, join CSV files, and exposes a large collection of composable commands that can be chained together to perform a wide variety of typical tasks.

`xan` also leverages its own expression language so you can perform complex tasks that cannot be done by relying on the simplest commands. This minimalistic language has been tailored for CSV data and is faster than evaluating typical dynamically-typed languages such as Python, Lua, JavaScript etc.

Note that this tool is originally a fork of [BurntSushi](https://github.com/BurntSushi)'s [`xsv`](https://github.com/BurntSushi/xsv), but has been nearly entirely rewritten at that point, to fit [SciencesPo's médialab](https://github.com/medialab) use-cases, rooted in web data collection and analysis geared towards social sciences.

Finally, `xan` can be used to display CSV files in the terminal, for easy exploration, and can even be used to draw basic data visualisations.

*Displaying a CSV file in the terminal using `xan view`*

![view.png](./docs/img/view.png)

*Showing a flattened view of CSV records using `xan flatten`*

![flatten.png](./docs/img/flatten.png)

*Drawing a histogram of values using `xan hist`*

<p align="center">
  <img alt="hist.png" src="./docs/img/hist.png" height="100">
</p>

*Drawing a scatterplot using `xan plot`*

<p align="center">
  <img alt="scatter.png" src="./docs/img/scatter.png" height="400">
</p>

*Drawing a time series using `xan plot`*

<p align="center">
  <img alt="line.png" src="./docs/img/line.png" height="300">
</p>

## Summary

* [How to install](#how-to-install)
* [Quick tour](#quick-tour)
* [Available commands](#available-commands)
* [General flags and IO model](#general-flags-and-io-model)

## How to install

`xan` can be installed using cargo (it usually comes with [Rust](https://www.rust-lang.org/tools/install)):

```
cargo install xan
```

## Quick tour

Let's learn about the most commonly used `xan` commands by exploring a corpus of French medias:

### Downloading the corpus

```bash
curl -LO https://github.com/medialab/corpora/raw/master/polarisation/medias.csv
```
### Displaying the file's headers*

```bash
xan headers medias.csv
```

```txt
0   webentity_id
1   name
2   prefixes
3   home_page
4   start_pages
5   indegree
6   hyphe_creation_timestamp
7   hyphe_last_modification_timestamp
8   outreach
9   foundation_year
10  batch
11  edito
12  parody
13  origin
14  digital_native
15  mediacloud_ids
16  wheel_category
17  wheel_subcategory
18  has_paywall
19  inactive
```

### Counting the number of rows

```bash
xan count medias.csv
```

```txt
478
```

### Previewing the file in the terminal

```bash
xan view medias.csv
```

```txt
Displaying 5/20 cols from 10 first rows of medias.csv
┌───┬───────────────┬───────────────┬────────────┬───┬─────────────┬──────────┐
│ - │ name          │ prefixes      │ home_page  │ … │ has_paywall │ inactive │
├───┼───────────────┼───────────────┼────────────┼───┼─────────────┼──────────┤
│ 0 │ Acrimed.org   │ http://acrim… │ http://ww… │ … │ false       │ <empty>  │
│ 1 │ 24matins.fr   │ http://24mat… │ https://w… │ … │ false       │ <empty>  │
│ 2 │ Actumag.info  │ http://actum… │ https://a… │ … │ false       │ <empty>  │
│ 3 │ 2012un-Nouve… │ http://2012u… │ http://ww… │ … │ false       │ <empty>  │
│ 4 │ 24heuresactu… │ http://24heu… │ http://24… │ … │ false       │ <empty>  │
│ 5 │ AgoraVox      │ http://agora… │ http://ww… │ … │ false       │ <empty>  │
│ 6 │ Al-Kanz.org   │ http://al-ka… │ https://w… │ … │ false       │ <empty>  │
│ 7 │ Alalumieredu… │ http://alalu… │ http://al… │ … │ false       │ <empty>  │
│ 8 │ Allodocteurs… │ http://allod… │ https://w… │ … │ false       │ <empty>  │
│ 9 │ Alterinfo.net │ http://alter… │ http://ww… │ … │ <empty>     │ true     │
│ … │ …             │ …             │ …          │ … │ …           │ …        │
└───┴───────────────┴───────────────┴────────────┴───┴─────────────┴──────────┘
```

On unix, don't hesitate to use the `-p` flag to automagically forward the full output to an appropriate pager and skim through all the columns.

### Reading a flattened representation of the first row

```bash
# NOTE: drop -c to avoid truncating the values
xan slice -l 1 medias.csv | xan flatten -c
```

```txt
Row n°0
───────────────────────────────────────────────────────────────────────────────
webentity_id                      1
name                              Acrimed.org
prefixes                          http://acrimed.org|http://acrimed69.blogspot…
home_page                         http://www.acrimed.org
start_pages                       http://acrimed.org|http://acrimed69.blogspot…
indegree                          61
hyphe_creation_timestamp          1560347020330
hyphe_last_modification_timestamp 1560526005389
outreach                          nationale
foundation_year                   2002
batch                             1
edito                             media
parody                            false
origin                            france
digital_native                    true
mediacloud_ids                    258269
wheel_category                    Opinion Journalism
wheel_subcategory                 Left Wing
has_paywall                       false
inactive                          <empty>
```

### Searching for rows

```bash
# Counting rows related to "Le Monde"
xan search -i -s name lemonde medias.csv | xan count
```

```txt
1
```

### Selecting some columns

```bash
xan select foundation_year,name medias.csv | xan view
```

```txt
Displaying 2 cols from 10 first rows of <stdin>
┌───┬─────────────────┬───────────────────────────────────────┐
│ - │ foundation_year │ name                                  │
├───┼─────────────────┼───────────────────────────────────────┤
│ 0 │ 2002            │ Acrimed.org                           │
│ 1 │ 2006            │ 24matins.fr                           │
│ 2 │ 2013            │ Actumag.info                          │
│ 3 │ 2012            │ 2012un-Nouveau-Paradigme.com          │
│ 4 │ 2010            │ 24heuresactu.com                      │
│ 5 │ 2005            │ AgoraVox                              │
│ 6 │ 2008            │ Al-Kanz.org                           │
│ 7 │ 2012            │ Alalumieredunouveaumonde.blogspot.com │
│ 8 │ 2005            │ Allodocteurs.fr                       │
│ 9 │ 2005            │ Alterinfo.net                         │
│ … │ …               │ …                                     │
└───┴─────────────────┴───────────────────────────────────────┘
```

### Sorting the file

```bash
xan sort -s foundation_year medias.csv | xan select name,foundation_year | xan view -l 10
```

```txt
Displaying 2 cols from 10 first rows of <stdin>
┌───┬────────────────────────────────────┬─────────────────┐
│ - │ name                               │ foundation_year │
├───┼────────────────────────────────────┼─────────────────┤
│ 0 │ Le Monde Numérique (Ouest France)  │ <empty>         │
│ 1 │ Le Figaro                          │ 1826            │
│ 2 │ Le journal de Saône-et-Loire       │ 1826            │
│ 3 │ L'Indépendant                      │ 1846            │
│ 4 │ Le Progrès                         │ 1859            │
│ 5 │ La Dépêche du Midi                 │ 1870            │
│ 6 │ Le Pélerin                         │ 1873            │
│ 7 │ Dernières Nouvelles d'Alsace (DNA) │ 1877            │
│ 8 │ La Croix                           │ 1883            │
│ 9 │ Le Chasseur Francais               │ 1885            │
│ … │ …                                  │ …               │
└───┴────────────────────────────────────┴─────────────────┘
```

### Computing frequency tables

```bash
xan frequency -s edito medias.csv | xan view
```

```txt
Displaying 3 cols from 5 rows of <stdin>
┌───┬───────┬────────────┬───────┐
│ - │ field │ value      │ count │
├───┼───────┼────────────┼───────┤
│ 0 │ edito │ media      │ 423   │
│ 1 │ edito │ individu   │ 30    │
│ 2 │ edito │ plateforme │ 14    │
│ 3 │ edito │ agrégateur │ 10    │
│ 4 │ edito │ agence     │ 1     │
└───┴───────┴────────────┴───────┘
```

### Printing a histogram

```bash
xan frequency -s edito medias.csv | xan hist
```

```txt
Histogram for edito (bars: 5, sum: 478, max: 423):

media      |423  88.49%|━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━|
individu   | 30   6.28%|━━━╸                                                  |
plateforme | 14   2.93%|━╸                                                    |
agrégateur | 10   2.09%|━╸                                                    |
agence     |  1   0.21%|╸                                                     |
```

### Computing descriptive statistics

```bash
xan stats -s indegree,edito medias.csv | xan transpose | xan view -I
```

```txt
Displaying 2 cols from 14 rows of <stdin>
┌─────────────┬───────────────────┬────────────┐
│ field       │ indegree          │ edito      │
├─────────────┼───────────────────┼────────────┤
│ count       │ 463               │ 478        │
│ count_empty │ 15                │ 0          │
│ type        │ int               │ string     │
│ types       │ int|empty         │ string     │
│ sum         │ 25987             │ <empty>    │
│ mean        │ 56.12742980561554 │ <empty>    │
│ variance    │ 4234.530197929737 │ <empty>    │
│ stddev      │ 65.07326792108829 │ <empty>    │
│ min         │ 0                 │ <empty>    │
│ max         │ 424               │ <empty>    │
│ lex_first   │ 0                 │ agence     │
│ lex_last    │ 99                │ plateforme │
│ min_length  │ 0                 │ 5          │
│ max_length  │ 3                 │ 11         │
└─────────────┴───────────────────┴────────────┘
```

### Using evaluated expressions to filter a file

```bash
# Finding all medias founded after 2000
# NOTE: some rows have no "foundation_year" so -E ignore makes
# sure we won't crash on those
xan filter -E ignore 'foundation_year > 2000' medias.csv | xan count
```

```txt
268
```

### Creating a new column based on other ones

```bash
xan map 'fmt("{} ({})", name, foundation_year)' key medias.csv | xan select key | xan slice -l 10
```

```txt
key
Acrimed.org (2002)
24matins.fr (2006)
Actumag.info (2013)
2012un-Nouveau-Paradigme.com (2012)
24heuresactu.com (2010)
AgoraVox (2005)
Al-Kanz.org (2008)
Alalumieredunouveaumonde.blogspot.com (2012)
Allodocteurs.fr (2005)
Alterinfo.net (2005)
```

### Performing custom aggregation

```bash
xan agg 'sum(indegree) as total_indegree, mean(indegree) as mean_indegree' medias.csv | xan view -I
```

```txt
Displaying 1 col from 1 rows of <stdin>
┌────────────────┬───────────────────┐
│ total_indegree │ mean_indegree     │
├────────────────┼───────────────────┤
│ 25987          │ 56.12742980561554 │
└────────────────┴───────────────────┘
```

### Grouping rows and performing per-group aggregation

```bash
xan groupby edito 'sum(indegree) as indegree' medias.csv | xan view -I
```

```txt
Displaying 1 col from 5 rows of <stdin>
┌────────────┬──────────┐
│ edito      │ indegree │
├────────────┼──────────┤
│ agence     │ 50       │
│ agrégateur │ 459      │
│ plateforme │ 658      │
│ media      │ 24161    │
│ individu   │ 659      │
└────────────┴──────────┘
```

## Available commands

- **agg** - Aggregate data from CSV file
- [**behead**](./docs/cmd/behead.md) - Drop header from CSV file
- **bins** - Dispatch numeric columns into bins
- **cat** - Concatenate by row or column
- [**count**](./docs/cmd/count.md) - Count records
- **datefmt** - Format a recognized date column to a specified format and timezone
- **dedup** - Deduplicate a CSV file
- **enum** - Enumerate CSV file by preprending an index column
- **explode** - Explode rows based on some column separator
- **filter** - Only keep some CSV rows based on an evaluated expression
- **fixlengths** - Makes all records have same length
- **flatmap** - Emit one row per value yielded by an expression evaluated for each CSV row
- **flatten** - Show one field per line
- **fmt** - Format CSV output (change field delimiter)
- **foreach** - Loop over a CSV file to execute bash commands
- **frequency** - Show frequency tables
- **from** - Convert a variety of formats to CSV
- **glob** - Create a CSV file with paths matching a glob pattern
- **groupby** - Aggregate data by groups of a CSV file
- [**headers**](./docs/cmd/headers.md) - Show header names
- **hist** - Print a histogram with rows of CSV file as bars
- **implode** - Collapse consecutive identical rows based on a diverging column
- **index** - Create CSV index for faster access
- **input** - Read CSV data with special quoting rules
- **join** - Join CSV files
- **map** - Create a new column by evaluating an expression on each CSV row
- **merge** - Merge multiple similar already sorted CSV files
- **partition** - Partition CSV data based on a column value
- **plot** - Draw a scatter plot or line chart
- **progress** - Display a progress bar while reading CSV data
- **range** - Create a CSV file from a numerical range
- **rename** - Rename columns of a CSV file
- **reverse** - Reverse rows of CSV data
- **sample** - Randomly sample CSV data
- **search** - Search CSV data with regexes
- **select** - Select columns from CSV
- **shuffle** - Shuffle CSV data
- **slice** - Slice records from CSV
- **sort** - Sort CSV data
- **split** - Split CSV data into many files
- **stats** - Compute basic statistics
- **transform** - Transform a column by evaluating an expression on each CSV row
- **transpose** - Transpose CSV file
- **union-find** - Apply the union-find algorithm on a CSV edge list
- **view** - Preview a CSV file in a human-friendly way

## General flags and IO model

### Getting help

If you ever feel lost, each command has a `-h/--help` flag that will print the related documentation.

### Specifying the file's delimiter

All `xan` commands accept a `-d/--delimiter` flag (defaulting to the standard `,`) to indicate what is the file's delimiter character.

Note that `xan` is perfectly able to infer the delimiter from typical file extensions such as `.tsv` or `.tab`.

### Working with headless CSV file

Even if this is good practice to name your columns, some CSV file simply don't have headers. Most commands are able to deal with those file if you give the `-n/--no-headers` flag.

Note that this flag always relates to the input, not the output. If for some reason you want to drop a CSV output's header row, use the `xan behead` command.

### Regarding stdin

By default, all commands will try to read from stdin when the file path is not specified. This makes piping easy and comfortable as it respects typical unix standards. Some commands may have multiple inputs (`xan join`, for instance), in which case stdin is usually specifiable using the `-` character:

```bash
# First file from stdin
cat file1.csv | xan join col1 - col2 file2.csv
```

Note that the command will also warn you when stdin cannot be read, in case you forgot to indicate the file's path.

### Regarding stdout

By default, all commands will print their output to stdout (note that this output is usually buffered for performance reasons).

In addition, all commands expose a `-o/--output` flag that can be use to specify where to write the output. This can be useful if you do not want to or cannot use `>` (typically in some Windows shells). In which case, `-` as a output path will mean forwarding to stdout also. This can be useful when scripting sometimes.

### Gzipped files

`xan` is able to read gzipped files (having a `.gz` extension) out of the box.
