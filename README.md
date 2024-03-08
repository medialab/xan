# xan

`xan` is a command line tool that can be used to process CSV files directly from the shell.

It has been written in Rust to be as performant as possible and can easily handle very large CSV files (Gigabytes). It is also able to leverage parallelism (through multithreading) to make some tasks complete as fast as your computer can allow.

It can easily preview, filter, slice, aggregate, join CSV files, and exposes a large collection of composable commands that can be chained together to perform a wide variety of typical tasks.

`xan` also exposes its own expression language so you can perform complex tasks that cannot be done by relying on the simplest commands. This minimalistic language has been tailored for CSV data and is faster than evaluating typical dynamically-typed languages such as Python, Lua, JavaScript etc.

Note that this tool is originally a fork of [BurntSushi](https://github.com/BurntSushi)'s [`xsv`](https://github.com/BurntSushi/xsv), but has been nearly entirely rewritten at that point, to fit [SciencesPo's médialab](https://github.com/medialab) use-cases, rooted in web data collection and analysis geared towards social sciences.

Finally, `xan` can be used to display CSV files in the terminal, for easy exploration, and can even be used to draw basic data visualisations.

*Displaying a CSV file in the terminal using `xan view`*

![view.png](./docs/img/view.png)

*Showing a flattened view of CSV records using `xan flatten`*

![flatten.png](./docs/img/flatten.png)

*Drawing a histogram of values using `xan hist`*

![hist.png](./docs/img/hist.png)

## How to install

`xan` can be installed using cargo (it usually comes with [Rust](https://www.rust-lang.org/tools/install)):

```
cargo install xan
```

## Quick tour

WIP...

## Available commands

- **agg** - Aggregate data from CSV file
- **behead** - Drop header from CSV file
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
- **headers** - Show header names
- **help** - Show this usage message.
- **hist** - Print a histogram with rows of CSV file as bars
- **implode** - Collapse consecutive identical rows based on a diverging column
- **index** - Create CSV index for faster access
- **input** - Read CSV data with special quoting rules
- **join** - Join CSV files
- **kway** - Merge multiple similar already sorted CSV files
- **map** - Create a new column by evaluating an expression on each CSV row
- **partition** - Partition CSV data based on a column value
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
- **view** - Preview a CSV file in a human-friendly way

If you ever feel lost, each command has a `-h/--help` flag that will print the related documentation.
