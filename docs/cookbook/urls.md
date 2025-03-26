# Joining files by URL prefixes

- [Use case](#use-case)
- [Basic `xan url-join` usage](#basic-xan-url-join-usage)
- [Keeping only certain columns from second file in final output](#keeping-only-certain-columns-from-second-file-in-final-output)
- [What to do when entities are symbolized by multiple urls](#what-to-do-when-entities-are-symbolized-by-multiple-urls)
- [Keeping rows from first file even if they do not match](#keeping-rows-from-first-file-even-if-they-do-not-match)
- [Sidestepping issues related to http/https or www](#sidestepping-issues-related-to-httphttps-or-www)
- [What to do when you only want to filter the file](#what-to-do-when-you-only-want-to-filter-the-file)
- [The difficulty of joining files by urls](#the-difficulty-of-joining-files-by-urls)

## Use case

In this guide I will show you how to use the `xan url-join` command to "join" (i.e. match rows) two CSV files containing urls.

The usecase is the following: let's say on the one hand you are interested by online media websites and you have a CSV file listing those medias along with some useful metadata:

| id  | homepage                        | media            | politics |
| --- | ------------------------------- | ---------------- | -------- |
| 1   | https://www.lemonde.fr          | lemonde          | center   |
| 2   | https://www.lefigaro.fr         | lefigaro         | right    |
| 3   | https://www.liberation.fr       | liberation       | left     |
| 4   | https://www.lemonde.fr/economie | lemonde-business | right    |
| ... | ...                             | ...              | ...      |

On the other hand, you collected many tweets while researching the subject of online polarization. And sometimes those tweets may mention urls. As such, to study how Twitter users are referencing your list of online medias, you created a second CSV file such as each line is representing one Twitter user mentioning a given url in one of their tweets. It could look like this:

| tweet_id | twitter_user | url                                                             |
| -------- | ------------ | --------------------------------------------------------------- |
| 1        | @johnjohn    | https://www.lemonde.fr/planete/article/2021/02/23/covid...      |
| 2        | @jackie      | https://www.lefigaro.fr/flash-actu/le-zoo-de-lille...           |
| 3        | @mary        | https://www.liberation.fr/societe/sante/apres-la-vaccination... |
| ...      | ...          | ...                                                             |

But now if you want to be able to answer whether your users share more right-wing or left-wing media articles, for instance, you will need to find a way to match lines from your second file to the correct ones from the first one so we can obtain this result in the end:

| tweet_id | twitter_user | url                                                             | politics |
| -------- | ------------ | --------------------------------------------------------------- | -------- |
| 1        | @johnjohn    | https://www.lemonde.fr/planete/article/2021/02/23/covid...      | center   |
| 2        | @jackie      | https://www.lefigaro.fr/flash-actu/le-zoo-de-lille...           | right    |
| 3        | @mary        | https://www.liberation.fr/societe/sante/apres-la-vaccination... | left     |
| ...      | ...          | ...                                                             | ...      |

## Basic `xan url-join` usage

Fortunately `xan` do this from the comfort of the command line (note that doing so correctly and efficiently is not completely straightforward and you can read more about it [at the end of this document](#the-difficulty-of-joining-files-by-urls) if you wish).

```bash
xan url-join url tweets.csv homepage medias.csv > joined.csv
```

Do so and you will get a `joined.csv` file with same columns as `tweets.csv` with 4 new columns `id`, `media`, `homepage` and `politics`, filled with relevant info from the `medias.csv` file when a match was found.

The order in which the arguments are to be given can be hard to remember, so be sure to try `xan`'s help before doing things in reverse:

```bash
xan url-join -h
```

## Keeping only certain columns from second file in final output

In some cases, you may want to avoid copying all the columns from the second file into the output when matching. If so, a combination of the `-L/--prefix-left`, `-R/--prefix-right` and piping the result into `xan select` or `xan drop` should help you produce the correct output:

```bash
# `:2` in the selection means "first columns up to column with index 2, inclusive",
# or said differently "the first three column",
xan url-join url tweets.csv homepage medias.csv -R media_ | xan select :2,media_politics > joined.csv
```

and you will get:

| tweet_id | twitter_user | url                                                             | media_politics |
| -------- | ------------ | --------------------------------------------------------------- | -------------- |
| 1        | @johnjohn    | https://www.lemonde.fr/planete/article/2021/02/23/covid...      | center         |
| 2        | @jackie      | https://www.lefigaro.fr/flash-actu/le-zoo-de-lille...           | right          |
| 3        | @mary        | https://www.liberation.fr/societe/sante/apres-la-vaccination... | left           |
| ...      | ...          | ...                                                             | ...            |

and now we can compute and visualize a frequency table of the `media_politics` seen across all the shared links of the dataset like so:

```bash
xan freq -s media_politics joined.csv | xan hist
```

```
Histogram for media_politics (bars: 3, sum: 3,448, max: 1,722):

center |1,722  49.94%|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■|
right  |  928  26.91%|■■■■■■■■■■■■■■■■■■■■■■■■■                     |
left   |  798  23.14%|■■■■■■■■■■■■■■■■■■■■■■                        |
```

## What to do when entities are symbolized by multiple urls

Often you will find that a single url is not enough to delimit an interesting "entity" you would want to study as a whole. For instance, you may want to assert that any url of a tweet posted by Le Monde's Twitter account should be associated to the media, as well as any article on their website. But to do so, Le Monde's homepage is not sufficient to symbolize it since you now require at least two urls to define the boundaries of your entity: https://www.lemonde.fr/ and https://twitter.com/lemondefr.

So that both those urls:

- https://www.lemonde.fr/pixels/article/2021/02/23/jeu-video-entre-suite-et-reedition-le-retour-sur-le-devant-de-la-scene-de-diablo_6070953_4408996.html
- https://twitter.com/lemondefr/status/1364248725661564928

can be matched to this same "Le Monde" entity.

This is not an uncommon approach and multiple tools, such as our lab's web crawler [Hyphe](https://hyphe.medialab.sciences-po.fr/) will keep multiple url prefixes per entity.

The most natural way to handle this is of course to have multiple lines per media in our first file like so:

| id  | prefix                        | media   | politics |
| --- | ----------------------------- | ------- | -------- |
| 1   | https://www.lemonde.fr        | lemonde | center   |
| 1   | https://twitter.com/lemondefr | lemonde | center   |
| ... | ...                           | ...     | ...      |

As such, the same metadata can be accessed through different urls.

But you may also prefer keeping both urls in the same CSV line and to do so, people often keep them in a single cell, separated by a specific character such as `|`, `,` or just a simple whitespace, for instance.

To handle this, `xan` can be told to "explode" the file before passing it to `url-join` downstream so you can represent your entities thusly:

| id  | prefixes                                             | media   | politics |
| --- | ---------------------------------------------------- | ------- | -------- |
| 1   | https://www.lemonde.fr https://twitter.com/lemondefr | lemonde | center   |
| ... | ...                                                  | ...     | ...      |

You can then pipe `xan explode` into `xan url-join` to consider each prefix as a distinct row:

```bash
# Note how "-" is meant to represent stdin in the url-join command
xan explode prefixes --sep " " --rename prefix medias.csv | \
xan url-join prefix - url tweets.csv > joined.csv
```

## Keeping rows from first file even if they do not match

Sometimes you might want to keep rows from the first file even if no url prefix from the second file matched. In SQL join terminology, this would be named a "left" join. This can be done with the `--left` flag. So given this file:

| tweet_id | twitter_user | url                           |
| -------- | ------------ | ----------------------------- |
| 1        | @johnjohn    | https://www.lemonde.fr/...    |
| 2        | @gary        | https://www.mediapart.fr/...  |
| 3        | @jackie      | https://www.lefigaro.fr/...   |
| 4        | @mary        | https://www.liberation.fr/... |
| 5        | @lucy        | https://msn.com/...           |
| ...      | ...          | ...                           |

and our medias file here:

| id  | homepage                        | media            | politics |
| --- | ------------------------------- | ---------------- | -------- |
| 1   | https://www.lemonde.fr          | lemonde          | center   |
| 2   | https://www.lefigaro.fr         | lefigaro         | right    |
| 3   | https://www.liberation.fr       | liberation       | left     |
| 4   | https://www.lemonde.fr/economie | lemonde-business | right    |
| ... | ...                             | ...              | ...      |

a left join using the following command:

```bash
xan url-join url tweets.csv homepage medias.csv --left | xan select tweet_id,media > joined.csv
```

would produce:

| tweet_id | media      |
| -------- | ---------- |
| 1        | lemonde    |
| 2        |            |
| 3        | lefigaro   |
| 4        | liberation |
| 5        |            |
| ...      | ...        |

## Sidestepping issues related to http/https or www

Differences between `http` & `https` and the presence of other url details irrelevant for prefix matching can sometimes cause issues. Fortunately, `xan url-join` has a `-S/--simplified` flag that will sidestep this issue by ignoring url scheme and usually irrelevant parts such as `www` subdomains, ports, user auth etc.

What's more, `xan` commands do not care whether url scheme is present. Which means given a slighlty altered medias file like this one:

| id  | homepage            | media            | politics |
| --- | ------------------- | ---------------- | -------- |
| 1   | lemonde.fr          | lemonde          | center   |
| 2   | lefigaro.fr         | lefigaro         | right    |
| 3   | liberation.fr       | liberation       | left     |
| 4   | lemonde.fr/economie | lemonde-business | right    |
| ... | ...                 | ...              | ...      |

we could match our tweets with the following command:

```bash
xan url-join url tweets.csv homepage medias.csv -S > joined.csv
```

## What to do when you only want to filter the file

Sometimes you don't care about media metadata, since you only want to filter your tweets to keep the ones sharing a url matching a single prefix or a bunch of them at once. Know that you can use the `xan search -u/--url-prefix` command instead for those cases:

```bash
# Searching for all tweets sharing a link from "Le Monde":
xan search -s url -u lemonde.fr tweets.csv > le-monde-tweets.csv
# Searching for all tweets sharing a link from any media in our list
xan search -s url -u --patterns medias.csv --pattern-column homepage > filtered-tweets.csv
```

---

<p align="center">⁂</p>

## The difficulty of joining files by urls

Matching urls correctly is not as easy as it first seems.

A naive approach could be to match urls if they share a common prefix, or based on their domain name, for instance. But as you will quickly discover when working with web data and more specifically urls, nothing is ever straightforward and everything requires a pinch of craft and tricks.

Indeed, urls are an incredibly messy way of conveying a sense of content hierarchy and you might stumble upon various issues preventing you from considering urls as sound hierarchical sequences because web actors (such as a media) tend to live on multiple domains and/or websites at once.

So, in order to correctly match urls, what you really need is first to reorder urls as meaningful hierarchical sequences of their parts (e.g. their domain, path, query, fragment etc.). We personally use a method to do so that produce sequences we like to call LRUs, as a pun (a reverse URL). You can read more about this [here](https://github.com/medialab/ural#lru-explanation).

Equipped with this method producing truly hierarchical sequences from urls and a specialized [prefix tree](https://en.wikipedia.org/wiki/Trie), we can now match urls as being the longest matching prefix of the ones we indexed in our tree.

To illustrate this, let's consider those online "medias":

1. Le Monde: https://www.lemonde.fr
2. Le Monde business section: https://www.lemonde.fr/economie/ (because we want to keep it as a separate entity in our aggregations)
3. Le Figaro: https://www.lefigaro.fr
4. Libération: https://www.liberation.fr

Then this url: https://www.lefigaro.fr/flash-actu/le-zoo-de-lille-accueille-un-jeune-panda-roux-male-pour-tenter-la-reproduction-20210223 should of course match `Le Figaro`

This url: https://www.lemonde.fr/idees/article/2021/02/23/macron-se-retrouve-face-a-un-double-defi-s-inventer-un-bilan-et-au-dela-tracer-des-voies-pour-2022_6070906_3232.html should match `Le Monde`

But this next url: https://www.lemonde.fr/economie/article/2021/02/23/les-lits-inexploites-mal-persistant-du-tourisme-de-montagne_6070899_3234.html should match `Le Monde business` and not `Le Monde`!

Finally, note that in our current example, the https://www.mediapart.fr/journal/france/180321/les-gobelins-une-institution-royale-l-heure-neoliberale url should not match anything.

On top of this, you can sprinkle other issues related to url having parts which are not useful to determine whether they point to the same resource or not (such as `www`, `http` vs. `https` or those pesky `?xtor` query items you can easily find attached to urls found on the web so SEO people can track what you do and where you come from...) and you have yourself quite a challenge if you want to reliably join files by the urls they contain!
