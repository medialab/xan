# `xan`, the CSV magician

`xan` is a command line tool that can be used to process CSV files directly from the shell.

It has been written in Rust to be as performant as possible and can easily handle very large CSV files (Gigabytes). It is also able to leverage parallelism (through multithreading) to make some tasks complete as fast as your computer can allow.

It can easily preview, filter, slice, aggregate, sort, join CSV files, and exposes a large collection of composable commands that can be chained together to perform a wide variety of typical tasks.

`xan` also leverages its own expression language so you can perform complex tasks that cannot be done by relying on the simplest commands. This minimalistic language has been tailored for CSV data and is faster than evaluating typical dynamically-typed languages such as Python, Lua, JavaScript etc.

Note that this tool is originally a fork of [BurntSushi](https://github.com/BurntSushi)'s [`xsv`](https://github.com/BurntSushi/xsv), but has been nearly entirely rewritten at that point, to fit [SciencesPo's médialab](https://github.com/medialab) use-cases, rooted in web data collection and analysis geared towards social sciences (you might think CSV is outdated by now, but read our [love letter](./docs/LOVE_LETTER.md) to the format before judging too quickly).

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

*Displaying a progress bar using `xan progress`*

<p align="center">
  <img alt="progress.gif" src="./docs/img/progress.gif" width="90%">
</p>

## Summary

* [How to install](#how-to-install)
* [Quick tour](#quick-tour)
* [Available commands](#available-commands)
* [General flags and IO model](#general-flags-and-io-model)
* [Expression language reference](#expression-language-reference)
  * [Syntax](#syntax)
  * [Functions & Operators](#functions--operators)
  * [Aggregation functions](#aggregation-functions)
* [Advanced use-cases](#advanced-use-cases)
* [Frequently Asked Questions](#frequently-asked-questions)

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

### Displaying the file's headers

```bash
xan headers medias.csv
```

```
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

```
478
```

### Previewing the file in the terminal

```bash
xan view medias.csv
```

```
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

```
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
xan search -s outreach internationale medias.csv | xan view
```

```
Displaying 4/20 cols from 10 first rows of <stdin>
┌───┬──────────────┬────────────────────┬───┬─────────────┬──────────┐
│ - │ webentity_id │ name               │ … │ has_paywall │ inactive │
├───┼──────────────┼────────────────────┼───┼─────────────┼──────────┤
│ 0 │ 25           │ Businessinsider.fr │ … │ false       │ <empty>  │
│ 1 │ 59           │ Europe-Israel.org  │ … │ false       │ <empty>  │
│ 2 │ 66           │ France 24          │ … │ false       │ <empty>  │
│ 3 │ 220          │ RFI                │ … │ false       │ <empty>  │
│ 4 │ 231          │ fr.Sott.net        │ … │ false       │ <empty>  │
│ 5 │ 246          │ Voltairenet.org    │ … │ true        │ <empty>  │
│ 6 │ 254          │ Afp.com /fr        │ … │ false       │ <empty>  │
│ 7 │ 265          │ Euronews FR        │ … │ false       │ <empty>  │
│ 8 │ 333          │ Arte.tv            │ … │ false       │ <empty>  │
│ 9 │ 341          │ I24News.tv         │ … │ false       │ <empty>  │
│ … │ …            │ …                  │ … │ …           │ …        │
└───┴──────────────┴────────────────────┴───┴─────────────┴──────────┘
```

### Selecting some columns

```bash
xan select foundation_year,name medias.csv | xan view
```

```
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

```
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

### Deduplicating the file on some column

```bash
# Some medias of our corpus have the same ids on mediacloud.org
xan dedup -s mediacloud_ids medias.csv | xan count && xan count medias.csv
```

```
457
478
```

Deduplicating can also be done while sorting:

```bash
xan sort -s mediacloud_ids -u medias.csv
```

### Computing frequency tables

```bash
xan frequency -s edito medias.csv | xan view
```

```
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

```
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

```
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

### Evaluating an expression to filter a file

```bash
xan filter 'batch > 1' medias.csv | xan count
```

```
130
```

To access the expression language's [cheatsheet](#syntax), run `xan filter --cheatsheet`. To display the full list of available [functions](#functions--operators), run `xan filter --functions`.

### Evaluating an expression to create a new column based on other ones

```bash
xan map 'fmt("{} ({})", name, foundation_year)' key medias.csv | xan select key | xan slice -l 10
```

```
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

To access the expression language's [cheatsheet](#syntax), run `xan map --cheatsheet`. To display the full list of available [functions](#functions--operators), run `xan map --functions`.

### Transform a column by evaluating an expression

```bash
xan transform name 'split(name, ".") | first | upper' medias.csv | xan select name | xan slice -l 10
```

```
name
ACRIMED
24MATINS
ACTUMAG
2012UN-NOUVEAU-PARADIGME
24HEURESACTU
AGORAVOX
AL-KANZ
ALALUMIEREDUNOUVEAUMONDE
ALLODOCTEURS
ALTERINFO
```

To access the expression language's [cheatsheet](#syntax), run `xan transform --cheatsheet`. To display the full list of available [functions](#functions--operators), run `xan transform --functions`.

### Performing custom aggregation

```bash
xan agg 'sum(indegree) as total_indegree, mean(indegree) as mean_indegree' medias.csv | xan view -I
```

```
Displaying 1 col from 1 rows of <stdin>
┌────────────────┬───────────────────┐
│ total_indegree │ mean_indegree     │
├────────────────┼───────────────────┤
│ 25987          │ 56.12742980561554 │
└────────────────┴───────────────────┘
```

To access the expression language's [cheatsheet](#syntax), run `xan agg --cheatsheet`. To display the full list of available [functions](#functions--operators), run `xan agg --functions`. Finally, to display the list of available [aggregation functions](#aggregation-functions), run `xan agg --aggs`.

### Grouping rows and performing per-group aggregation

```bash
xan groupby edito 'sum(indegree) as indegree' medias.csv | xan view -I
```

```
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

To access the expression language's [cheatsheet](#syntax), run `xan groupby --cheatsheet`. To display the full list of available [functions](#functions--operators), run `xan groupby --functions`. Finally, to display the list of available [aggregation functions](#aggregation-functions), run `xan groupby --aggs`.

## Available commands

*All commands are not fully documented on this README yet, but all the necessary information can be found directly from the command line. Just run `xan command -h` for help*

- **agg** - Aggregate data from CSV file
- [**behead**](./docs/cmd/behead.md) - Drop header from CSV file
- **bins** - Dispatch numeric columns into bins
- **blank** - Blank down a CSV file
- **cat** - Concatenate by row or column
- **cluster** - Cluster CSV data to find near-duplicates
- [**count**](./docs/cmd/count.md) - Count records
- **dedup** - Deduplicate a CSV file
- **enum** - Enumerate CSV file by preprending an index column
- **explode** - Explode rows based on some column separator
- **filter** - Only keep some CSV rows based on an evaluated expression
- **fixlengths** - Makes all records have same length
- **flatmap** - Emit one row per value yielded by an expression evaluated for each CSV row
- **flatten** - Show one field per line
- **fmt** - Format CSV output (change field delimiter)
- **foreach** - Loop over a CSV file to perform side effects
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
- **tokenize** - Tokenize a text column
- **top** - Find top rows of a CSV file according to some column
- **transform** - Transform a column by evaluating an expression on each CSV row
- **transpose** - Transpose CSV file
- **union-find** - Apply the union-find algorithm on a CSV edge list
- **view** - Preview a CSV file in a human-friendly way
- **vocab** - Build a vocabulary over tokenized documents

## General flags and IO model

### Getting help

If you ever feel lost, each command has a `-h/--help` flag that will print the related documentation.

### Regarding input & output formats

All `xan` commands expect a "standard" CSV file, e.g. comma-delimited, with proper double-quote escaping. This said, `xan` is also perfectly able to infer the delimiter from typical file extensions such as `.tsv` or `.tab`.

If you need to process a file with a custom delimiter, you can either use the `xan input` command or use the `-d/--delimiter` flag available with all commands.

If you need to output a custom CSV dialect (e.g. using `;` delimiters), feel free to use the `xan fmt` command.

Finally, even if most `xan` commands won't even need to decode the file's bytes, some might still need to. In this case, `xan` will expect correctly formatted UTF-8 text. Please use `iconv` or other utils if you need to process other encodings such as `latin1` ahead of `xan`.

### Working with headless CSV file

Even if this is good practice to name your columns, some CSV file simply don't have headers. Most commands are able to deal with those file if you give the `-n/--no-headers` flag.

Note that this flag always relates to the input, not the output. If for some reason you want to drop a CSV output's header row, use the `xan behead` command.

### Regarding stdin

By default, all commands will try to read from stdin when the file path is not specified. This makes piping easy and comfortable as it respects typical unix standards. Some commands may have multiple inputs (`xan join`, for instance), in which case stdin is usually specifiable using the `-` character:

```bash
# First file given to join will be read from stdin
cat file1.csv | xan join col1 - col2 file2.csv
```

Note that the command will also warn you when stdin cannot be read, in case you forgot to indicate the file's path.

### Regarding stdout

By default, all commands will print their output to stdout (note that this output is usually buffered for performance reasons).

In addition, all commands expose a `-o/--output` flag that can be use to specify where to write the output. This can be useful if you do not want to or cannot use `>` (typically in some Windows shells). In which case, `-` as a output path will mean forwarding to stdout also. This can be useful when scripting sometimes.

### Gzipped files

`xan` is able to read gzipped files (having a `.gz` extension) out of the box.

## Expression language reference

### Syntax

This help can be found in the terminal by executing `xan map --cheatsheet`.

```
xan script language cheatsheet (use --functions for comprehensive list of
available functions & operators):

  . Indexing a column by name:
        'name'

  . Indexing column with forbidden characters (e.g. spaces, commas etc.):
        'col("Name of film")'

  . Indexing column by index (0-based):
        'col(2)'

  . Indexing a column by name and 0-based nth (for duplicate headers):
        'col("col", 1)'

  . Applying functions:
        'trim(name)'
        'trim(concat(name, " ", surname))'

  . Using operators (unary & binary):
        '-nb1'
        'nb1 + nb2'
        '(nb1 > 1) || nb2'

  . Integer literals:
        '1'

  . Float literals:
        '0.5'

  . Boolean literals:
        'true'
        'false'

  . Null literals:
        'null'

  . String literals (can use single or double quotes):
        '"hello"'
        "'hello'"

  . Regex literals:
        '/john/'
        '/john/i' (case-insensitive)

  . Nesting function calls:
        'add(sub(col1, col2), mul(col3, col4))'
```

### Functions & Operators

This help can be found in the terminal by executing `xan map --functions`.

```
# Available functions & operators

(use --cheatsheet for a reminder of the expression language's basics)

## Operators

### Unary operators

    !x - boolean negation
    -x - numerical negation,

### Numerical comparison

Warning: those operators will always consider operands as numbers and will
try to cast them around as such. For string/sequence comparison, use the
operators in the next section.

    x == y - numerical equality
    x != y - numerical inequality
    x <  y - numerical less than
    x <= y - numerical less than or equal
    x >  y - numerical greater than
    x >= y - numerical greater than or equal

### String/sequence comparison

Warning: those operators will always consider operands as strings or
sequences and will try to cast them around as such. For numerical comparison,
use the operators in the previous section.

    x eq y - string equality
    x ne y - string inequality
    x lt y - string less than
    x le y - string less than or equal
    x gt y - string greater than
    x ge y - string greater than or equal

### Arithmetic operators

    x + y  - numerical addition
    x - y  - numerical subtraction
    x * y  - numerical multiplication
    x / y  - numerical division
    x % y  - numerical remainder

    x // y - numerical integer division
    x ** y - numerical exponentiation

## String operators

    x . y - string concatenation

## Logical operators

    x &&  y - logical and
    x and y
    x ||  y - logical or
    x or  y

    x in y
    x not in y

## Pipeline operator (using "_" for left-hand size substitution)

    'trim(name) | len(_)'         - Same as len(trim(name))
    'trim(name) | len'            - Supports elision for unary functions
    'trim(name) | add(1, len(_))' - Can be nested
    'add(trim(name) | len, 2)'    - Can be used anywhere

## Arithmetics

    - abs(x) -> number
        Return absolute value of number.

    - add(x, y, *n) -> number
        Add two or more numbers.

    - argmax(numbers, labels?) -> any
        Return the index or label of the largest number in the list.

    - argmin(numbers, labels?) -> any
        Return the index or label of the smallest number in the list.

    - ceil(x) -> number
        Return the smallest integer greater than or equal to x.

    - div(x, y, *n) -> number
        Divide two or more numbers.

    - floor(x) -> number
        Return the smallest integer lower than or equal to x.

    - idiv(x, y) -> number
        Integer division of two numbers.

    - log(x) -> number
        Return the natural logarithm of x.

    - max(x, y, *n) -> number
    - max(list_of_numbers) -> number
        Return the maximum number.

    - min(x, y, *n) -> number
    - min(list_of_numbers) -> number
        Return the minimum number.

    - mod(x, y) -> number
        Return the remainder of x divided by y.

    - mul(x, y, *n) -> number
        Multiply two or more numbers.

    - neg(x) -> number
        Return -x.

    - pow(x, y) -> number
        Raise x to the power of y.

    - round(x) -> number
        Return x rounded to the nearest integer.

    - sqrt(x) -> number
        Return the square root of x.

    - sub(x, y, *n) -> number
        Subtract two or more numbers.

    - trunc(x) -> number
        Truncate the number by removing its decimal part.

## Boolean operations & branching

    - and(a, b, *x) -> T
        Perform boolean AND operation on two or more values.

    - if(cond, then, else?) -> T
        Evaluate condition and switch to correct branch.

    - unless(cond, then, else?) -> T
        Shorthand for `if(not(cond), then, else?)`.

    - not(a) -> bool
        Perform boolean NOT operation.

    - or(a, b, *x) -> T
        Perform boolean OR operation on two or more values.

## Comparison

    - eq(s1, s2) -> bool
        Test string or sequence equality.

    - ne(s1, s2) -> bool
        Test string or sequence inequality.

    - gt(s1, s2) -> bool
        Test that string or sequence s1 > s2.

    - ge(s1, s2) -> bool
        Test that string or sequence s1 >= s2.

    - lt(s1, s2) -> bool
        Test that string or sequence s1 < s2.

    - ge(s1, s2) -> bool
        Test that string or sequence s1 <= s2.

## String & sequence helpers

    - compact(list) -> list
        Drop all falsey values from given list.

    - concat(string, *strings) -> string
        Concatenate given strings into a single one.

    - contains(seq, subseq) -> bool
        Find if subseq can be found in seq. Subseq can
        be a regular expression.

    - count(seq, pattern) -> int
        Count number of times pattern appear in seq. Pattern
        can be a regular expression.

    - endswith(string, pattern) -> bool
        Test if string ends with pattern.

    - escape_regex(string) -> string
        Escape a string so it can be used safely in a regular expression.

    - first(seq) -> T
        Get first element of sequence.

    - fmt(string, *replacements):
        Format a string by replacing "{}" occurrences by subsequent
        arguments.

        Example: `fmt("Hello {} {}", name, surname)` will replace
        the first "{}" by the value of the name column, then the
        second one by the value of the surname column.

    - get(target, index_or_key, default?) -> T
        Get nth element of sequence (can use negative indexing), or key of mapping.
        Returns nothing if index or key is not found or alternatively the provided
        default value.

    - join(seq, sep) -> string
        Join sequence by separator.

    - last(seq) -> T
        Get last element of sequence.

    - len(seq) -> int
        Get length of sequence.

    - ltrim(string, pattern?) -> string
        Trim string of leading whitespace or
        provided characters.

    - lower(string) -> string
        Lowercase string.

    - replace(string, pattern, replacement) -> string
        Replace pattern in string. Can use a regex.

    - rtrim(string, pattern?) -> string
        Trim string of trailing whitespace or
        provided characters.

    - slice(seq, start, end?) -> seq
        Return slice of sequence.

    - split(string, sep, max?) -> list
        Split a string by separator.

    - startswith(string, pattern) -> bool
        Test if string starts with pattern.

    - trim(string, pattern?) -> string
        Trim string of leading & trailing whitespace or
        provided characters.

    - unidecode(string) -> string
        Convert string to ascii as well as possible.

    - upper(string) -> string
        Uppercase string.

## Utils

    - coalesce(*args) -> T
        Return first truthy value.

    - col(name_or_pos, nth?) -> string
        Return value of cell for given column, by name, by position or by
        name & nth, in case of duplicate header names.

    - cols(from_name_or_pos?, to_name_or_pos?) -> list
        Return list of cell values from the given colum by name or position
        to another given column by name or position, inclusive.
        Can also be called with a single argument to take a slice from the
        given column to the end, or no argument at all to take all columns.

    - err(msg) -> error
        Make the expression return a custom error.

    - headers(from_name_or_pos?, to_name_or_pos?) -> list
        Return list of header names from the given colum by name or position
        to another given column by name or position, inclusive.
        Can also be called with a single argument to take a slice from the
        given column to the end, or no argument at all to return all headers.

    - index() -> integer?
        Return the row's index, if applicable.

    - json_parse(string) -> T
        Parse the given string as JSON.

    - typeof(value) -> string
        Return type of value.

    - val(value) -> T
        Return a value as-is. Useful to return constants.

## IO & path wrangling

    - abspath(string) -> string
        Return absolute & canonicalized path.

    - bytesize(integer) -> string
        Return a number of bytes in human-readable format (KB, MB, GB, etc.).

    - ext(path) -> string?
        Return the path's extension, if any.

    - filesize(string) -> int
        Return the size of given file in bytes.

    - isfile(string) -> bool
        Return whether the given path is an existing file on disk.

    - pathjoin(string, *strings) -> string
        Join multiple paths correctly.

    - read(path, encoding?, errors?) -> string
        Read file at path. Default encoding is "utf-8".
        Default error handling policy is "replace", and can be
        one of "replace", "ignore" or "strict".

    - write(string, path) -> string
        Write string to path as utf-8 text. Will create necessary
        directories recursively before actually writing the file.
        Return the path that was written.

## Random

    - md5(string) -> string
        Return the md5 hash of string in hexadecimal representation.

    - uuid() -> string
        Return a uuid v4.
```

### Aggregation functions

This help can be found in the terminal by executing `xan agg --aggs`.

```
# Available aggregation functions

(use --cheatsheet for a reminder of how the scripting language works)

Note that most functions ignore null values (empty strings), but that functions
operating on numbers will yield an error if encountering a string that cannot
be safely parsed as a number.

You can always use `coalesce` to nudge values around and force aggregation functions to
consider null values or make them avoid non-numerical values altogether.

Example: considering null values when computing a mean => 'mean(coalesce(number, 0))'

    - all(<expr>) -> bool
        Returns true if all elements returned by given expression are truthy.

    - any(<expr>) -> bool
        Returns true if one of the elements returned by given expression is truthy.

    - argmin(<expr>, <expr>?) -> any
        Return the index of the row where the first expression is minimized, or
        the result of the second expression where the first expression is minimized.

    - argmax(<expr>, <expr>?) -> any
        Return the index of the row where the first expression is maximized, or
        the result of the second expression where the first expression is maximized.

    - avg(<expr>) -> number
        Average of numerical values. Same as `mean`.

    - cardinality(<expr>) -> number
        Number of distinct values returned by given expression.

    - count(<expr>?) -> number
        Count the number of row. Works like in SQL in that `count(<expr>)`
        will count all non-empy values returned by given expression, while
        `count()` without any expression will count every matching row.

    - count_empty(<expr>) -> number
        Count the number of empty values returned by given expression.

    - distinct_values(<expr>, separator?) -> string
        List of sorted distinct values joined by a pipe character ('|') by default or by
        the provided separator.

    - first(<expr>) -> string
        Return first seen non empty element of the values returned by the given expression.

    - last(<expr>) -> string
        Return last seen non empty element of the values returned by the given expression.

    - lex_first(<expr>) -> string
        Return first string in lexicographical order.

    - lex_last(<expr>) -> string
        Return last string in lexicographical order.

    - min(<expr>) -> number | string
        Minimum numerical value.

    - max(<expr>) -> number | string
        Maximum numerical value.

    - mean(<expr>) -> number
        Mean of numerical values. Same as `avg`.

    - median(<expr>) -> number
        Median of numerical values, interpolating on even counts.

    - median_high(<expr>) -> number
        Median of numerical values, returning higher value on even counts.

    - median_low(<expr>) -> number
        Median of numerical values, returning lower value on even counts.

    - mode(<expr>) -> string
        Value appearing the most, breaking ties arbitrarily in favor of the
        first value in lexicographical order.

    - quantile(<expr>, p) -> number
        Return the desired quantile of numerical values.

    - q1(<expr>) -> number
        Return the first quartile of numerical values.

    - q2(<expr>) -> number
        Return the second quartile of numerical values. Alias for median.

    - q3(<expr>) -> number
        Return the third quartile of numerical values.

    - stddev(<expr>) -> number
        Population standard deviation. Same as `stddev_pop`.

    - stddev_pop(<expr>) -> number
        Population standard deviation. Same as `stddev`.

    - stddev_sample(<expr>) -> number
        Sample standard deviation (i.e. using Bessel's correction).

    - sum(<expr>) -> number
        Sum of numerical values.

    - type(<expr>) -> string
        Best type description for seen values.

    - types(<expr>) -> string
        Sorted list, pipe-separated, of all the types seen in the values.

    - values(<expr>, separator?) -> string
        List of values joined by a pipe character ('|') by default or by
        the provided separator.

    - var(<expr>) -> number
        Population variance. Same as `var_pop`.

    - var_pop(<expr>) -> number
        Population variance. Same as `var`.

    - var_sample(<expr>) -> number
        Sample variance (i.e. using Bessel's correction).
```

## Advanced use-cases

### Reading files in parallel

Let's say one column of your CSV file is containing paths to files, relative to some `downloaded` folder, and you want to make sure all of them contain some string (maybe you crawled some website and want to make sure you were correctly logged in by searching for some occurrence of your username):

```bash
xan progress files.csv | \
xan filter -p 'pathjoin("downloaded", path) | read | !contains(_, /yomguithereal/i)' > not-logged.csv
```

### Generating a CSV of paginated urls to download

Let's say you want to download the latest 50 pages from [Hacker News](https://news.ycombinator.com) using another of our tools named [minet](https://github.com/medialab/minet).

You can pipe `xan range` into `xan select -e` into `minet fetch`:

```bash
xan range -s 1 50 -i | \
xan select -e '"https://news.ycombinator.com/?p=".n as url' | \
minet fetch url -i -
```

### Piping to `xargs`

Let's say you want to delete all files whose path can be found in a column of CSV file. You can select said column and format it with `xan` before piping to `xargs`:

```bash
xan select path files.csv | \
xan behead | \
xan fmt --quote-never | \
xargs -I {} rm {};
```

## Frequently Asked Questions

### How to display a vertical bar chart?

Rotate your screen ;\)
