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
    xlsx    - Excel spreasheet

Some formats can be streamed, some others require the full CSV file to be loaded into
memory. The streamable formats are `html`, `jsonl` and `ndjson`.

JSON options:
    -B, --buffer-size <size>  Number of CSV rows to sample to infer column types.
                              [default: 512]
    --nulls                   Convert empty string to a null value.
    --omit                    Ignore the empty values.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
```
