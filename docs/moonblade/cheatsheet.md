# Expression language cheatsheet

Use `xan help functions` for a comprehensive list of available
functions & operators.

Use `xan help aggs` for a comprehensive list of available
aggregation functions.

```
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
    'name.trim()'

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
```

Note that constant expressions will never be evaluated more than once
when parsing the program.

This means that when evaluating the following:
    'get(read_json("config.json"), name)'

The "config.json" file will never be read/parsed more than once and will not
be read/parsed once per row.
