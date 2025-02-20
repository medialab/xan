# Changelog

## 0.46.0 (provisional)

*Breaking*

* Adding fractional seconds to default `datetime` serialization.

*Features*

* Adding `xan to html`.
* Adding `xan to md`.
* Adding `xan to npy`.
* Adding `xan from npy`.
* Adding `-R,--regression-line` to `xan plot`.
* Adding `xan t` alias for `xan transpose`.

*Fixes*

* Fixing `xan search --pattern-column`.
* Fixing autocompletion with range and multiple selection.
* Fixing url highlighting in `xan view` and `xan flatten`.
* Fixing datetime highlighting in `xan view` and `xan flatten` wrt Z-terminated timestamp formats.
* Fixing `stats` date & url inference.
* Fixing moonblade support for Z-terminated timestamp formats.
* Fixing `xan plot -T` granularity inference.

*Performance*

* Optimizing aggregator memory consumption.