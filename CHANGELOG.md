# Changelog

## 0.55.0 (provisional)

*Breaking*

* Changing how `xan separate` generates default column names.
* `xan from -f=(json|ndjson|jsonl)` will now emit column in input order by default.
* Changing `xan to -B/--buffer-size` to `--sample-size` to harmonize flag names with `xan from`.

*Features*

* Adding the `xan complete` command.
* Adding an optional unit to `ceil`, `floor`, `round` & `trunc` moonblade function. E.g. floor to nearest decade: `floor(year, 10)`.
* Adding `basename` & `dirname` moonblade functions.
* Adding `parse_py_literal` moonblade functions. Useful to deal with files dubiously serialized using `pandas`.
* Adding `xan view --repeat-headers=(auto|always|never)`.
* Adding `xan view --reveal-whitespace=(auto|always|never)`.
* Adding `--color` support to `XAN_VIEW_ARGS`.
* Adding `xan from -f json --sample-size -1` to sample the whole file.
* Adding `xan from -f json --single-object`.
* Adding `xan from --sort-keys`.
* Adding `xan to (json|ndjson|jsonl) --sample-size -1` to sample the whole file.
* Adding `xan to (json|ndjson|jsonl) --strings` flag.
* Adding `xan separate --prefix`.
* Adding `xan heatmap -C` short flag for `--cram`.
* Adding `xan heatmap --repeat-headers`.
* Adding `rank`, `cume_dist` and `ntile` window functions.

*Quality of Life*

* `xan view -p` will not print bottom header anymore by default.
* `xan view` will not reveal problematic whitespace if output is not colored anymore, by default.
* Better `xan hist` error messages and help.

## 0.54.1

*Fixes*

* Fixing `xan freq --groupby` incorrectly unescaping group cells.
* Fixing help related to `xan pivot` & `xan unpivot`.
* Upgrading `simd-csv` to get safety fixes.
* Fixing evaluation of moonblade commands related to column indexing.

## 0.54.0

The **SIMD** update.

*Breaking*

* Bumping MSRV to `1.83.0`.
* Dropping `xan plot -Y/--add-series`. It is now possible to select multiple columns as `<y>` in  `xan plot <x> <y>` instead.
* Dropping the `-C/--force-colors` flag in `flatten`, `heatmap`, `hist`, `plot` and `view` in favor of the more standardized and flexible `--color=(auto|never|always)` flag.
* `xan join` will now automatically drop joined columns from one the files when it is obviously safe to do so.
* `xan behead` & `xan rename` do not normalize the output anymore to be as fast as possible.
* The new SIMD CSV parser might not deal with CSV irregular cases the same way `rust-csv` did. In any case, `xan input` will still continue to use `rust-csv`.
* `xan slice -B/--byte-offset` & `xan slice -A/--accumulate` are now mutually exclusive.
* `xan input` has been overhauled.
* Dropping `xan count --sample-size`.
* Overhauling `xan fixlengths` to accept streams by shifting default from double-pass read to buffering the whole stream into memory.
* `xan plot --x-scale log & --y-scale log` are now natural log. Use `log10` for the base10 log as before.
* Dropping `xan reverse -m/--in-memory` flag. Behavior is now automatically detected.
* Dropping `xan shuffle -m/--in-memory` flag. Loading the file into memory is now the default. The `xan shuffle -e/--external` flag has been added if
you want the old default behavior.
* `xan bins` now outputs `<empty>` values instead of `<nulls>`.
* Overhauling `xan bins`. The default is now to find nice boundaries for the bins. Use `-e/--exact` to revert to the old behavior. The default number of bins is now `10`, and won't use Freedman-Diaconis rule by default. A `-H/--heuristic` flag has been added if you want to automatically select a suitable number of bins.

*Features*

* Adding `xan flatten -F/--flatter`.
* `xan pivot` can now target multiple columns.
* Adding the `xan grep` command for fast but coarse filtering.
* Adding `xan search -f/--flag`.
* Adding `xan map -F/--filter`.
* `xan search -B/--breakdown` now consolidates the results when multiple patterns have a same name.
* Adding `xan flatten --row-separator`.
* Adding `xan flatten --csv`.
* Adding `xan headers --color`.
* Adding the `xan join <columns> <input1> <input2>` arity as a convenience when joined column names are the same in both inputs.
* Adding `xan join -D/--drop-key=(none|both|left|right)`.
* Adding `xan fuzzy-join -D/--drop-key=(none|both|left|right)`.
* Adding `xan plot -A/--aggregate`.
* Adding support for plural selection clauses in both `xan select -e` & `xan map` e.g. `xan map 'full_name.split(" ") as (first_name, last_name)`.
* Adding `xan search -P/--add-pattern`.
* Adding `xan groupby -M/--along-matrix`.
* Adding `xan groupby -T/--total`.
* Adding support for `.ndjson` & `.jsonl` files. Those are considered as headless TSV files with null byte quoting so you can easily use them with `xan` commands.
* Adding out-of-the-box support for `.vcf`, `.sam`, `.bed`, `.gtf` & `.gff2` files.
* Adding a `xan cat cols` alias to `xan cat columns`.
* Adding `zstd` support.
* Adding `earliest` & `latest` moonblade functions.
* Adding `xan dedup -f/--flag`.
* Adding `-k` short flag for `xan dedup --keep-duplicates`, and `-C` short flag for `xan dedup --choose`.
* Adding `xan fixlengths -H/--trust-header`.
* Adding `xan separate`.
* Adding full log scale support to `xan plot`.
* Adding `xan hist --scale`.
* `xan window` is now able to run total aggregations.
* Adding `thousands_sep`, `comma` and `significance` kwargs to `numfmt` moonblade function.

*Fixes*

* Fixing `xan dedup --check` bug where the first record was ignored.
* Fixing `xan hist -D` when a same date is found multiple times.
* Fixing `xan from -f xls` datetime conversion.
* Fixing `xan flatten` & `xan view` when column names contain line breaks.
* Fixing invalid argument parsing error being printed to stdout instead of stderr.
* Fixing `xan progress` SIGINT corrupting output.
* Fixing `xan enum -A/--accumulate`.
* Fixing `xan from -f tar` when tarball archive is not gzipped.
* Fixing `min` & `max` moonblade function when passing a list of numbers.
* Fixing `xan flatten -H` edge cases.
* Fixing commands requiring seekable streams accepting unindexed compressed files by error.
* Fixing `xan plot --count --y-scale log`.

*Performance*

* Wildly improving performance of most of `xan` commands by leveraging a novel SIMD CSV parser/writer.
* Improving performance of `xan from -f txt` & `xan from -f npy`.
* Improving memory footprint of hash-based commands (e.g. `frequency`, `groupby`, `dedup` etc.).
* Improving performance of `xan progress`, `xan range`, `xan enum`, `xan behead`, `xan rename`.

*Quality of Life*

* `xan parallel cat` now flushing more consistently.
* Better highlighting of problematic strings in `xan flatten`, `xan view` & `xan headers`.
* `xan parallel` will now generally stop as soon as an error is detected in a subprocess and cleanly report errors.
* Better argv parsing error UX in general.
* The `-p` flag will now avoid going further than 16 to avoid issues on server with many CPUs where hogging the resources is an issue and where using too much threads at once could hurt performance. The `-t` flag remain available to tweak the number of threads.
* `xan hist` will now dim bars having a `0` count so you can easily distinguish them from non-empty bars.

## 0.53.0

*Breaking*

* `xan partition` now normalizes filenames to lowercase to correctly deal with case-insensitive filesystems. `xan partition` also gets a related `-C/--case-sensitive` flag.

*Features*

* Adding `all` and `any` moonblade higher-order functions.
* Allowing moonblade `printf` function to be called with lists.
* Adding `-f/--evaluate-file` flag to `map`, `filter`, `flatmap` & `transform` commands.
* Adding `xan map -O/--overwrite`.

*Fixes*

* Fixing `xan top -T/--ties` edge case.
* Fixing broken pipe panics for some commands.
* Dropping remnant `dbg!` macro when reading files in reverse.
* `xan flatten -H` now correctly working on more data types.

*Performance*

* Using `jemallocator` for musl builds.

*Quality of Life*

* Better moonblade `printf` function error messages.

## 0.52.0

*Breaking*

* `xan search --count` will not emit rows with 0 matches anymore unless `--left` is used.

*Features*

* `xan transform` is now able to work on a selection of columns, rather than on a single column.
* Adding the `xan unpivot` command.
* Adding the `xan pivot` command.
* Adding `xan join --semi` & `xan join --anti` commands.
* Adding `xan slice --raw`.
* Adding default expression argument to `lead` & `lag` window functions.
* Adding `shlex_split`, `cmd` and `shell` moonblade functions.
* Adding `aarch64-apple-darwin` and `aarch64-unknown-linux-gnu` to CI builds.
* Adding `to_fixed` moonblade function.
* Adding decimal places optional argument to `ratio` & `percentage` aggregation functions.
* Adding `frac` & `dense_rank` aggregation functions to `xan window`.

*Fixes*

* Loosening `xan partition` sanitizer to allow hyphens, dashes and points.
* Fixing `xan parallel --progress` display.
* Fixing logic error in `xan search -B` when using without `--left`.
* Fixing `xan parallel cat` when working on file chunks with `-P` or `-H`.
* Fixing moonblade list/string slicing with some combinations of negatives indices.
* Fixing moonblade `split` function not using regex patterns properly.
* Fixing moonblade parsing wrt regex patterns and comments (using a regex pattern containing `#` was not possible).
* Fixing `lead` window aggregation function when working on any column that is not the first one.
* Fixing `xan view -S/--significance` being overzealous, especially wrt integers.

*Performance*

* Improving performance of `xan parallel` when working on file chunks.

*Quality of Life*

* `xan headers` now report more useful information when files have diverging headers.
* Better error messages for `read_json` and `parse_json` moonblade functions.
* `xan view -p` will not engage pager when input errored or is empty.
* `xan select -e & -f` become boolean flags instead of error-inducing invocation variants.

## 0.51.0

The **parallel** update.

*Breaking*

* Dropping undocumented `xan index` and related interactions (in `xan count`, `xan sample`, `xan slice` & `xan split --jobs`).
* Dropping now useless `coalesce` moonblade function.
* `xan split` now accepts its output directory as an optional flag.
* `xan partition` now accepts its output directory as an optional flag.
* `xan split -s` becomes `xan split -S` to avoid confusion with the `-s/--select` flag used everywhere else.
* Dropping useless `xan count --csv` flag.
* Dropping `xan freq -t/--threshold`. Use `xan freq | xan filter 'count >= n'` instead.
* Adding `xan slice -I/--indices` taking care of `xan slice -i` polymorphism taking multiple indices before.
* `xan parallel freq` now follows `xan freq` behavior regarding limits.
* Dropping `xan url-join` & `xan regex-join`. Both commands have been merged into a new `xan fuzzy-join` command using the `-u/--url-prefix` & `-r/--regex` flags respectively.
* `xan from --sheet` becomes `--sheet-name` and is no longer the default. `--sheet-index 0` becomes the default.
* Dropping `xan foreach`. It is not distinctive enough as you can use `xan map` for the same purpose and get useful information about the results of evaluated side effects or write to `/dev/null`.
* Renaming `xan agg --cols` to `xan agg --along-rows`.
* Changing `cell` placeholder to anonymous `_` value in `xan agg -R/--along-rows`.
* Dropping moonblade commands `-E/--errors` flags. A lot has changed since they were created. They will be reevaluated in the future if required. You can rely on the `try` & `warn` moonblade functions instead, for now.
* Dropping `xan select -A/--append`. Latest `xan map` is now actually equivalent to `xan select -eA`.
* Changing `xan map` to accept a selection expression able to create multiple columns at once rather than a single expression and a column name. This means `xan map 'expr' col_name` becomes `xan map 'expr as col_name'`.

*Features*

* Adding `xan count -a/--approx`.
* Adding `xan slice --end-byte`.
* Adding `xan slice -S/--start-condition` & `xan slice -E/--end-condition`.
* Adding `xan slice -L/--last`.
* Allowing `-n/--no-headers` and `-d/--delimiter` flags to appear before subcommands.
* Adding backtick quoted strings to moonblade.
* Adding moonblade `printf` function.
* Adding moonblade `pad`, `lpad` & `rpad` functions.
* Adding `xan select -f/--evaluate-file`.
* Adding multi-member gzip files support (to handle files compressed with `bgzip` notably).
* Adding `xan split -f` & `xan partition -f` short flag for `--filename`.
* Adding `xan split -c/--chunks` & `xan split --segments`.
* Adding `xan sample -§/--cursed`.
* Adding `xan search -B/--breakdown` and the related `--name-column` flag.
* Adding CSV file chunking capabilities to `xan parallel`.
* Adding `xan from md`.
* Adding `xan parallel map`.
* Adding `-p/--parallel` & `-t/--threads` to `count`, `freq`, `stats`, `search`, `agg` & `groupby` commands.
* Adding piped column access to expression given to `xan flatmap -r`.
* Adding `xan rename -R/--replace` & `xan rename -x/--suffix`.
* Adding `xan parallel freq -l/--limit, -A/--all, -a/--approx & -N/--no-extra`.
* Adding `xan search -U/--unique-matches & --sep & --left`.
* Adding parallelization through novel file segmentation of files compressed with `bgzip` when a `.gzi` index can be found.
* Adding the `xan window` command for window aggregations like rolling averages, cumulative sums, lags etc.
* Adding `xan help window`.
* Adding `xan head` & `xan tail` as aliases over `xan slice -l` & `xan slice -L` respectively.
* Adding `xan from --sheet-index & --list-sheets`.
* Adding `xan flatten -H/--highlight & -i/--ignore-case`.
* Adding `xan agg -C/--along-cols` & `xan agg -M/--along-matrix`.
* Adding `xan groupby -C/--along-cols`.
* Adding support for `xan search -l -p -t`.
* Adding `rms` moonblade aggregation function.
* Adding `xan scrape -E/--encoding`.
* Adding CDX files support.
* Adding `regex` moonblade function.
* Adding `header`, `col_index` & `col_index?` moonblade functions.
* Adding `find` & `find_index` moonblade functions.
* Adding `-l/--limit` support to `xan search -p` & `xan filter -p`.
* Adding `xan pivot-longer` alias to `xan unpivot`.
* Adding `xan pivot-wider` alias to `xan pivot`.

*Fixes*

* Adding missing highlight for `NULL` values in `xan view` & `xan flatten`.
* Fixing moonblade slicing wrt negative indexing and nontrivial inner expression.
* Fixing moonblade `get` function for bytes.
* Fixing `xan sort -e` skipping first record of each chunk.
* Fixing `xan sort -e` stability.
* More accurate `xan sort -e` memory usage calculations.
* Fixing `xan transform -n`.
* Fixing `xan view -g -s`.
* Fixing moonblade concretization wrt branching.
* Fixing `xan behead -o` and `xan behead -Ao`.
* Reorganizing `xan help functions`.
* Fixing lexicographic extent merging in `xan parallel stats`.
* Fixing `xan to md` width alignment.
* Renaming `xan parallel --shell-preprocessing` short flag to be `-H` because it was being overriden by `-S/--source-column`.
* Adding missing subcommand completions for `xan parallel` & `xan cat`.
* Better default threads count heuristics.
* Better `xan plot -T` date parsing.
* Fixing `xan search` replacements when using the `-s/--select` flag with a non-full selection.
* Adding the `xan view -r/--right` flag to force right alignment for a selection of columns.
* Fixing `xan flatten` broken pipe panics when piped.
* Fixing `xan plot -R/--regression-line` when linear function endpoints are out of bounds.
* `xan parallel` early exits when a target file does not exist.
* Fixing `moonblade` list slicing.
* Fixing `cols()` & `headers()` moonblade functions without arguments.
* Fixing `cols()` & `headers()` not working with dynamic arguments.
* Fixing moonblade indexing parsing.
* Fixing aggregation arity validation.
* Fixing `xan agg` & `xan groupby` behavior wrt `-n/--no-headers`.
* Fixing shortcircuiting of `and` and `or` moonblade functions.
* Fixing issue with degenerate cases in `xan bins --nice`.
* Fixing bin allocation in `xan bins --nice`.
* Fixing `xan bins --nice` first and last bound to stick to min & max.
* Fixing negative indexing with `col*(name, pos)` moonblade functions.
* Fixing `argmin` & `argmax` parallel stability.
* Fixing panic with `xan plot` when using log scales and min/max are <= 0.

*Performance*

* Switching hashmaps to `ahash`.
* Optimizing moonblade pipelines with more than a single underscore substitution.
* Improving `xan reverse` performance.
* Reducing memory footprint of aggregators.
* Optimizing `xan select -e` allocations.

*Quality of Life*

* Prepending xan subcommand to error messages.
* Better error messages when moonblade expressions cannot be parsed.
* Displaying number of threads actually used when using `xan parallel`.
* `xan view` now automatically right-align columns containing only integers.
* Better moonblade casting errors.
* `xan bins` formatted bound will now be padded for better readability.

## 0.50.0

*Features*

* Adding moonblade function `log2`, `log10` and support for custom base with an optional argument of `log`.
* Adding `\0`, `\x..` and `\u{......}` literals to moonblade strings.
* Adding moonblade function `col?`.

*Fixes*

* Better color support for legacy Windows terminals.
* Fixing `to_timezone` function with UTC timestamps on some platforms where jiff is built "bundled".
* Fixing moonblade commands (e.g. `filter`, `map` etc.) when using `-n/--no-headers`.

## 0.49.3

*Fixes*

* Adding missing `-M/--hide-info` support with `XAN_VIEW_ARGS`.
* Pinning MSRV to `1.81.0` in CI builds to avoid Windows Defender false positives.

## 0.49.2

*Fixes*

* Overhauling & fixing CI builds.

## 0.49.1

*Fixes*

* Fixing compilation with musl.
* Fixing `xan cat rows -n`.

## 0.49.0

*Breaking*

* Dropping mostly useless `-p/--parallel` & `-c/--chunk-size` flags in `xan agg` & `xan groupby`. They were only useful when the inner aggregated expression was costly (i.e. reading files) and you can use `xan map -p` upstream for this instead. See also `xan parallel (agg | groupby)` if you want to work in parallel over multiple files.

*Features*

* Adding `xan input --tabs`.
* Adding `xan input -H/--skip-headers`.
* Adding `xan input -L/--skip-lines`.
* Adding `xan input -R/--skip-rows`.
* Adding `xan input --vcf, --gtf, --gff`.
* Adding `xan search -R/--replace` & `xan search --replacement-column`.
* Adding `xan rename -S/--slugify`.
* Adding moonblade function `sum`.
* Adding support for `.psv`, `.ssv` & `.scsv` file extensions.
* Adding `xan headers -s/--start`.
* Adding `xan to txt`.
* Adding `xan behead -A/--append`.
* Adding `xan hist -G/--compress-gaps`.
* Adding `xan agg --cols`.

*Fixes*

* `xan view --no-headers` now automatically toggles `--hide-headers`.
* `xan from` correctly decompress some gzipped formats.
* `xan fill -v` correctly fills empty cells at beginning of files.
* `xan parallel -t` will not use more threads than inputs.
* Fixing `xan stats` panicking when encountering NaN values.
* Allowing tabs to be understood as whitespace in moonblade expressions.
* Fixing `xan join --cross` when joined files don't have the same number of columns.
* Adding missing `-n/--no-headers` & `-d/--delimiter` to `xan to`.
* Fixing `xan progress -B` with gzipped files.
* Adding missing `-C/--force-colors` to `xan plot`.

## 0.48.0

*Breaking*

* Dropping `xan union-find`. See `xan network` now for similar utilities.
* `xan explode --singular` becomes `xan explode --singularize`.
* `xan implode --plural` becomes `xan implode --pluralize`.

*Features*

* Adding scraping selectors `prev_sibling`, `next_sibling`, `find_ancestor` & `last`.
* Adding `xan scrape images`.
* Adding `xan search -u/--url-prefix`.
* Adding `xan network -f nodelist` and `xan network --degrees`.
* Adding various format aliases to `xan from`.
* Adding `xan explode -D/--drop-empty`.
* Adding `xan from tar`.
* Adding `xan url-join`.

*Fixes*

* More inflection cases supported in both `xan explode -S` and `xan implode -P`.
* Better error reporting with `xan scrape`.
* Fixing `xan scrape` processing values when selection is empty.
* Fixing `parent` scraping selector.
* Adding missing higher-order functions documentation to `xan help`.

*Performance*

* Improving performance of `xan explode`.
* Improving performance of `xan implode`.

## 0.47.1

*Fixes*

Fixing CI builds.

## 0.47.0

*Breaking*

* Moonblade `strftime()` and all other date formatting functions such as `ymd()` do not support timezones any more, see `to_timezone` and `to_local_timezone` instead.

*Features*

* Adding moonblade function `to_timezone` and `to_local_timezone`
* Improving `xan help cheatsheet`.
* Adding moonblade function `try`.
* Adding moonblade functions `int` & `float`.
* Adding moonblade functions `lru` & `urljoin`.
* `moonblade` now tolerates line-breaks as whitespace when parsing. This makes it possible to write your expressions on multiple lines.
* `moonblade` now accepts comments starting with `# `.
* Adding `xan scrape` command.
* Adding `xan help scraping` subcommand.
* Adding moonblade function `html_unescape`.

*Fixes*

* Fixing `xan flatten -w` wrt line breaks.
* Fixing underscore expansion when contained in map & list expressions.
* Fixing `xan to md` cell escaping.

*Performance*

* Decrease `moonblade`-related commands memory consumption.

## 0.46.2

*Fixes*

* Fixing Windows compilation.

## 0.46.1

*Fixes*

* Fixing Windows compilation.

## 0.46.0

*Breaking*

* Moonblade concat operator becomes `++` instead of `.`.
* Overhauling moonblade cli documentation:
  * Dropping `--functions`, `--cheatsheet` and `--aggs` everywhere
  * Adding a proper `xan help` command
* Dropping `xan glob`.
* Adding access & call operator to moonblade:
  * Member access: `map.name` (same as `get(map, "name")`)
  * Function call: `string.len()` (same as `len(string)`)
* Help is now printed in stdout (typically when using the `-h/--help` flag).

*Features*

* Adding `xan to html`.
* Adding `xan to md`.
* Adding `xan to npy`.
* Adding `xan from npy`.
* Adding `-R/--regression-line` to `xan plot`.
* Adding `xan t` alias for `xan transpose`.
* Adding map substitution to `fmt` moonblade function.
* Adding `xan sort -C/--cells`.
* Adding `xan search --count --overlapping`.
* Adding `xan tokenize words -F/--flatmap`.

*Fixes*

* Fixing `xan search --pattern-column`.
* Fixing autocompletion with range and multiple selection.
* Fixing url highlighting in `xan view` and `xan flatten`.
* Fixing datetime highlighting in `xan view` and `xan flatten` wrt Z-terminated timestamp formats.
* Fixing `stats` date & url inference.
* Fixing moonblade support for Z-terminated timestamp formats.
* Fixing `xan plot -T` granularity inference.
* Fixing missing fractional seconds to default `datetime` serialization.
* Fixing bin allocation with `xan bins --nice`.
* Fixing `xan search --patterns -i`.
* Fixing `xan search -r -i --patterns --count` results.

*Performance*

* Optimizing aggregator memory consumption.
