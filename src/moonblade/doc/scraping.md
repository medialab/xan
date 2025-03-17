# Scraping cheatsheet

`xan` scraping language should be very reminiscent of CSS/SCSS syntax, as
it follows the same selection principles (it is probably useful, when
using --evaluate-file, to save your scrapers on disk using the `.css`,
`.sass` or `.scss`, extension to get proper syntax highlighting).

This language is able to:

1. perform complex element selection using nested CSS selectors
and/or custom expressions
2. to extract and process data from selected elements

For instance, here is a simple example selecting links contained in a
h2 tag:

```scss
h2 > a {
  title: text;
  url: attr("href");
}
```

The above scraper will extract a "title" column containing the text
of selected tag and a "url" column containing its "href" attribute value.

Each inner directive is understood as:

`<column-name>: <extractor-function>;`

A full list of extractor functions can be found at the end of this help.

And processing using a moonblade expression taking `value` as the extractor
function's output value is also possible:

```scss
h2 > a {
  title: text, lower(value)[10:];
  url: attr("href");
}
```

In which case, inner directives will be understood as:

`<column-name>: <extractor-function>, <processing-expression>;`

Selections can be nested:

```scss
.main-content {
  h2 {
    title: text;
  }

  a.main-link {
    url: attr("href");
  }
}
```

Multiple selection rules can be given per scraper:

```scss
[data-id=45] {
  title: text;
}

script[type="application/ld+json"] {
  data: json_ld("NewsArticle");
}
```

Selection can use expressions to navigate freely through the DOM (see
a comprehensive list of all selector functions at the end of this help):

```scss
first("h2", containing="Summary").parent() {
  title: text;
}

main > p {
  all("a") {
    urls: attr("href");
  }
}
```

`:scope` or `&` can be used to ease nested selection (see how we are able
to select direct children of `main > p`):

```scss
main > p {
  & > a {
    url: attr("href");
  }
}
```

`:scope` and `&` are also useful when using `xan scrape --foreach`
because we sometimes need a way to select from the scope of an already
selected element.

The following example assumes we gave --foreach "h2 > a" to `xan scrape`:

```scss
& {
  title: text;
  url: attr("href");
}
```

For more examples of real-life scrapers, check out this link:
https://github.com/medialab/xan/tree/master/docs/scrapers