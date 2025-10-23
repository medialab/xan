# Expression language cheatsheet

Some `xan` commands such as `xan map`, `xan filter`, `xan groupby` etc. allow
users to evaluate custom expressions to filter/transform/create values from
the processed CSV data.

The expression language (nicknamed `moonblade` here and there) should be very
reminiscent of most high-level dynamically typed languages like Python or
JavaScript. It is however limited, memory-efficient, tailored for CSV data and
therefore faster than the beforementioned languages.

It does not support anything more than evaluating simple expressions and is loaded
with a lot of helpful functions that you can review using [`xan help functions`](./functions.md)
and that should be enough for most typical tasks. To read about aggregation
capabilities, on the other hand, you should use [`xan help aggs`](./aggs.md) instead.

Note finally that it was not designed to be sandboxed nor to be particularly safe,
so precautions regarding evaluating untrusted expressions provided by users should
apply here too.

## Summary

- [Basic examples](#basic-examples)
- [Literal values & data types](#literal-values--data-types)
- [Referencing columns](#referencing-columns)
- [Operators & calling functions](#operators--calling-functions)
- [Indexing & slicing](#indexing--slicing)
- [Higher-order functions](#higher-order-functions)
- [Constant evaluation](#constant-evaluation)
- [Named expressions](#named-expressions)
- [Multiple lines & comments](#multiple-lines--comments)
- [Implementation details & design choices](#implementation-details--design-choices)

## Basic examples

```python
# Checking that the value of the "count" column is over 10
count > 10

# Lowercasing a "text" column
lower(text)

# Checking that a lowercased name has some desired value (mind the "eq" operator):
lower(name) eq "john"

# Checking that a "name" column is one of the provided values:
name in ["john", "lucy", "mary"]

# Formatting a full name from a "first_name" and "last_name" column:
fmt("{} {}", first_name, last_name)

# Getting the first part of a mime type:
split(mimetype, "/")[0]
```

A common pitfall is to forget that string operators are different than numerical
ones, like in Perl. String equality is actually `eq`, not `==`, and string
concatenation would be `++` not `+`. Read the design choices section if you
want to understand why.

## Referencing columns

Column can be referenced directly by name if they only contain alphanumeric
characters or underscores and don't start with a number:

```python
# Computing the ratio between "tweet_count" and "retweet_count":
tweet_count / retweet_count
```

If the column names contain forbidden characters, or if you need to access
columns with duplicate names,  they can be accessed through the `col` function:

```python
# Column name with spaces:
col("Name of Movie")
# Second column named "text":
col("text", 1)
```

It is also possible to access columns by their zero-based index (negative indices
are also accepted):

```python
# Third column:
col(2)
# Last column:
col(-1)
```

If an identifier or a `col` call tries to access an inexisting column in target
CSV file, `xan` will usually throw an error before even attempting to evaluate
the given expression. This can be problematic sometimes when you want to process
many different files with slightly different column names. To this end, you
can also use "unsure" identifiers, postfixed with `?`, or the `col?` function like so:

```python
# Will return the "text" column or the "content" one if not found
text? || content?
# Same but for column with complicated names etc.
col?("Subscription Date")
```

Note that the language does not have user-defined variables and identifiers always
refer to some column or some program-defined value like in `xan tokenize --flatmap`
or `xan dedup --choose`.

This said, the pipeline operator (`|`) refers to the result of the left-hand side
expression through the special `_` identifier. This can be used to avoid repetition
sometimes:

```python
split(date, '/') | fmt('{}-{}-{}', _[2], _[0], _[1])
```

## Literal values & data types

```python
# Integers
1
# Integers can contain underscores for readability
10_000

# Floats
0.5

# Booleans
true
false

# Null value
null

# Strings (single quotes, double quotes, backticks)
"hello"
'hello'
`hello`

# Typical escaping
"Hello\nThis is world!"
# Supported: \n, \r, \t, \\, \", \', \0, \x67 and \u{1F60A}

# Binary strings (single quotes, double quotes, backticks)
b"hello"
b'hello'
b`hello`

# Regexes
/john/

# Case-insensitive regexes
/john/i

# Lists
[1, 2, 3]
["one", "two"]

# Maps
{"one": 1, "two": 2}
{one: 1, two: 2}
{leaf: "hello", nested: [1, 2, 3]}
```

## Operators & calling functions

Operators:

```python
# Unary operators:
-count
!has_description

# Binary operators:
count1 + count2
count1 < count2

# Nested expressions:
(count1 > 1) || count2
```

Functions:

```python
# Simple call
trim(name)

# Nested call
trim(concat(name, " ", surname))

# Using the operator "." is the same as calling a function with left operand
# as first argument
name.trim()
# is equivalent to:
trim(name)

"data".pathjoin(filename)
# is equivalent to:
pathjoin("data", filename)

# Some functions accepts named arguments:
read(path, encoding="utf8")
```

For a full list of available operators and functions, check out [`xan help functions`](./functions.md).

## Indexing & slicing

Indexing and slicing works a lot like in Python and JavaScript:

```python
# Zero-based indexing:
list[1]

# Negative indexing:
list[-2]

# Slicing:
list[1:4]
list[:4]
list[1:]

# Negative slicing:
list[1:-3]
list[:-2]
list[-4:]

# Key-based indexing:
map["name"]
# Same as:
map.name
```

## Higher-order functions

Higher-order functions, such as `map` or `filter`, also exists in the language
and can be given anonymous functions like so:

```javascript
map(numbers, x => x + 2)
filter(users, name => "john" in name)
```

## Constant evaluation

Note that the language will always perform some level of static analysis of the
given expression to assess which part actually need to run for each of the
processed CSV rows.

This means that constant parts of the expressions will be evaluated only once
when parsed, then folded into a new, simpler expression.

This can be very useful when, for instance, reading some JSON file to perform
one lookup per row like so:

```python
# Here, "config.json" will only be read once when parsing the expression,
# not once per processed CSV row, which is fortunate.
read_json("config.json").name
```

To debug and/or experiment with the expression static analysis, check out the
`xan eval --explain` command.

## Named expressions

Some commands, typically `xan map`, `xan agg`, `xan groupby` and `xan select -e`
let their user provide a series of named expression, separated by comma, rather
than a single expression.

Here is how they work:

```python
# Anonymous expressions (names will be created from the stringified expressions)
sum(retweets), retweets / replies

# Named expressions
sum(retweets) as total_retweets, retweets / replies as ratio

# Names with special characters
sum(retweets) as "Total Retweets"
```

In some commands, namely `xan select -e` & `xan map`, it is even possible for
named expressions to return multiple things at once, in order to avoid repeating
computations (notice the parenthesis around names):

```python
# Splitting full names
full_name.split(" ") as (first_name, last_name)

# Extracting data from a JSON cell
data.parse_json() | [_.name, _.meta[2].age] as (name, age)
```

Note that the expression must return a list of items having exactly the correct
number of values or an error will be raised.

## Multiple lines & comments

Expressions can be written on multiple lines freely:

```python
sum(
  retweets +
  replies
)
```

Comments can be added starting with `#`:

```python
# Summing
sum(
  retweets + # we add retweets,
  replies # and replies
)
```

## Implementation details & design choices

The `moonblade` expression language uses a PEG-like parser and a tree-walker
interpreter over a single `enum` of dynamic data types. It does not rely on
garbage collection to operate.

Since CSV data contain only strings and is not typed whatsoever, some design choices
were made to make sure expressions would stick to this reality, all while
remaining comfortable to write:

- To avoid requiring users to explicitly cast their values to some numerical
representation, for instance, we use two sets of operators for string-like types
and for other types (`eq` vs. `==` for instance). This is reminiscent of Perl.

- Most functions semantically encode what type they will operate on. Consider the
difference between the aggregation functions `min` and `lex_first`. One will consider
numbers, the other one strings.
