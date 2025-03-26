# Deduplicating CSV data with `xan`

Deduplicating data seems to be a very straightforward task, no?

> No.

It can be a deep rabbithole as there are many ways to deduplicate data and `xan` exposes a lot of ways to do so, all tailored to specific use-cases.

## Summary

* [Hashmap-based deduplication with `xan dedup`](#hashmap-based-deduplication-with-xan-dedup)
* [Deduplicating already sorted data](#deduplicating-already-sorted-data)
* [Deduplicating while sorting with `xan sort -u`](#deduplicating-while-sorting-with-xan-sort--u)
* [Deduplicating while merging multiple sorted files with `xan merge -u`](#deduplicating-while-merging-multiple-sorted-files-with-xan-merge--u)
* [Merging groups of related contiguous rows with `xan implode`](#merging-groups-of-related-contiguous-rows-with-xan-implode)
* [Deduplicating by grouping rows with `xan groupby`](#deduplicating-by-grouping-rows-with-xan-groupby)

## Hashmap-based deduplication with `xan dedup`

note about flushing and memory usage with a table, speak about -e, and the various --keep/--choice

## Deduplicating already sorted data

## Deduplicating while sorting with `xan sort -u`

speaking about -e, and --count

## Deduplicating while merging multiple sorted files with `xan merge -u`

## Merging groups of related contiguous rows with `xan implode`

## Deduplicating by grouping rows with `xan groupby`

speak about --sorted
