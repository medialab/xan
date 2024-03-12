# xan behead

The `behead` command simply removes the header row from target CSV file.

This is mostly useful to conform with other tools that don't expect header rows or to easily pipe the result into other unix commands expecting simple lines of data.

This means this file:

*people.csv*

<table>
  <tr>
    <th>Name</th>
    <th>Surname</th>
  </tr>
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

Will become:

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

If you run:

```bash
xan behead people.csv
```