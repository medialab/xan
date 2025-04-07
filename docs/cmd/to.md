<!-- Generated -->
# xan to

```txt
Convert a CSV file to a variety of data formats.

Usage:
    xan to <format> [options] [<input>]
    xan to --help

Supported formats:
    html    - HTML table
    json    - JSON array or object
    jsonl   - JSON lines (same as `ndjson`)
    md      - Markdown table
    ndjson  - Newline-delimited JSON (same as `jsonl`)
    npy     - Numpy array
    txt     - Text lines
    xlsx    - Excel spreasheet

Some formats can be streamed, some others require the full CSV file to be loaded into
memory.

Streamable formats are `html`, `jsonl`, `ndjson` and `txt`.

JSON options:
    -B, --buffer-size <size>  Number of CSV rows to sample to infer column types.
                              [default: 512]
    --nulls                   Convert empty string to a null value.
    --omit                    Ignore the empty values.

NPY options:
    --dtype <type>  Number type to use for the npy conversion. Must be one of "f32"
                    or "f64". [default: f64]

TXT options:
    -s, --select <column>  Column of file to emit as text. Will error if file
                           to convert to text has multiple columns or if
                           selection yields more than a single column.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be evaled
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
