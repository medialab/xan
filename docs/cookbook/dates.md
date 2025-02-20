# Dates in xan

## Summary

* [Parsing and formatting standard dates](#parsing-and-formatting-standard-iso-8601-dates)
* [Visualizing dates](#visualizing-dates)
* [Parsing non-standard dates](#parsing-non-standard-dates)


## Parsing and formatting standard (ISO 8601) dates
Let's say the column `local_time` of your CSV file is containing dates in [ISO 8601 format](https://en.wikipedia.org/wiki/ISO_8601), for example `2022-03-22`, `2022-03-22 23:20:24`, `2022-03-22T00:00:00[CET]` or `2022-03-22T23:20:24+01:00[Europe/Paris]`.

### xan stats
To explore temporal data, for example to find out if there are empty cells or to find the start and end dates of the corpus, the first thing you can do is `xan stats` on the `local_time` column:

```bash
xan stats -s local_time dates.csv | xan view

```

| count | count_empty | type | min | max | ... |lex_first           | lex_last            | ... |
| ----- | ----------- | ---- | --- | --- | --- |------------------- | ------------------- | --- |
| 92    | 0           | date |     |     | ... |2022-02-25T17:09:47 | 2022-03-27T12:56:25 | ... |

One can have a better view of the same table with `xan transpose`:


```bash
xan stats -s local_time dates.csv | xan transpose | xan view

```

| field       | local_time          |
| ----------- | ------------------- |
| count       | 92                  |
| count_empty | 0                   |
| type        | date                |
| types       | date                |
| sum         | 0                   |
| mean        |                     |
| variance    |                     |
| stddev      |                     |
| min         |                     |
| max         |                     |
| lex_first   | 2022-02-25T17:09:47 |
| lex_last    | 2022-03-27T12:56:25 |
| min_length  | 19                  |
| max_length  | 19                  |

Here we observe that there are no empty fields (see `count_empty`), and that the data extends from February 25 to March 3, 2022 (see `lex_first` and `lex_last`).

### Different formats
If you want to keep only the year and month in a new column called `formatted_time`, you can apply the `year_month` (or `ym`) function.
Functions can be applied using the `xan map` command, to create a new column with the result of an expression,
or with the `xan transform` command, to directly transform the column. Here is an example with `xan map`:

```bash
xan map 'ym(local_time)' formatted_time dates.csv | xan view
```

| local_time          | formatted_time |
| ------------------- | -------------- |
| 2022-02-25T17:09:47 | 2022-02        |
| 2022-02-25T17:33:28 | 2022-02        |
| 2022-02-26T09:18:05 | 2022-02        |
| 2022-02-27T07:23:00 | 2022-02        |
| 2022-02-27T09:07:17 | 2022-02        |
| 2022-02-28T09:45:15 | 2022-02        |
| 2022-03-01T14:39:03 | 2022-03        |
| 2022-03-01T17:42:13 | 2022-03        |

Other formatting functions, such as `year()` or `year_month_day()` exist.
The list of all date-related functions is available
[here](https://github.com/medialab/xan/blob/master/docs/moonblade.md#functions--operators) and can also be displayed using:

```bash
xan map --functions | grep Dates -A 50
```

## Visualizing dates

### xan freq
Now that you now how to format dates, you can use `xan freq` to count the number of lines per day in you dataset:

```bash
xan map 'ymd(local_time)' year_month_day dates.csv | xan freq -s year_month_day | xan view
```
| field          | value      | count |
| -------------- | ---------- | ----- |
| year_month_day | 2022-03-23 | 9     |
| year_month_day | 2022-03-24 | 8     |
| year_month_day | 2022-03-12 | 7     |
| year_month_day | 2022-03-25 | 7     |
| year_month_day | 2022-03-26 | 5     |
| year_month_day | 2022-03-02 | 4     |
| year_month_day | 2022-03-03 | 4     |
| year_month_day | 2022-03-07 | 4     |
| year_month_day | 2022-03-10 | 4     |
| year_month_day | 2022-03-11 | 4     |
| year_month_day | <rest>     | 36    |

This view is helpful (it is sorted by decreasing `count`) but in the case of dates one would prefer to have lines sorted in chronological order.

### xan hist -D
The simplest way to do this is to use `xan hist -D`.
`xan hist` will plot a histogram, and the `-D` (or `--dates`) flag will have the histogram sorted by date and add empty bars for missing days (or months, or years).
Don't forget to add `-A` (or `--all`) in `xan freq` in order to plot all days.

```bash
xan map 'ymd(local_time)' year_month_day dates.csv | xan freq -s year_month_day -l 0 | xan hist -D
```

This way, you immediatly notice the fact that there is no line in your dataset on March 18 and 19.

```
2022-02-25 |2   2.17%|■■■■■■■■■                            |
2022-02-26 |1   1.09%|■■■■■                                |
2022-02-27 |2   2.17%|■■■■■■■■■                            |
2022-02-28 |1   1.09%|■■■■■                                |
2022-03-01 |2   2.17%|■■■■■■■■■                            |
2022-03-02 |4   4.35%|■■■■■■■■■■■■■■■■■                    |
2022-03-03 |4   4.35%|■■■■■■■■■■■■■■■■■                    |
2022-03-04 |1   1.09%|■■■■■                                |
2022-03-05 |1   1.09%|■■■■■                                |
2022-03-06 |1   1.09%|■■■■■                                |
2022-03-07 |4   4.35%|■■■■■■■■■■■■■■■■■                    |
2022-03-08 |2   2.17%|■■■■■■■■■                            |
2022-03-09 |3   3.26%|■■■■■■■■■■■■■                        |
2022-03-10 |4   4.35%|■■■■■■■■■■■■■■■■■                    |
2022-03-11 |4   4.35%|■■■■■■■■■■■■■■■■■                    |
2022-03-12 |7   7.61%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■        |
2022-03-13 |1   1.09%|■■■■■                                |
2022-03-14 |2   2.17%|■■■■■■■■■                            |
2022-03-15 |1   1.09%|■■■■■                                |
2022-03-16 |3   3.26%|■■■■■■■■■■■■■                        |
2022-03-17 |4   4.35%|■■■■■■■■■■■■■■■■■                    |
2022-03-18 |0   0.00%|                                     |
2022-03-19 |0   0.00%|                                     |
2022-03-20 |1   1.09%|■■■■■                                |
2022-03-21 |4   4.35%|■■■■■■■■■■■■■■■■■                    |
2022-03-22 |1   1.09%|■■■■■                                |
2022-03-23 |9   9.78%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■|
2022-03-24 |8   8.70%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■    |
2022-03-25 |7   7.61%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■        |
2022-03-26 |5   5.43%|■■■■■■■■■■■■■■■■■■■■■                |
2022-03-27 |3   3.26%|■■■■■■■■■■■■■                        |

```

## Parsing and formatting non-standard dates

### datetime()
If you have dates in non ISO 8601 format, such as `31/12/22` or `02 Jan 2006 15:04`, you can parse them using the `datetime` function.
The conversion specifications (i.e. how to tell `datetime()` the format you have in mind) are listed
[here](https://docs.rs/jiff/latest/jiff/fmt/strtime/index.html#conversion-specifications).

`datetime()` takes as first argument the name of the column containing the date expression, and as second argument the desired format (we will see the third argument in the [Dealing with timezones](#dealing-with-timezones) section).

```bash
xan map 'datetime(initial_date, "%d/%m/%y")' parsed_date strange_dates.csv | xan v
```

| initial_date     | parsed_date              |
| ---------------- | ------------------------ |
| 25/02/22         | 2022-02-25T00:00:00[CET] |
| 25/02/22         | 2022-02-25T00:00:00[CET] |
| 26/02/22         | 2022-02-26T00:00:00[CET] |
| 27/02/22         | 2022-02-27T00:00:00[CET] |
| 27/02/22         | 2022-02-27T00:00:00[CET] |
| 28/02/22         | 2022-02-28T00:00:00[CET] |
| 01/03/22         | 2022-03-01T00:00:00[CET] |
| 01/03/22         | 2022-03-01T00:00:00[CET] |

 Of course, you can combine `datetime()` with other formatting functions to print your output in the desired format.
 Below an example with `month_day()`:

```bash
xan map 'datetime(initial_date, "%d/%m/%y") | month_day(_)' formatted_date strange_dates.csv | xan v
```

| initial_date | formatted_date |
| ------------ | -------------- |
| 25/02/22     | 02-25          |
| 25/02/22     | 02-25          |
| 26/02/22     | 02-26          |
| 27/02/22     | 02-27          |
| 27/02/22     | 02-27          |
| 28/02/22     | 02-28          |
| 01/03/22     | 03-01          |
| 01/03/22     | 03-01          |

### strftime()

You can also specify your custom output format using the `strftime()` formatting function:

```bash
xan map 'datetime(initial_date, "%d/%m/%y") | strftime(_, "%A")' day_of_week strange_dates.csv | xan v
```

| initial_date | day_of_week |
| ------------ | ----------- |
| 25/02/22     | Friday      |
| 25/02/22     | Friday      |
| 26/02/22     | Saturday    |
| 27/02/22     | Sunday      |
| 27/02/22     | Sunday      |
| 28/02/22     | Monday      |
| 01/03/22     | Tuesday     |
| 01/03/22     | Tuesday     |

`strftime()` can also be used without `datetime()` if the date expression is in [ISO 8601 format](https://en.wikipedia.org/wiki/ISO_8601):

```bash
xan map 'strftime(local_time, "%A")' day_of_week dates.csv | xan v
```

| local_time          | day_of_week |
| ------------------- | ----------- |
| 2022-02-25T17:09:47 | Friday      |
| 2022-02-25T17:33:28 | Friday      |
| 2022-02-26T09:18:05 | Saturday    |
| 2022-02-27T07:23:00 | Sunday      |
| 2022-02-27T09:07:17 | Sunday      |
| 2022-02-28T09:45:15 | Monday      |
| 2022-03-01T14:39:03 | Tuesday     |
| 2022-03-01T17:42:13 | Tuesday     |
