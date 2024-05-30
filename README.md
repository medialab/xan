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

## How to install

`xan` can be installed using cargo (it usually comes with [Rust](https://www.rust-lang.org/tools/install)):

```
cargo install xan
```

## Quick tour

Let's learn about the most commonly used `xan` commands by exploring a corpus of French medias:

*Downloading the corpus*

```bash
curl -LO https://github.com/medialab/corpora/raw/master/polarisation/medias.csv
```
*Displaying the file's headers*

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
