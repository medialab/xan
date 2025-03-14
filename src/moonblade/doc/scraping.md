# Scraping cheatsheet

`xan` scraping language should be very reminiscent of CSS/SCSS syntax, as
it follows the same selection principles (it is probably useful, when
using --evaluate-file, to save your scrapers on disk using the `.css` or
`.scss` extension to get proper syntax highlighting).

This language is able to:

1. perform complex element selection using nested CSS selectors
and/or custom expression
2. to extract and process data from selected elements

For instance, here is a simple example selecting links contained in a
h2 tag:

```scss
h2 > a {
  title: text;
  url: attr("href");
}
```

The above scraper will then extract a "title" column containing the text
of selected tag and a "url" column containing its "href" attribute value.

todo: complex selection, extractor, processing, & scope