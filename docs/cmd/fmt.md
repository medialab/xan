<!-- Generated -->
# xan fmt

```txt
Formats CSV data with a custom delimiter or CRLF line endings.

Generally, all commands in xan output CSV data in a default format, which is
the same as the default format for reading CSV data. This makes it easy to
pipe multiple xan commands together. However, you may want the final result to
have a specific delimiter or record separator, and this is where 'xan fmt' is
useful.

Usage:
    xan fmt [options] [<input>]

fmt options:
    -t, --out-delimiter <arg>  The field delimiter for writing CSV data.
                               [default: ,]
    --crlf                     Use '\r\n' line endings in the output.
    --ascii                    Use ASCII field and record separators.
    --quote <arg>              The quote character to use. [default: "]
    --quote-always             Put quotes around every value.
    --quote-never              Never put quotes around values, even if this would
                               produce invalid CSV data.
    --escape <arg>             The escape character to use. When not specified,
                               quotes are escaped by doubling them.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
