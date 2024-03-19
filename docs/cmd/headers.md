# xan headers

The `headers` command displays the column names of given CSV file.

So, given this particular file:

*people.csv*

| name      | surname |
| --------- | ------- |
| John      | Black   |
| Lucy      | Red     |
| Guillaume | Orange  |

The following command:

```bash
xan headers people.csv
```

Will return:

```txt
0 name
1 surname
```

Note that the command will highlight in red duplicated column names to help you spot them better as they can be cumbersome sometimes.

## Ignoring column indices

Using the `-j/--just-names` you can avoid printing the column indices:

```bash
xan headers -j people.csv
```

Will return:

```txt
name
surname
```