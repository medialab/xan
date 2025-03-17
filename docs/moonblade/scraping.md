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

## Selector functions

- **first**(*css*, *containing=pattern?*) -> `element?`: Select the first element matching given css selection, if any.
- **all**(*css*, *containing=pattern?*) -> `elements`: Select all elements matching given css selection.
- **root**() -> `element`: Select the root element of document.
- **parent**() -> `element?`: Select the parent element of current selection, if any.

## Extractor functions

- **name** -> `string`: Extract the HTML tag name of selected element.
- **id** -> `string?`: Extract the id attribute of selected element, if any.
- **attr**(*name*) -> `string?`: Extract desired attribute of selected element, if it exists.
- **attrs** -> `map`: Extract a map of selected element's attributes.
- **classes** -> `list`: Extract a list of selected element's classes.
- **text** -> `string`: Extract a readable version of selected element's text, all while attempting to respect inline vs. block display.
- **raw_text** -> `string`: Extract selected element's text, without any reformatting except from trimming.
- **inner_html** -> `string`: Extract selected element's inner HTML.
- **outer_html** -> `string`: Extract selected element's outer HTML.
- **json** -> `any`: Parse selected element's text as JSON data.
- **json_ld**(*type*) -> `map`: Parse selected element's text as JSON data, then attempt to find the JSON-LD item matching given type.
