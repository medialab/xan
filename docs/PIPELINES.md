# `xan` pipelines

Curated collection of unhinged `xan` pipelines.

## Summary

* [Paginating urls to download](#paginating-urls-to-download)
* [Making sure a crawler was logged in by reading files in parallel](#making-sure-a-crawler-was-logged-in-by-reading-files-in-parallel)
* [Parsing logs using `xan separate`](#parsing-logs-using-xan-separate)
* [Running subprocesses to extract raw text from PDF files](#running-subprocesses-to-extract-raw-text-from-pdf-files)

## Paginating urls to download

Let's say you want to download the latest 50 pages from [Hacker News](https://news.ycombinator.com). Fortunately our [`minet`](https://github.com/medialab/minet) tool knows how to efficiently download a bunch of urls fed through a CSV file.

The idea here is to generate CSV data out of thin air and to transform it into an url list to be fed to the `minet fetch` command:

```bash
xan range --start 1 50 --inclusive | \
xan select --evaluate '"https://news.ycombinator.com/?p=" ++ n as url' | \
minet fetch url --input -
```

The `xan range` command produces a CSV looking like this:

| n   |
| --- |
| 1   |
| 2   |
| 3   |
| 4   |
| 5   |
| ... |

Then the `xan select --evaluate` part use the following expression to transform the file on the fly:

```python
# We append the content of the "n" column to the given url
"https://news.ycombinator.com/?p=" ++ n as url
```

This gives us:

| url                               |
| --------------------------------- |
| https://news.ycombinator.com/?p=1 |
| https://news.ycombinator.com/?p=2 |
| https://news.ycombinator.com/?p=3 |
| https://news.ycombinator.com/?p=4 |
| https://news.ycombinator.com/?p=5 |
| ...                               |

That is fit to be fed into `minet fetch`.

## Making sure a crawler was logged in by reading files in parallel

Let's say you crawled some media website and 1. wrote all the downloaded files into a directory (aptly named `downloaded`) and 2. produced a CSV report listing the downloaded files and their relative paths on disk.

Now you had to be logged in to retrieve the full text of crawled articles. Because you are a dilligent individual, you did not forget to use a proper authenticated cookie while crawling. But what if you messed up? Let's double check pages were crawled correctly.

Fortunately, the crawled media website shows your username on the top right section of each page when you are logged in, so you could easily check whether everything went smoothly by searching for an occurrence of your very specific username (`yomguithereal`) in every HTML file downloaded.

Let's do so with `xan`, in parallel, with a progress bar for flair (indeed, reading millions of HTML files tends to take some time):

```bash
xan progress crawl.csv | \
xan filter --parallel '"downloaded".pathjoin(path).read() | !contains(_, /yomguithereal/i)' | \
> not-crawled-correctly.csv
```

Here the `xan filter` command will know, thanks to the `--parallel` flag, how to use a suitable amount of threads to read and test files as fast as possible.

Now the following moonblade expression:

```perl
"downloaded".pathjoin(path).read() | !contains(_, /yomguithereal/i)
```

means: "join `downloaded` to each row's `path` column value, then read the content at the created full relative path, then check whether it does not contain an occurrence of the `/yomguithereal/i` case-insenstive regex".

## Parsing logs using `xan separate`

`xan separate` is a command able to "separate" a single CSV column into multiple ones through a variety of different methods. It boasts both a `-r/--regex` and `-c/--capture-groups` flags that let you give a regex pattern and create new columns based on its matched groups. It is therefore suitable to use it to parse logs.

See an example here of using a command to parse k8s access logs to structure them better and produce some quick time series:

```bash
xan from --from txt ~/Downloads/access.log.gz --column log | \
xan separate log -rc '- - \[([^\]]+)\] "([^"]+)" (\d+) \d+ "[^"]*" "([^"]+)"' \
  --keep \
  --into datetime,http_call,http_status,user_agent \ |
xan map --overwrite 'datetime.datetime("%d/%b/%Y:%H:%M:%S %z") as datetime, http_call.split(" ")[1] as url' \
> logs.csv
```

First we use the `xan from` command to convert our log lines into proper CSV data (log lines can countain commas or quotes for instance and those must be dealt with properly).

Then we apply our unwieldy regex to create some new columns given to the `--into` flag. The `--keep` flag is here because we want to keep the original log line in the result, so we can add further processing later on if needed.

Now, time in the logs is indicated using this atrocious format: `11/Jun/2025:05:48:49 +0000`, so we apply a `xan map` command to the result to convert it to something more appealing like ISO and we also extract the url from HTTP call at the same time. The `--overwrite` flag of the `map` command means we can replace any column from input having the same name in the output. Here it means we will replace the `datetime` column altogether and add a new one named `url`. This saves us a `xan transform` in addition to the `xan map`.

Now here is what a time series of all the logs look like:

```bash
# We use --ignore because some records don't have a time
# The --count flag means we don't have value for the y axis, we just
# want to count number of rows for each time slot
xan plot --line --time datetime --count logs.csv --ignore
```

![separate-log1](./img/pipelines/separate-log1.png)

But as with any access log, there is noise related to bots and people accessing stylesheets, scripts & images so let's focus on our website's homepage thusly:

```bash
# Searching exact matches for url "/", that is to say the homepage
xan search -s url --exact / logs.csv | xan plot -LT datetime --count
```

![separate-log2](./img/pipelines/separate-log2.png)

## Running subprocesses to extract raw text from PDF files

Ok, let's go wild: we have downloaded a long list of PDF reports from some UN subcommittee. We will attempt to use the `pdftotext` command on them to extract their raw text so we can do proper NLP down the line. But there is an issue: we are very bad at using the `xargs` or `parallel` commands and never remember how to write a proper bash loop.

Don't worry, `xan` is here for us:

```bash
xan filter 'http_status == 200 && col("path", 1).endswith(".pdf")' report-files.csv | \
xan map --parallel 'col("path", 1) | pjoin("files", _) | fmt("pdftotext {} -", _) | shell(_).trim() as text' | \
xan select ndoc,uid,title,lastModified,link,text | \
xan rename -s lastModified last_modified > report-files-with-raw-text.csv
```

Here `xan` was able to manage `pdftotext` subprocesses (using the `shell` moonblade function), in parallel, for each row of our CSV file listing the reports on disk, so we can add the extracted text in a new column. Pretty rad, no?

We need to use `col("path", 1)` in our expressions because of course there are two distinct columns with same name in our input CSV file.

We also use the `xan rename` command in the end because mixing camelCase and snake_case is an unforgivable fashion *faux-pas*.


<!--

xan parallel cat \
  --progress \
  -S media \
  -B -1 \
  -P '
    select -f scripts/harmonization.moonblade |
    map "date_published.ym().try() || `N/A` as month" |
    search -Bri -s headline,description,text
      --patterns scripts/climate_week/queries.csv
      --pattern-column pattern
      --name-column name |
    groupby month -C -5: "sum(_)" |
    sort -s month' \
  */articles.csv.gz | \
xan transform media '_.split("/")[0]' > $BASE_DIR/matches.csv

xan groupby into xan heatmap example

-->