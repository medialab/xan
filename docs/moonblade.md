# xan expression language reference

* [Cheatsheet](#cheatsheet)
* [Functions & Operators](#functions--operators)
* [Aggregation functions](#aggregation-functions)

## Cheatsheet

```txt
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

  . Indexing a column that may not exist:
        'name?'

  . Applying functions:
        'trim(name)'
        'trim(concat(name, " ", surname))'

  . Named function arguments:
        'read(path, encoding="utf-8")'

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

  . Binary string literals (can use single or double quotes):
        'b"hello"'
        "b'hello'"

  . Regex literals:
        '/john/'
        '/john/i' (case-insensitive)

  . List literals:
        '[1, 2, 3]'
        '["one", "two"]

  . Map literals:
        '{one: 1, two: 2}'
        '{leaf: "hello", "nested": [1, 2, 3]}'

Note that constant expressions will never be evaluated more than once
when parsing the program.

This means that when evaluating the following:
    'get(read_json("config.json"), name)'

The "config.json" file will never be read/parsed more than once and will not
be read/parsed once per row.
```

## Functions & Operators

```md
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

## Indexing & slicing operators

    x[y] - get y from x (string or list index, map key)
    x[start:end] - slice x from start index to end index
    x[:end] - slice x from start to end index
    x[start:] - slice x from start index to end

    Negative indices are accepted and mean the same thing as with
    the Python language.

## Pipeline operator (using "_" for left-hand side substitution)

    trim(name) | len(_)         - Same as len(trim(name))
    trim(name) | len            - Supports elision for unary functions
    trim(name) | add(1, len(_)) - Can be nested
    add(trim(name) | len, 2)    - Can be used anywhere

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
        Will actually short-circuit. Contrary to "or" and "and".

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

    - fmt(string, *replacements) -> string:
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

    - match(string, pattern, group?) -> string
        Return a regex pattern match on the string.

    - numfmt(number) -> string:
        Format a number with thousands separator and proper significance.

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

## Dates

    - datetime(string, format=?, timezone=?) -> datetime
        Parse a string as a datetime according to format and timezone
        (https://docs.rs/jiff/latest/jiff/fmt/strtime/index.html#conversion-specifications).
        If no format is provided, string is parsed as ISO 8601 date format.
        Default timezone is the system timezone.

    - strftime(target, format, timezone=?) -> string
        Format target (a time in ISO 8601 format,
        or the result of datetime() function) according to format.

    - timestamp(number) -> datetime
        Parse a number as a POSIX timestamp in seconds
        (nb of seconds since 1970-01-01 00:00:00 UTC),
        and convert it to a datetime in local time.

    - timestamp_ms(number) -> datetime
        Parse a number as a POSIX timestamp in milliseconds
        (nb of milliseconds since 1970-01-01 00:00:00 UTC),
        and convert it to a datetime in local time.

    - year_month_day(target, timezone=?) -> string
    - ymd(target, timezone=?) -> string
        Extract the year, month and day of a datetime.
        If the input is a string, first parse it into datetime, and then extract the year, month and day.
        Equivalent to strftime(string, format = "%Y-%m-%d")

    - month_day(target, timezone=?) -> string
        Extract the month and day of a datetime.
        If the input is a string, first parse it into datetime, and then extract the month and day.
        Equivalent to strftime(string, format = "%m-%d")

    - month(target, timezone=?) -> string
        Extract the month of a datetime.
        If the input is a string, first parse it into datetime, and then extract the month.
        Equivalent to strftime(string, format = "%m")

    - year(target, timezone=?) -> string
        Extract the year of a datetime.
        If the input is a string, first parse it into datetime, and then extract the year.
        Equivalent to strftime(string, format = "%Y")

    - year_month(target, timezone=?) -> string
    - ym(target, timezone=?) -> string
        Extract the year and month of a datetime.
        If the input is a string, first parse it into datetime, and then extract the year and month.
        Equivalent to strftime(string, format = "%Y-%m")

## Collections (list of maps) functions

    - index_by(collection, key) -> map
        Create a map from item key to collection item.

## Map functions

    - keys(map) -> [string]
        Return a list of the map's keys.

    - values(map) -> [T]
        Return a list of the map's values.

## List aggregation functions

    - mean(numbers) -> number?
        Return the means of the given numbers.

## Fuzzy matching & information retrieval

    - fingerprint(string) -> string
        Fingerprint a string by normalizing characters, re-ordering
        and deduplicating its word tokens before re-joining them by
        spaces.

    - carry_stemmer(string) -> string
        Apply the "Carry" stemmer targeting the French language.

    - s_stemmer(string) -> string
        Apply a very simple stemmer removing common plural inflexions in
        some languages.

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

    - mime_ext(string) -> string
        Return the extension related to given mime type.

    - parse_dataurl(string) -> [string, bytes]
        Parse the given data url and return its mime type and decoded binary data.

    - parse_json(string) -> any
        Parse the given string as JSON.

    - typeof(value) -> string
        Return type of value.

## IO & path wrangling

    - abspath(string) -> string
        Return absolute & canonicalized path.

    - bytesize(integer) -> string
        Return a number of bytes in human-readable format (KB, MB, GB, etc.).

    - copy(source_path, target_path) -> string
        Copy a source to target path. Will create necessary directories
        on the way. Returns target path as a convenience.

    - ext(path) -> string?
        Return the path's extension, if any.

    - filesize(string) -> int
        Return the size of given file in bytes.

    - isfile(string) -> bool
        Return whether the given path is an existing file on disk.

    - move(source_path, target_path) -> string
        Move a source to target path. Will create necessary directories
        on the way. Returns target path as a convenience.

    - pjoin(string, *strings) -> string
    - pathjoin(string, *strings) -> string
        Join multiple paths correctly.

    - read(path, encoding=?, errors=?) -> string
        Read file at path. Default encoding is "utf-8".
        Default error handling policy is "replace", and can be
        one of "replace", "ignore" or "strict".

    - read_csv(path) -> list[map]
        Read and parse CSV file at path, returning its rows as
        a list of maps with headers as keys.

    - read_json(path) -> any
        Read and parse JSON file at path.

    - write(string, path) -> string
        Write string to path as utf-8 text. Will create necessary
        directories recursively before actually writing the file.
        Return the path that was written.

## Random

    - md5(string) -> string
        Return the md5 hash of string in hexadecimal representation.

    - random() -> float
        Return a random float between 0 and 1.

    - uuid() -> string
        Return a uuid v4.
```

## Aggregation functions

```md
# Available aggregation functions

(use --cheatsheet for a reminder of how the scripting language works)

Note that most functions ignore empty values. This said, functions working on
number will yield an error when encountering a string that cannot be safely
parsed as a suitable number.

You can always cast values around and force aggregation functions to
consider empty values or make them avoid non-numerical values altogether.

For instance, considering null values when computing a mean is as easy
as `mean(number || 0)`.

Finally, note that expressions returning lists will be understood as multiplexed rows.
This means that computing `cardinality([source, target])`, for instance, will return
the number of nodes in a graph represented by a CSV edge list.

    - all(<expr>) -> bool
        Returns true if all elements returned by given expression are truthy.

    - any(<expr>) -> bool
        Returns true if one of the elements returned by given expression is truthy.

    - approx_cardinality(<expr>) -> int
        Returns the approximate cardinality of the set of values returned by given
        expression using the HyperLogLog+ algorithm.

    - approx_quantile(<expr>, p) -> number
        Returns an approximation of the desired quantile of values returned by given
        expression using t-digests.

    - argmin(<expr>, <expr>?) -> any
        Return the index of the row where the first expression is minimized, or
        the result of the second expression where the first expression is minimized.
        Ties will be broken by original row index.

    - argmax(<expr>, <expr>?) -> any
        Return the index of the row where the first expression is maximized, or
        the result of the second expression where the first expression is maximized.
        Ties will be broken by original row index.

    - argtop(k, <expr>, <expr>?, separator?) -> string
        Find the top k values returned by the first expression and either
        return the indices of matching rows or the result of the second
        expression, joined by a pipe character ('|') or by the provided separator.
        Ties will be broken by original row index.

    - avg(<expr>) -> number
        Average of numerical values. Same as `mean`.

    - cardinality(<expr>) -> number
        Number of distinct values returned by given expression.

    - correlation(<expr>, <expr>) -> number
        Return the correlation (covariance divided by the product of standard
        deviations) of series represented by the two given expressions.

    - count(<expr>?) -> number
        Count the number of truthy values returned by given expression.
        Expression can also be omitted to count all rows.

    - count_seconds(<expr>) -> number
        Count the number of seconds between earliest and latest datetime
        returned by given expression.

    - count_hours(<expr>) -> number
        Count the number of hours between earliest and latest datetime
        returned by given expression.

    - count_days(<expr>) -> number
        Count the number of days between earliest and latest datetime
        returned by given expression.

    - count_years(<expr>) -> number
        Count the number of years between earliest and latest datetime
        returned by given expression.

    - covariance(<expr>, <expr>) -> number
        Return the population covariance of series represented by
        the two given expressions. Same as `covariance_pop`.

    - covariance_pop(<expr>, <expr>) -> number
        Return the population covariance of series represented by
        the two given expressions. Same as `covariance`.

    - covariance_sample(<expr>, <expr>) -> number
        Return the sample covariance of series represented by
        the two given expressions.

    - distinct_values(<expr>, separator?) -> string
        List of sorted distinct values joined by a pipe character ('|') by default or by
        the provided separator.

    - earliest(<expr>) -> datetime
        Earliest datetime returned by given expression.

    - first(<expr>) -> string
        Return first seen non empty element of the values returned by the given expression.

    - latest(<expr>) -> datetime
        Latest datetime returned by given expression.

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

    - most_common(k, <expr>, separator?) -> string
        List of top k most common values returned by expression
        joined by a pipe character ('|') or by the provided separator.
        Ties will be broken by lexicographical order.

    - most_common_counts(k, <expr>, separator?) -> numbers
        List of top k most common counts returned by expression
        joined by a pipe character ('|') or by the provided separator.

    - percentage(<expr>) -> number
        Return the percentage of truthy values returned by expression.

    - quantile(<expr>, p) -> number
        Return the desired quantile of numerical values.

    - q1(<expr>) -> number
        Return the first quartile of numerical values.

    - q2(<expr>) -> number
        Return the second quartile of numerical values. Alias for median.

    - q3(<expr>) -> number
        Return the third quartile of numerical values.

    - ratio(<expr>) -> number
        Return the ratio of truthy values returned by expression.

    - stddev(<expr>) -> number
        Population standard deviation. Same as `stddev_pop`.

    - stddev_pop(<expr>) -> number
        Population standard deviation. Same as `stddev`.

    - stddev_sample(<expr>) -> number
        Sample standard deviation (i.e. using Bessel's correction).

    - sum(<expr>) -> number
        Sum of numerical values. Will return nothing if the sum overflows.
        Uses the Kahan-Babuska routine for precise float summation.

    - top(k, <expr>, separator?) -> any
        Find the top k values returned by the expression and join
        them by a pipe character ('|') or by the provided separator.
        Ties will be broken by original row index.

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

