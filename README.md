# xan

**Warning**: this repository stores [SciencesPo's médialab](https://medialab.sciencespo.fr/en/) fork of [BurntSushi](https://github.com/BurntSushi)'s [`xsv`](https://github.com/BurntSushi/xsv) command line tool.

Feel free to use it, if you feel its added [features](#new-features) are useful to your own workflows.

## Presentation

`xan` is a command line program for indexing, slicing, analyzing, splitting
and joining CSV files. Commands should be simple, fast and composable:

1. Simple tasks should be easy.
2. Performance trade offs should be exposed in the CLI interface.
3. Composition should not come at the expense of performance.

This README contains information on how to
[install `xan`](#installation), in addition to
a quick tour of several commands.

Dual-licensed under MIT or the [UNLICENSE](https://unlicense.org).

### How to install

`xan` can be installed using cargo:

```
cargo install xan
```

<strong id="new-features">New Features</strong>

* `xan agg`
* `xan behead`
* `xan bins`
* `xan datefmt`
* `xan dedup`
* `xan enum`
* `xan explode`
* `xan flatmap`
* `xan filter`
* `xan foreach`
* `xan glob`
* `xan groupby`
* `xan hist`
* `xan implode`
* `xan join --prefix-left/--prefix-right`
* `xan jsonl`
* `xan kway`
* `xan map`
* `xan rename`
* `xan reverse --in-memory`
* `xan search --exact`
* `xan search --flag col`
* `xan shuffle`
* `xan sort -u`
* `xan transform`
* `xan view`
* `xan xls`

### Available commands

* **agg** - Aggregate data from CSV file.
* **behead** - Drop headers from CSV file.
* **bins** - Dispatch numeric columns into bins.
* **cat** - Concatenate CSV files by row or by column.
* **count** - Count the rows in a CSV file. (Instantaneous with an index.)
* **datefmt** - Add a column with the date from a CSV column in a specified format and timezone.
* **dedup** - Deduplicate a CSV file.
* **enum** - Enumerate CSV file by preprending an index column.
* **explode** - Explode rows into multiple ones by splitting a column value based on the
given separator.
* **filter** - Only keep some CSV rows based on an evaluated expression.
* **fixlengths** - Force a CSV file to have same-length records by either
  padding or truncating them.
* **flatmap** - Emit one row per value yielded by an expression evaluated for each CSV row.
* **flatten** - A flattened view of CSV records. Useful for viewing one record
  at a time. e.g., `xan slice -i 5 data.csv | xan flatten`.
* **fmt** - Reformat CSV data with different delimiters, record terminators
  or quoting rules. (Supports ASCII delimited data.)
* **foreach** - Loop over a CSV file to execute bash commands.
* **frequency** - Build frequency tables of each column in CSV data. (Uses
  parallelism to go faster if an index is present.)
* **glob** - Create a CSV file with paths matching a glob pattern.
* **groupby** - Aggregate data by groups of a CSV file.
* **headers** - Show the headers of CSV data. Or show the intersection of all
  headers between many CSV files.
* **implode** - Collapse consecutive identical rows based on a diverging column.
* **index** - Create an index for a CSV file. This is very quick and provides
  constant time indexing into the CSV file.
* **input** - Read CSV data with exotic quoting/escaping rules.
* **jsonl** - Convert newline-delimited JSON to CSV.
* **join** - Inner, outer and cross joins. Uses a simple hash index to make it
  fast.
* **kway** - Merge multiple similar already sorted CSV files.
* **lang**, *optional* - Add a column with the language detected in a given CSV column.
* **map** - Create a new column by evaluating an expression on each CSV row.
* **partition** - Partition CSV data based on a column value.
* **pseudo** - Pseudonymise the value of the given column by replacing them by an incremental identifier.
* **sample** - Randomly draw rows from CSV data using reservoir sampling (i.e.,
  use memory proportional to the size of the sample).
* **rename** - Rename columns of a CSV file.
* **reverse** - Reverse order of rows in CSV data.
* **search** - Run a regex over CSV data. Applies the regex to each field
  individually and shows only matching rows.
* **select** - Select or re-order columns from CSV data.
* **shuffle** - Shuffle rows of a CSV file.
* **slice** - Slice rows from any part of a CSV file. When an index is present,
  this only has to parse the rows in the slice (instead of all rows leading up
  to the start of the slice).
* **sort** - Sort CSV data.
* **split** - Split one CSV file into many CSV files of N chunks.
* **stats** - Show basic types and statistics of each column in the CSV file.
  (i.e., mean, standard deviation, median, range, etc.)
* **transform** - Transform a column by evaluating an expression on each CSV row.
* **view** - Preview a CSV file in a human-friendly way.
* **xls** - Convert Excel/OpenOffice spreadsheets to CSV.
