# Available functions & operators

## Summary

- [Operators](#operators)
    - [Unary operators](#unary-operators)
    - [Numerical comparison](#numerical-comparison)
    - [String/sequence comparison](#stringsequence-comparison)
    - [Arithmetic operators](#arithmetic-operators)
    - [String/sequence operators](#stringsequence-operators)
    - [Logical operators](#logical-operators)
    - [Indexing & slicing operators](#indexing--slicing-operators)
    - [Pipeline operator](#pipeline-operator)
- [Arithmetics](#arithmetics)
- [Boolean operations & branching](#boolean-operations--branching)
- [Comparison](#comparison)
- [String & sequence helpers](#string--sequence-helpers)
- [Dates](#dates)
- [Higher-order functions](#higherorder-functions)
- [Urls & web-related](#urls--webrelated)
- [Collections (list of maps) functions](#collections-list-of-maps-functions)
- [Map functions](#map-functions)
- [Aggregation functions](#aggregation-functions)
- [Fuzzy matching & information retrieval](#fuzzy-matching--information-retrieval)
- [Utils](#utils)
- [IO & path wrangling](#io--path-wrangling)
- [Random](#random)

## Operators

### Unary operators

```txt
!x - boolean negation
-x - numerical negation
```

### Numerical comparison

Warning: those operators will always consider operands as numbers or dates and will try to cast them around as such. For string/sequence comparison, use the operators in the next section.

```txt
x == y - numerical equality
x != y - numerical inequality
x < y  - numerical less than
x <= y - numerical less than or equal
x > y  - numerical greater than
x >= y - numerical greater than or equal
```

### String/sequence comparison

Warning: those operators will always consider operands as strings or sequences and will try to cast them around as such. For numerical comparison, use the operators in the previous section.

```txt
x eq y - string equality
x ne y - string inequality
x lt y - string less than
x le y - string less than or equal
x gt y - string greater than
x ge y - string greater than or equal
```

### Arithmetic operators

```txt
x + y  - numerical addition
x - y  - numerical subtraction
x * y  - numerical multiplication
x / y  - numerical division
x % y  - numerical remainder
x // y - numerical integer division
x ** y - numerical exponentiation
```

### String/sequence operators

```txt
x ++ y - string concatenation
```

### Logical operators

```txt
x && y     - logical and
x and y
x || y     - logical or
x or y
x in y
x not in y
```

### Indexing & slicing operators

Negative indices are accepted and mean the same thing as with the Python language.

```txt
x[y]         - get y from x (string or list index, map key)
x[start:end] - slice x from start index to end index
x[:end]      - slice x from start to end index
x[start:]    - slice x from start index to end
```

### Pipeline operator

using "_" for left-hand side substitution.

```txt
trim(name) | len(_)         - Same as len(trim(name))
trim(name) | len            - Supports elision for unary functions
trim(name) | add(1, len(_)) - Can be nested
add(trim(name) | len, 2)    - Can be used anywhere
```


## Arithmetics

- **abs**(*x*) -> `number`: Return absolute value of number.
- **add**(*x*, *y*, *\*n*) -> `number`: Add two or more numbers.
- **argmax**(*numbers*, *labels?*) -> `any`: Return the index or label of the largest number in the list.
- **argmin**(*numbers*, *labels?*) -> `any`: Return the index or label of the smallest number in the list.
- **ceil**(*x*) -> `number`: Return the smallest integer greater than or equal to x.
- **div**(*x*, *y*, *\*n*) -> `number`: Divide two or more numbers.
- **idiv**(*x*, *y*) -> `number`: Integer division of two numbers.
- **floor**(*x*) -> `number`: Return the smallest integer lower than or equal to x.
- **log**(*x*) -> `number`: Return the natural logarithm of x.
- **log10**(*x*) -> `number`: Return the base 10 logarithm of x.
- **max**(*x*, *y*, *\*n*) -> `number`: Return the maximum number.
- **max**(*list_of_numbers*) -> `number`: Return the maximum number.
- **min**(*x*, *y*, *\*n*) -> `number`: Return the minimum number.
- **min**(*list_of_numbers*) -> `number`: Return the minimum number.
- **mod**(*x*, *y*) -> `number`: Return the remainder of x divided by y.
- **mul**(*x*, *y*, *\*n*) -> `number`: Multiply two or more numbers.
- **neg**(*x*) -> `number`: Return -x.
- **pow**(*x*, *y*) -> `number`: Raise x to the power of y.
- **round**(*x*) -> `number`: Return x rounded to the nearest integer.
- **sqrt**(*x*) -> `number`: Return the square root of x.
- **sub**(*x*, *y*, *\*n*) -> `number`: Subtract two or more numbers.
- **trunc**(*x*) -> `number`: Truncate the number by removing its decimal part.

## Boolean operations & branching

- **and**(*a*, *b*, *\*n*) -> `T`: Perform boolean AND operation on two or more values.
- **if**(*cond*, *then*, *else?*) -> `T`: Evaluate condition and switch to correct branch.
- **unless**(*cond*, *then*, *else?*) -> `T`: Shorthand for `if(not(cond), then, else?)`
- **not**(*a*) -> `bool`: Perform boolean NOT operation.
- **or**(*a*, *b*, *\*n*) -> `T`: Perform boolean OR operation on two or more values.

## Comparison

- **eq**(*s1*, *s2*) -> `bool`: Test string or sequence equality.
- **ne**(*s1*, *s2*) -> `bool`: Test string or sequence inequality.
- **gt**(*s1*, *s2*) -> `bool`: Test string or sequence s1 > s2.
- **ge**(*s1*, *s2*) -> `bool`: Test string or sequence s1 >= s2.
- **lt**(*s1*, *s2*) -> `bool`: Test string or sequence s1 < s2.
- **le**(*s1*, *s2*) -> `bool`: Test string or sequence s1 <= s2.

## String & sequence helpers

- **compact**(*list*) -> `list`: Drop all falsey values from given list.
- **concat**(*string*, *\*strings*) -> `string`: Concatenate given strings into a single one.
- **contains**(*seq*, *subseq*) -> `bool`: Find if subseq can be found in seq. Subseq can be a regular expression.
- **count**(*seq*, *pattern*) -> `int`: Count number of times pattern appear in seq. Pattern can be a regular expression.
- **endswith**(*string*, *pattern*) -> `bool`: Test if string ends with pattern.
- **escape_regex**(*string*) -> `string`: Escape a string so it can be used safely in a regular expression.
- **first**(*seq*) -> `T`: Get first element of sequence.
- **fmt**(*string*, *\*replacements*) -> `string`: Format a string by replacing "{}" occurrences by subsequent arguments.<br>Example: `fmt("Hello {} {}", name, surname)` will replace the first "{}" by the value of the name column, then the second one by the value of the surname column.<br>Can also be given a substitution map like so:<br>`fmt("Hello {name}", {name: "John"})`.
- **fmt**(*string*, *map*) -> `string`: Format a string by replacing "{}" occurrences by subsequent arguments.<br>Example: `fmt("Hello {} {}", name, surname)` will replace the first "{}" by the value of the name column, then the second one by the value of the surname column.<br>Can also be given a substitution map like so:<br>`fmt("Hello {name}", {name: "John"})`.
- **get**(*target*, *index_or_key*, *default?*) -> `T`: Get nth element of sequence (can use negative indexing), or key of mapping. Returns nothing if index or key is not found or alternatively the provided default value.
- **join**(*seq*, *sep*) -> `string`: Join sequence by separator.
- **last**(*seq*) -> `T`: Get last element of sequence.
- **len**(*seq*) -> `int`: Get length of sequence.
- **ltrim**(*string*, *pattern?*) -> `string`: Trim string of leading whitespace or provided characters.
- **lower**(*string*) -> `string`: Lowercase string.
- **match**(*string*, *pattern*, *group*) -> `string`: Return a regex pattern match on the string.
- **numfmt**(*number*) -> `string`: Format a number with thousands separator and proper significance.
- **replace**(*string*, *pattern*, *replacement*) -> `string`: Replace pattern in string. Can use a regex.
- **rtrim**(*string*, *pattern?*) -> `string`: Trim string of trailing whitespace or provided characters.
- **slice**(*seq*, *start*, *end?*) -> `seq`: Return slice of sequence.
- **split**(*string*, *sep*, *max?*) -> `list`: Split a string by separator.
- **startswith**(*string*, *pattern*) -> `bool`: Test if string starts with pattern.
- **trim**(*string*, *pattern?*) -> `string`: Trim string of leading & trailing whitespace or provided characters.
- **upper**(*string*) -> `string`: Uppercase string.

## Dates

- **datetime**(*string*, *format=?*, *timezone=?*) -> `datetime`: Parse a string as a datetime according to format and timezone. If no format is provided, string is parsed as ISO 8601 date format. Default timezone is the system timezone.<br>https://docs.rs/jiff/latest/jiff/fmt/strtime/index.html#conversion-specifications
- **strftime**(*target*, *format*) -> `string`: Format target (a time in ISO 8601 format, or the result of datetime() function) according to format.
- **timestamp**(*number*) -> `datetime`: Parse a number as a POSIX timestamp in seconds (nb of seconds since 1970-01-01 00:00:00 UTC), and convert it to a datetime in local time.
- **timestamp_ms**(*number*) -> `datetime`: Parse a number as a POSIX timestamp in milliseconds (nb of milliseconds since 1970-01-01 00:00:00 UTC), and convert it to a datetime in local time.
- **to_timezone**(*target*, *timezone_in*, *timezone_out*) -> `datetime`: Parse target (a time in ISO 8601 format, or the result of datetime() function) in timezone_in, and convert it to timezone_out.
- **to_local_timezone**(*target*) -> `datetime`: Parse target (a time in ISO 8601 format, or the result of datetime() function) in timezone_in, and convert it to the system's local timezone.
- **year_month_day**(*target*) -> `string` (aliases: **ymd**): Extract the year, month and day of a datetime. If the input is a string, first parse it into datetime, and then extract the year, month and day.<br>Equivalent to `strftime(string, format="%Y-%m-%d")`.
- **month_day**(*target*) -> `string`: Extract the month and day of a datetime. If the input is a string, first parse it into datetime, and then extract the month and day.<br>Equivalent to `strftime(string, format="%m-%d")`.
- **month**(*target*) -> `string`: Extract the month of a datetime. If the input is a string, first parse it into datetime, and then extract the month.<br>Equivalent to `strftime(string, format="%m")`.
- **year**(*target*) -> `string`: Extract the year of a datetime. If the input is a string, first parse it into datetime, and then extract the year.<br>Equivalent to `strftime(string, format="%Y")`.
- **year_month**(*target*) -> `string` (aliases: **ym**): Extract the year and month of a datetime. If the input is a string, first parse it into datetime, and then extract the year and month.<br>Equivalent to `strftime(string, format="%Y-%m")`.

## Higher-order functions

- **filter**(*list*, *lambda*) -> `list`: Return a list containing only elements for which given lambda returned true.
- **map**(*list*, *lambda*) -> `list`: Return a list with elements transformed by given lambda.

## Urls & web-related

- **html_unescape**(*string*) -> `string`: Unescape given HTML string by converting HTML entities back to normal text.
- **lru**(*string*) -> `string`: Convert the given URL to LRU format.<br>For more info, read this: https://github.com/medialab/ural#about-lrus
- **parse_dataurl**(*string*) -> `[string, bytes]`: Parse the given data url and return its mime type and decoded binary data.
- **urljoin**(*string*, *string*) -> `string`: Join an url with the given addendum.

## Collections (list of maps) functions

- **index_by**(*collection*, *key*) -> `map`: Create a map from item key to collection item.

## Map functions

- **keys**(*map*) -> `[string]`: Return a list of the map's keys.
- **values**(*map*) -> `[T]`: Return a list of the map's values.

## Aggregation functions

- **mean**(*numbers*) -> `number?`: Return the mean of the given numbers.
- **sum**(*numbers*) -> `number?`: Return the sum of the given numbers, or nothing if the sum overflowed.

## Fuzzy matching & information retrieval

- **fingerprint**(*string*) -> `string`: Fingerprint a string by normalizing characters, re-ordering and deduplicating its word tokens before re-joining them by spaces.
- **carry_stemmer**(*string*) -> `string`: Apply the "Carry" stemmer targeting the French language.
- **s_stemmer**(*string*) -> `string`: Apply a very simple stemmer removing common plural inflexions in some languages.
- **unidecode**(*string*) -> `string`: Convert string to ascii as well as possible.

## Utils

- **coalesce**(*\*args*) -> `T`: Return first truthy value.
- **col**(*name_or_pos*, *nth?*) -> `bytes`: Return value of cell for given column, by name, by position or by name & nth, in case of duplicate header names.
- **cols**(*from_name_or_pos?*, *to_name_or_pos?*) -> `list[bytes]`: Return list of cell values from the given colum by name or position to another given column by name or position, inclusive. Can also be called with a single argument to take a slice from the given column to the end, or no argument at all to take all columns.
- **err**(*msg*) -> `error`: Make the expression return a custom error.
- **float**(*any*) -> `float`: Cast value as float and raise an error if impossible.
- **headers**(*from_name_or_pos?*, *to_name_or_pos?*) -> `list[string]`: Return list of header names from the given colum by name or position to another given column by name or position, inclusive. Can also be called with a single argument to take a slice from the given column to the end, or no argument at all to return all headers.
- **index**() -> `int?`: Return the row's index, if applicable.
- **int**(*any*) -> `int`: Cast value as int and raise an error if impossible.
- **mime_ext**(*string*) -> `string`: Return the extension related to given mime type.
- **parse_json**(*string*) -> `any`: Parse the given string as JSON.
- **try**(*T*) -> `T`: Attempt to evaluate given expression and return null if it raised an error.
- **typeof**(*value*) -> `string`: Return type of value.

## IO & path wrangling

- **abspath**(*string*) -> `string`: Return absolute & canonicalized path.
- **bytesize**(*string*) -> `string`: Return a number of bytes in human-readable format (KB, MB, GB, etc.).
- **copy**(*source_path*, *target_path*) -> `string`: Copy a source to target path. Will create necessary directories on the way. Returns target path as a convenience.
- **ext**(*path*) -> `string?`: Return the path's extension, if any.
- **filesize**(*string*) -> `int`: Return the size of given file in bytes.
- **isfile**(*string*) -> `bool`: Return whether the given path is an existing file on disk.
- **move**(*source_path*, *target_path*) -> `string`: Move a source to target path. Will create necessary directories on the way. Returns target path as a convenience.
- **pathjoin**(*string*, *\*strings*) -> `string` (aliases: **pjoin**): Join multiple paths correctly.
- **read**(*path*, *encoding=?*, *errors=?*) -> `string`: Read file at path. Default encoding is "utf-8". Default error handling policy is "replace", and can be one of "replace", "ignore" or "strict".
- **read_csv**(*path*) -> `list[map]`: Read and parse CSV file at path, returning its rows as a list of maps with headers as keys.
- **read_json**(*path*) -> `any`: Read and parse JSON file at path.
- **write**(*string*, *path*) -> `string`: Write string to path as utf-8 text. Will create necessary directories recursively before actually writing the file. Return the path that was written.

## Random

- **md5**(*string*) -> `string`: Return the md5 hash of string in hexadecimal representation.
- **random**() -> `float`: Return a random float between 0 and 1.
- **uuid**() -> `string`: Return a uuid v4.

