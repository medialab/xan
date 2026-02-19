# `xan` pipelines

Curated collection of unhinged `xan` pipelines.

## Summary

* [Paginating urls to download](#paginating-urls-to-download)
* [Making sure a crawler was logged in by reading files in parallel](#making-sure-a-crawler-was-logged-in-by-reading-files-in-parallel)
* [Parsing logs using `xan separate`](#parsing-logs-using-xan-separate)

## Paginating urls to download

Let's say you want to download the latest 50 pages from [Hacker News](https://news.ycombinator.com). Fortunately our [`minet`](https://github.com/medialab/minet) tool knows how to efficiently download a bunch of urls fed through a CSV file.

The idea here is to generate CSV data out of thin air and to transform it into an url list to be fed to the `minet fetch` command:

```bash
xan range --start 1 50 --inclusive | \
xan select --evaluate '"https://news.ycombinator.com/?p=" ++ n as url' | \
minet fetch url --input -
```

## Making sure a crawler was logged in by reading files in parallel

Let's say one column of your CSV file is containing paths to files, relative to some `downloaded` folder, and you want to make sure all of them contain some string (maybe you crawled some website and want to make sure you were correctly logged in by searching for some occurrence of your username):

```bash
xan progress files.csv | \
xan filter -p 'pathjoin("downloaded", path) | read | !contains(_, /yomguithereal/i)' > not-logged.csv
```

## Parsing logs using `xan separate`

<!-- show plots -->

```bash
xan from -f txt ~/Downloads/toflit18.log.gz | xan rename log | xan separate 0 -rc '- - \[([^\]]+)\] "([^"]+)" (\d+) \d+ "[^"]*" "([^"]+)"' --keep --into datetime,http_call,http_status,user_agent | xan map -O 'datetime.datetime("%d/%b/%Y:%H:%M:%S %z") as datetime, http_call.split(" ")[1] as url' > toflit18-log.csv

xan search -s url -e / toflit18-log.csv.gz | xan plot -LT datetime --count
xan plot -LT datetime --count toflit18-log.csv.gz --ignore
```

<!--

xan filter 'http_status == 200 && col("path", 1).endswith(".pdf")' report-files.csv | xan map -p 'col("path", 1) | pjoin("files", _) | fmt("pdftotext {} -", _) | shell(_).trim() as text' | xan select ndoc,uid,title,lastModified,link,text | xan rename -s lastModified last_modified > final.csv

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

 -->