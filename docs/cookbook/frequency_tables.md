# Merging frequency tables, three ways

## Summary

* [Introduction](#introduction)
* [1. Basic approach](#1-basic-approach)
* [2. Throwing more CPUs at the problem](#2-throwing-more-cpus-at-the-problem)
* [3. Sparing as much memory as possible](#3-sparing-as-much-memory-as-possible)

## Introduction

Let's say we work for an Internet archive and each month the system outputs a tally of the linked domains across all the crawled web pages, in CSV format.

Here is what was found for March 2024:

*202403_domains.csv*

| domain      | count |
| ----------- | ----- |
| lefigaro.fr | 23    |
| lemonde.fr  | 56    |
| meta.com    | 3     |
| wagon.net   | 1     |

And here is what was found for April 2024:

*202404_domains.csv*

| domain        | count |
| ------------- | ----- |
| abelard.co.uk | 2     |
| lemonde.fr    | 4     |
| meta.com      | 8     |
| zorba.net     | 16    |

Now let's say we have a whole lot of those files, since the archive has now been opened for many years now. How can we use `xan` to merge those frequency tables, to see what are the most popular domains across the whole dataset?

## 1. Basic approach

The first step is to gather all those files into a single one that we will be able to process downstream. To do so, we can use the `xan cat rows` command that takes multiple CSV files as input and returns a concatenated file:

```bash
# Using bash glob syntax here to pass all my files to the command at once
xan cat rows *_domains.csv | xan view
```

And here is the result:

| domain        | count |
| ------------- | ----- |
| lefigaro.fr   | 23    |
| lemonde.fr    | 56    |
| meta.com      | 3     |
| wagon.net     | 1     |
| abelard.co.uk | 2     |
| lemonde.fr    | 4     |
| meta.com      | 8     |
| zorba.net     | 16    |

Now we need to "group" the rows by the `domain` column, all while summing values found in the `count` one, and we should have a merged frequency table. This is exactly what the `xan groupby` was designed to perform:

```bash
xan cat rows *_domains.csv | xan groupby domain 'sum(count) as count' | xan view
```

Here is the result:

| domain        | count |
| ------------- | ----- |
| lefigaro.fr   | 23    |
| abelard.co.uk | 2     |
| lemonde.fr    | 60    |
| wagon.net     | 1     |
| meta.com      | 11    |
| zorba.net     | 16    |

Note how this is reminiscent of SQL. The `groupby` command is able to perform a lot of different aggregations on the grouped rows. Run `xan groupby --aggs` to read the full list of what is available.

And now, if we want the top 3 entries (in a real life scenario we would of course have millions of domains, not just 6), we can use the `xan top` command like so:

```bash
# Using "\" to break the command into multiple lines for clarity
xan cat rows *_domains.csv | \
xan groupby domain 'sum(count) as count' | \
xan top count -l 3 | \
xan view
```

And we shall get:

| domain      | count |
| ----------- | ----- |
| lemonde.fr  | 60    |
| lefigaro.fr | 23    |
| zorba.net   | 16    |

Finally, note that bash commands usually only accept a certain number of arguments and will yell when given too much at once. This would probably happen if we need to process hundreds of months at once.

Fortunately the `xan cat rows` command is also able to be fed its paths through an external source using the `--paths` flag:

```bash
# Using the `find` command to list files and feeding `xan cat rows` through stdin
find . -name '*_domains.csv' | \
xan cat rows --paths - | \
xan groupby domain 'sum(count) as count' | \
xan top count -l 3 | \
xan view
```

## 2. Throwing more CPUs at the problem

The previous solution runs in a time that depends on the size of the total amount of CSV data to process. This is usually fine, but sometimes we need something faster. Fortunately most computers nowadays pack more than a single core. Some servers even have a number of cores that would make one blush.

So, if we wanted to go faster, maybe at the expense of more memory, we could leverage our cores through parallelism. This is exactly what `xan parallel` does, by providing typical map-reduce parallel implementations that can work when the dataset is split into multiple CSV files.

Thus, the `xan parallel` command has a `groupby` subcommand that makes it quite easy to port our first solution to the parallelized operation:

```bash
xan parallel groupby domain 'sum(count) as count' *_domains.csv | \
xan top count -l 3 | \
xan view
```

## 3. Sparing as much memory as possible

One thing to notice with both solutions 1 and 2 is that we still need to be able to fit all the existing domains and the associated count in memory. This might be too much for some computers.

But if we look closely our monthly input files, we would notice that they are already sorted on the domain column. Luckily, it is very easy to merge sorted lists without using more memory than what is required to store one CSV row per merged file, using the proper [algorithms](https://en.wikipedia.org/wiki/Merge_algorithm).

This is what the `xan merge` command does:

```bash
xan merge -s domain *_domains.csv | xan view
```

And we will get:

| domain        | count |
| ------------- | ----- |
| abelard.co.uk | 2     |
| lefigaro.fr   | 23    |
| lemonde.fr    | 4     |
| lemonde.fr    | 56    |
| meta.com      | 8     |
| meta.com      | 3     |
| wagon.net     | 1     |
| zorba.net     | 16    |

Notice how the file rows are interleaved together all while respecting domain order? This means that now, all rows relative to a single domain will be contiguous. What's more, the `xan groupby` command has a `-S/--sorted` flag for this precise use-case, so it can perform aggregation without needing more memory than what's required for a single group!

```bash
xan merge -s domain *_domains.csv | \
xan groupby --sorted domain 'sum(count) as count' | \
xan top count -l 3 | \
xan view
```

And here we have a solution that is as performant as the first one (probably even faster), but slower than the second one, and that uses as little memory as possible.

It can only work, however, if the input files are sorted, which is of course not always the case.

Now we can also rely on bash loops etc. to sort the files ahead of time and only use memory proportional to what is required to fit a single file in memory:

```bash
for path in $(find . -name '*_domains.csv')
do
  xan sort -s domain $path > $(basename -s.csv $path)_sorted.csv
done
```

Or, if you want to do so in parallel using `xan parallel`:

```bash
xan parallel map '{}_sorted.csv' *_domains.csv -P 'sort -s domain'
```
