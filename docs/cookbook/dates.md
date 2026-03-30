# Dates in xan

## Summary

* [Parsing and formatting standard dates](#parsing-and-formatting-standard-iso-8601-dates)
* [Visualizing dates](#visualizing-dates)
* [Parsing non-standard dates](#parsing-non-standard-dates)
* [Dealing with timezones](#dealing-with-timezones)


## Parsing and formatting standard dates
Let's say the column `local_time` of your CSV file is containing dates in [ISO 8601](https://en.wikipedia.org/wiki/ISO_8601) format,
for example `2022-03-22`, `2022-03-22 23:20:24`, `2022-03-22T00:00:00[CET]` or `2022-03-22T23:20:24+01:00[Europe/Paris]`.

| local_time                                     |
| ---------------------------------------------- |
| 2023-02-01                                     |
| 2023-02-01T18:50:56                            |
| 2023-02-01 19:13:44                            |
| 2023-02-01T20:03:52[America/Mexico_City]       |
| 2023-02-01T21:02:27[America/Mexico_City]       |
| 2023-02-02                                     |
| 2023-02-02 07:35:37                            |
| 2023-02-02T07:40:34                            |
| 2023-02-02T10:56:22                            |
| 2023-02-02T18:47:06                            |

### xan stats
To explore temporal data, for example to find out if there are empty cells or to find the start and end dates of the corpus,
the first thing you can do is `xan stats` on the `local_time` column:

```bash
xan stats -s local_time dates.csv | xan view

```

| count | count_empty | type  | types                          | min | max | ... | lex_first  | lex_last            | ... |
| ----- | ----------- | ----- | ------------------------------ | --- | --- | --- | ---------- | ------------------- | --- |
| 92    | 0           | mixed | zoned_datetime\|datetime\|date |     |     | ... | 2023-02-01 | 2023-02-28T22:17:33 | ... |


One can have a better view of the same table when piping into `xan transpose`:


```bash
xan stats -s local_time dates.csv | xan transpose | xan view

```

| field       | local_time                     |
| ----------- | ------------------------------ |
| count       | 92                             |
| count_empty | 0                              |
| type        | mixed                          |
| types       | zoned_datetime\|datetime\|date |
| sum         | 0                              |
| mean        |                                |
| variance    |                                |
| stddev      |                                |
| min         |                                |
| max         |                                |
| lex_first   | 2023-02-01                     |
| lex_last    | 2023-02-28T22:17:33            |
| min_length  | 10                             |
| max_length  | 40                             |


Here we observe that there are no empty fields (see `count_empty`), and that the data extends from February 1st to February 28th, 2023 (see `lex_first` and `lex_last`). The `types` field also tells us that there are different types of dates in our dataset: datetimes, dates (i.e. without time information) and zoned datetimes (i.e. datetimes with a timezone indication).

### Different formats
If you want to keep only the date (without the time or the timezone) in a new column called `date`, you can apply the `date()` (or `year_month_day()`/`ymd()`) function.
Functions can be applied using the `xan map` command, to create a new column with the result of an expression,
or with the `xan transform` command, to directly transform the column.
Here is an example where we created a new column called `date` obtained from `local_time` with `xan map`:

```bash
xan map 'local_time.date() as date' dates.csv | xan view
```

| local_time                               | date       |
| ---------------------------------------- | ---------- |
| 2023-02-01                               | 2023-02-01 |
| 2023-02-01T18:50:56                      | 2023-02-01 |
| 2023-02-01                               | 2023-02-01 |
| 2023-02-01T20:03:52[America/Mexico_City] | 2023-02-01 |
| 2023-02-01T21:02:27[America/Mexico_City] | 2023-02-01 |
| 2023-02-02                               | 2023-02-02 |
| 2023-02-02                               | 2023-02-02 |
| 2023-02-02T07:40:34                      | 2023-02-02 |
| 2023-02-02T10:56:22                      | 2023-02-02 |
| 2023-02-02T18:47:06                      | 2023-02-02 |

Other formatting functions, such as `year()` or `year_month()`/`ym()` exist.
The list of all date-related functions is available
[here](https://github.com/medialab/xan/blob/master/docs/moonblade/functions.md#dates) and can also be displayed using:

```bash
xan help functions --section dates
```

## Visualizing dates

### xan freq
Now that you now how to format dates, you can use `xan freq` to count the number of lines per day in you dataset:

```bash
xan map 'local_time.date() as date' dates.csv | xan freq -s date | xan view
```

| field | value      | count |
| ----- | ---------- | ----- |
| date  | 2023-02-06 | 7     |
| date  | 2023-02-19 | 6     |
| date  | 2023-02-28 | 6     |
| date  | 2023-02-01 | 5     |
| date  | 2023-02-02 | 5     |
| date  | 2023-02-03 | 5     |
| date  | 2023-02-08 | 5     |
| date  | 2023-02-13 | 5     |
| date  | 2023-02-07 | 4     |
| date  | 2023-02-14 | 4     |
| date  | \<rest\>   | 40    |

This view is helpful (it is sorted by decreasing `count`) but in the case of dates one would prefer to have lines sorted in chronological order.

### xan hist -D
The simplest way to do this is to use `xan freq` in combination with `xan hist -D`:
`xan hist` will plot a histogram with one bar per row from `xan freq`, and the `-D` (or `--dates`)
flag will have the histogram sorted by date and add empty bars for missing days.
Don't forget to add `-A` (or `--all`) in `xan freq` in order to plot all days.

```bash
xan map 'local_time.date() as date' dates.csv | xan freq -s date -A | xan hist -D
```

This way, you immediately notice the fact that there is no line in your dataset on February 4 and 5.

```
2023-02-01 |5   5.43%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■                   |
2023-02-02 |5   5.43%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■                   |
2023-02-03 |5   5.43%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■                   |
2023-02-04 |0   0.00%|                                                                   |
2023-02-05 |0   0.00%|                                                                   |
2023-02-06 |7   7.61%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■|
2023-02-07 |4   4.35%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■                            |
2023-02-08 |5   5.43%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■                   |
2023-02-09 |2   2.17%|■■■■■■■■■■■■■■■■■■■■                                               |
2023-02-10 |3   3.26%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■                                      |
2023-02-11 |1   1.09%|■■■■■■■■■■                                                         |
2023-02-12 |3   3.26%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■                                      |
2023-02-13 |5   5.43%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■                   |
2023-02-14 |4   4.35%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■                            |
2023-02-15 |3   3.26%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■                                      |
2023-02-16 |4   4.35%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■                            |
2023-02-17 |3   3.26%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■                                      |
2023-02-18 |3   3.26%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■                                      |
2023-02-19 |6   6.52%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■         |
2023-02-20 |3   3.26%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■                                      |
2023-02-21 |1   1.09%|■■■■■■■■■■                                                         |
2023-02-22 |1   1.09%|■■■■■■■■■■                                                         |
2023-02-23 |3   3.26%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■                                      |
2023-02-24 |4   4.35%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■                            |
2023-02-25 |2   2.17%|■■■■■■■■■■■■■■■■■■■■                                               |
2023-02-26 |1   1.09%|■■■■■■■■■■                                                         |
2023-02-27 |3   3.26%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■                                      |
2023-02-28 |6   6.52%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■         |

```

`xan hist -D` will adapt the granularity automatically if you want to count the number of lines per month or per year.

## Parsing and formatting non-standard dates

### datetime()
If you have dates in non ISO 8601 format, such as `31/12/22` or `02 Jan 2006 15:04`, you can parse them using the `datetime` function.
The conversion specifications (i.e. how to tell `datetime()` the format you have in mind) are listed
[here](https://docs.rs/jiff/latest/jiff/fmt/strtime/index.html#conversion-specifications).

`datetime()` takes as first argument the name of the column containing the date expression,
and as second argument the desired format:

```bash
xan map 'initial_date.datetime("%d/%m/%y") as parsed_date' strange_dates.csv | xan v
```

| initial_date     | parsed_date         |
| ---------------- | ------------------- |
| 25/02/22         | 2022-02-25T00:00:00 |
| 25/02/22         | 2022-02-25T00:00:00 |
| 26/02/22         | 2022-02-26T00:00:00 |
| 27/02/22         | 2022-02-27T00:00:00 |
| 27/02/22         | 2022-02-27T00:00:00 |
| 28/02/22         | 2022-02-28T00:00:00 |
| 01/03/22         | 2022-03-01T00:00:00 |
| 01/03/22         | 2022-03-01T00:00:00 |

 Of course, you can combine `datetime()` with other formatting functions to print your output in the desired format.
 Below an example with `month_day()`:

```bash
xan map 'initial_date.datetime("%d/%m/%y").month_day() as formatted_date' strange_dates.csv | xan v
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
xan map 'initial_date.datetime("%d/%m/%y").strftime("%A") as day_of_week' strange_dates.csv | xan v
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
xan map 'local_time.strftime("%A") as day_of_week' dates.csv | xan v
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

## Dealing with timezones

### with_timezone() / with_tz()
Let's say you live in Mexico City and your colleague in Paris sends you a file
called `july_data.csv` containing a `local_time` column without timezones.

| local_time          |
| ------------------- |
| 2022-07-01T11:55:06 |
| 2022-07-01T15:50:02 |
| 2022-07-01T16:07:11 |
| 2022-07-01T16:07:38 |
| 2022-07-01T16:07:54 |
| 2022-07-01T16:07:58 |
| 2022-07-02T08:35:08 |
| 2022-07-02T11:20:20 |
| 2022-07-02T11:23:04 |

When using xan's `datetime()` function, the tool does not assume any timezone information.
You want to tell xan to parse dates according to the Paris time zone,
and probably rename your column to remove the `local_time` header which could be misleading.
To do this you use the `xan transform` command associated with the `with_timezone()`/`with_tz()` function:

```
xan transform local_time 'local_time.with_timezone("Europe/Paris")' --rename paris_time july_data.csv | xan v
```

| paris_time                              |
| --------------------------------------- |
| 2022-07-01T11:55:06+02:00[Europe/Paris] |
| 2022-07-01T15:50:02+02:00[Europe/Paris] |
| 2022-07-01T16:07:11+02:00[Europe/Paris] |
| 2022-07-01T16:07:38+02:00[Europe/Paris] |
| 2022-07-01T16:07:54+02:00[Europe/Paris] |
| 2022-07-01T16:07:58+02:00[Europe/Paris] |
| 2022-07-02T08:35:08+02:00[Europe/Paris] |
| 2022-07-02T11:20:20+02:00[Europe/Paris] |
| 2022-07-02T11:23:04+02:00[Europe/Paris] |

### to_local_timezone() / to_local_tz()
Or maybe you would prefer to write the dates directly in your local timezone.
In that case you need to tell xan to parse the data in Paris time and
then convert it to your computer's timezone.
This is what the `to_local_timezone()`/`to_local_tz()` function is for:

```
xan transform local_time 'local_time.with_timezone("Europe/Paris").to_local_timezone()' --rename mexico_time july_data.csv | xan v
```

| mexico_time                                    |
| ---------------------------------------------- |
| 2022-07-01T04:55:06-05:00[America/Mexico_City] |
| 2022-07-01T08:50:02-05:00[America/Mexico_City] |
| 2022-07-01T09:07:11-05:00[America/Mexico_City] |
| 2022-07-01T09:07:38-05:00[America/Mexico_City] |
| 2022-07-01T09:07:54-05:00[America/Mexico_City] |
| 2022-07-01T09:07:58-05:00[America/Mexico_City] |
| 2022-07-02T01:35:08-05:00[America/Mexico_City] |
| 2022-07-02T04:20:20-05:00[America/Mexico_City] |
| 2022-07-02T04:23:04-05:00[America/Mexico_City] |

### to_timezone() / to_tz()
If you prefer to convert dates to a time zone other than your computer's (UTC, maybe?),
you can use the `to_timezone()`/`to_tz()`  function

```
xan transform local_time 'local_time.with_timezone("Europe/Paris").to_timezone("UTC")' --rename utc_time july_data.csv | xan v
```

| utc_time                       |
| ------------------------------ |
| 2022-07-01T09:55:06+00:00[UTC] |
| 2022-07-01T13:50:02+00:00[UTC] |
| 2022-07-01T14:07:11+00:00[UTC] |
| 2022-07-01T14:07:38+00:00[UTC] |
| 2022-07-01T14:07:54+00:00[UTC] |
| 2022-07-01T14:07:58+00:00[UTC] |
| 2022-07-02T06:35:08+00:00[UTC] |
| 2022-07-02T09:20:20+00:00[UTC] |
| 2022-07-02T09:23:04+00:00[UTC] |

### without_timezone() / without_tz()

If you want to discard the timezone information, it is always possible to use `without_timezone()`/`without_tz()`:
```
xan transform local_time 'local_time.with_timezone("Europe/Paris").to_timezone("UTC").without_tz()' --rename utc_time july_data.csv | xan v
```

| utc_time            |
| ------------------- |
| 2022-07-01T09:55:06 |
| 2022-07-01T13:50:02 |
| 2022-07-01T14:07:11 |
| 2022-07-01T14:07:38 |
| 2022-07-01T14:07:54 |
| 2022-07-01T14:07:58 |
| 2022-07-02T06:35:08 |
| 2022-07-02T09:20:20 |
| 2022-07-02T09:23:04 |

## Dealing with timestamps

### to_timestamp() and to_timestamp_ms()

The `to_timestamp()` and `to_timestamp_ms()` functions, are used to convert dates to Unix timestamps.
Since timestamps are always in UTC, `to_timestamp()` and `to_timestamp_ms()` can only read dates containing a timezone information. In the file your colleague sent from Paris, dates should therefore be parsed with
`with_timezone("Europe/Paris")` before using `to_timestamp()`:

```
xan transform local_time 'local_time.with_timezone("Europe/Paris").to_timestamp()' --rename timestamp_utc
july_data.csv | xan v
```

| timestamp_utc |
| ------------- |
| 1656669306    |
| 1656683402    |
| 1656684431    |
| 1656684458    |
| 1656684474    |
| 1656684478    |
| 1656743708    |
| 1656753620    |
| 1656753784    |

### from_timestamp() and from_timestamp_ms()

 Conversely, dates obtained by parsing a timestamp with `from_timestamp()` are always presented in UTC. If the file your colleague sent contained Unix timestamps instead of dates, you could convert them like this:

```
xan map 'timestamp_utc.from_timestamp() as UTC_time' july_data.csv | xan v
```

| timestamp_utc | UTC_time                       |
| ------------- | ------------------------------ |
| 1656669306    | 2022-07-01T09:55:06+00:00[UTC] |
| 1656683402    | 2022-07-01T13:50:02+00:00[UTC] |
| 1656684431    | 2022-07-01T14:07:11+00:00[UTC] |
| 1656684458    | 2022-07-01T14:07:38+00:00[UTC] |
| 1656684474    | 2022-07-01T14:07:54+00:00[UTC] |
| 1656684478    | 2022-07-01T14:07:58+00:00[UTC] |
| 1656743708    | 2022-07-02T06:35:08+00:00[UTC] |
| 1656753620    | 2022-07-02T09:20:20+00:00[UTC] |
| 1656753784    | 2022-07-02T09:23:04+00:00[UTC] |
