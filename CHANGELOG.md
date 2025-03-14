# Changelog

## 0.47.0 (provisional)

*Features*

* Adding moonblade function `to_timezone` and `to_local_timezone`
* Moonblade `strftime()` and all other date formatting functions such as `ymd()` do not support timezones any more.
* Improving `xan help cheatsheet`.
* Adding moonblade function `try`.
* Adding moonblade functions `int` & `float`.
* Adding moonblade functions `lru` & `urljoin`.
* `moonblade` now tolerates line-breaks as whitespace when parsing. This makes it possible to write your expressions on multiple lines.
* `moonblade` now accepts comments starting with `#`.
* Adding `xan scrape` command.
* Adding `xan help scraping` subcommand.
* Adding moonblade function `html_unescape`.

*Fix*

* Fixing `xan flatten -w` wrt line breaks.
* Fixing underscore expansion when contained in map & list expressions.

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
