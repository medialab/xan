<!-- Generated -->
# xan help

```txt

Usage:
    xan <command> [<args>...]
    xan [options]

Options:
    --list        List all commands available.
    -h, --help    Display this message
    <command> -h  Display the command help message
    --version     Print version info and exit

Commands:
    help        Show this usage message.

## Explore & visualize
    count       Count rows in file
    headers (h) Show header names
    view    (v) Preview a CSV file in a human-friendly way
    flatten (f) Display a flattened version of each row of a file
    hist        Print a histogram with rows of CSV file as bars
    plot        Draw a scatter plot or line chart
    progress    Display a progress bar while reading CSV data

## Search & filter
    search      Search CSV data with regexes
    filter      Only keep some CSV rows based on an evaluated expression
    slice       Slice rows of CSV file
    top         Find top rows of a CSV file according to some column
    sample      Randomly sample CSV data

## Sort & deduplicate
    sort        Sort CSV data
    dedup       Deduplicate a CSV file
    shuffle     Shuffle CSV data

## Aggregate
    frequency (freq) Show frequency tables
    groupby          Aggregate data by groups of a CSV file
    stats            Compute basic statistics
    agg              Aggregate data from CSV file
    bins             Dispatch numeric columns into bins

## Combine multiple CSV files
    cat         Concatenate by row or column
    join        Join CSV files
    merge       Merge multiple similar already sorted CSV files

## Add, transform, drop and move columns
    select      Select columns from CSV
    map         Create a new column by evaluating an expression on each CSV row
    transform   Transform a column by evaluating an expression on each CSV row
    enum        Enumerate CSV file by preprending an index column
    flatmap     Emit one row per value yielded by an expression evaluated for each CSV row

## Format, convert & recombobulate
    behead      Drop header from CSV file
    rename      Rename columns of a CSV file
    input       Read CSV data with special quoting rules
    fixlengths  Makes all rows have same length
    fmt         Format CSV output (change field delimiter)
    explode     Explode rows based on some column separator
    implode     Collapse consecutive identical rows based on a diverging column
    from        Convert a variety of formats to CSV
    reverse     Reverse rows of CSV data
    transpose   Transpose CSV file

## Split a CSV file into multiple
    split       Split CSV data into chunks
    partition   Partition CSV data based on a column value

## Parallel operation over multiple CSV files
    parallel (p) Map-reduce-like parallel computation

## Generate CSV files
    glob        Create a CSV file with paths matching a glob pattern
    range       Create a CSV file from a numerical range

## Perform side-effects
    foreach     Loop over a CSV file to perform side effects

## Lexicometry & fuzzy matching
    tokenize    Tokenize a text column
    vocab       Build a vocabulary over tokenized documents
    cluster     Cluster CSV data to find near-duplicates

## Graph algorithms
    union-find  Apply the union-find algorithm on a CSV edge list
```
