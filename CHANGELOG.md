# Changelog

## 0.51.0 (provisional)

*Breaking*

* Dropping undocumented `xan index` and related interactions (in `xan count`, `xan sample`, `xan slice` & `xan split --jobs`).
* Dropping now useless `coalesce` moonblade function.
* `xan split` now accepts its output directory as an optional flag.
* `xan partition` now accepts its output directory as an optional flag.
* `xan split -s` becomes `xan split -S` to avoid confusion with the `-s/--select` flag used everywhere else.
* Dropping useless `xan count --csv` flag.

*Features*

* Adding `xan count -a/--approx`.
* Adding `xan slice -E/--end-byte`.
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
* Adding `xan parallel -F/--single-file`.
* Adding `xan from md`.
* Adding `xan parallel map`.
* Adding `xan parallel -p/--parallel -t/--threads`.

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
* Adding missing subcommand completions for `xan parallel`.
* Better default threads count heuristics.

*Performance*

* Switching hashmaps to `ahash`.

*Quality of Life*

* Prepending xan subcommand to error messages.
* Better error messages when moonblade expressions cannot be parsed.
* Displaying number of threads actually used when using `xan parallel`.

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
