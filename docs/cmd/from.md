<!-- Generated -->
# xan from

```txt
Convert a variety of data formats to CSV.

Usage:
    xan from [options] [<input>]
    xan from --help

Supported formats:
    ods   - OpenOffice spreadsheet
    xls   - Excel spreasheet
    xlsb  - Excel spreasheet
    xlsx  - Excel spreasheet

    json    - JSON array or object
    ndjson  - Newline-delimited JSON
    jsonl   - Newline-delimited JSON

    txt - text lines

    npy - Numpy array

Some formats can be streamed, some others require the full file to be loaded into
memory. The streamable formats are `ndjson`, `jsonl`, `txt` and `npy`.

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

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
```
