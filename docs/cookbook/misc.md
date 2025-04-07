# Miscellaneous

## Summary

* [Generating a CSV of paginated urls to download](#generating-a-csv-of-paginated-urls-to-download)
* [Reading files in parallel](#reading-files-in-parallel)
* [Piping to `xargs`](#piping-to-xargs)

## Generating a CSV of paginated urls to download

Let's say you want to download the latest 50 pages from [Hacker News](https://news.ycombinator.com) using another of our tools named [minet](https://github.com/medialab/minet).

You can pipe `xan range` into `xan select -e` into `minet fetch`:

```bash
xan range -s 1 50 -i | \
xan select -e '"https://news.ycombinator.com/?p=" ++ n as url' | \
minet fetch url -i -
```

## Reading files in parallel

Let's say one column of your CSV file is containing paths to files, relative to some `downloaded` folder, and you want to make sure all of them contain some string (maybe you crawled some website and want to make sure you were correctly logged in by searching for some occurrence of your username):

```bash
xan progress files.csv | \
xan filter -p 'pathjoin("downloaded", path) | read | !contains(_, /yomguithereal/i)' > not-logged.csv
```

## Piping to `xargs`

Let's say you want to delete all files whose path can be found in a column of CSV file. You can select said column and format it with `xan` before piping to `xargs`:

```bash
xan to txt -s path files.csv | \
xargs -I {} rm {};
```
