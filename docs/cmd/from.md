<!-- Generated -->
# xan from

```txt
Convert a variety of data formats to CSV.

Usage:
    xan from [options] [<input>]
    xan from --help

Supported formats:
    ods      - OpenOffice spreadsheet
    xls      - Excel spreasheet
    xlsb     - Excel spreasheet
    xlsx     - Excel spreasheet
    json     - JSON array or object
    ndjson   - Newline-delimited JSON
    jsonl    - Newline-delimited JSON
    txt      - text lines
    npy      - Numpy array
    tar      - Tarball archive
    md       - Markdown table
    markdown - Markdown table

Some formats can be streamed, some others require the full file to be loaded into
memory. The streamable formats are `ndjson`, `jsonl`, `tar`, `txt` and `npy`.

Some formats will handle gzip decompression on the fly if the filename ends
in `.gz`: `json`, `ndjson`, `jsonl` and `txt`.

Tarball extraction was designed for utf8-encoded text files. Expect weird or
broken results with other encodings or binary files.

from options:
    -f, --format <format>  Format to convert from. Will be inferred from file
                           extension if not given. Must be specified when reading
                           from stdin, since we don't have a file extension to
                           work with.

Excel/OpenOffice-related options:
    -s, --sheet <name>     Name of the sheet to convert. [default: Sheet1]

JSON options:
    --sample-size <n>      Number of records to sample before emitting headers.
                           [default: 64]
    --key-column <name>    Name for the key column when parsing a JSON map.
                           [default: key]
    --value-column <name>  Name for the value column when parsing a JSON map.
                           [default: value]

Text lines options:
    -c, --column <name>    Name of the column to create.
                           [default: value]

Markdown options:
    -n, --nth-table <n>    Select nth table in document, starting at 0.
                           Negative index can be used to select from the end.
                           [default: 0]

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
```
