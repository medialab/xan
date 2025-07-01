<!-- Generated -->
# xan unpivot

```txt
Unpivot a CSV file by allowing multiple columns to be stacked into fewer columns.

For instance, given the following file:

dept,jan,feb,mar
electronics,1,2,3
clothes,10,20,30
cars,100,200,300

The following command:

    $ xan pivot jan: month sales file.csv

Will produce the following result:

dept,month,sales
electronics,jan,1
electronics,feb,2
electronics,mar,3
clothes,jan,10
clothes,feb,20
clothes,mar,30
cars,jan,100
cars,feb,200
cars,mar,300

Usage:
    xan unpivot [options] <columns> <name> <value> [<input>]
    xan unpivot --help

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
