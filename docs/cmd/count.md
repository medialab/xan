# xan count

The `count` command returns the number of rows of a CSV file.

It will not include the header row in the returned count, unless you provide the `-n/--no-headers` flag.

So, given this particular file:

*people.csv*

| name      | surname |
| --------- | ------- |
| John      | Black   |
| Lucy      | Red     |
| Guillaume | Orange  |

The following command:

```bash
xan count people.csv
```

Will return `3`.

## Counting CSV files having no headers

Given this file:

*people.csv*

<table>
  <tr>
    <td>John</td>
    <td>Black</td>
  </tr>
  <tr>
    <td>Lucy</td>
    <td>Red</td>
  </tr>
  <tr>
    <td>Guillaume</td>
    <td>Orange</td>
  </tr>
</table>

The following command:

```bash
xan count -n people.csv
```

Will return `3`.

Note that this is not always identical to the simpler:

```bash
wc -l people.csv
```

because the `xan count` command is of course CSV-aware and will be able to tolerate properly escaped newlines within cell values.
