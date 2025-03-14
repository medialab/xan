# Scraping cheatsheet

## Selector functions

- **first**(*css*) -> `element?`: Select the first element matching given css selection, if any.
- **all**(*css*) -> `elements`: Select all elements matching given css selection.
- **root**() -> `element`: Select the root element of document.
- **parent**() -> `element?`: Select the parent element of current selection, if any.
- **contains**(*string_or_regex*) -> `element?`: Filter the current selected elements by keeping only those containing given substring or matching given regex pattern.

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
