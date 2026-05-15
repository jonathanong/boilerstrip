# Test Fixtures

Hand-written HTML fixtures used by both Rust integration tests (`crate/tests/`) and Node tests (`package/__tests__/`).

## `learn/site_a/`

Three pages from a fictional site that share an identical `<nav class="site-nav">` and `<footer id="site-footer">` across all pages.  Each page has a unique article body.

`expected.json` — the `Removals` that `learn` should produce: selectors `.site-nav` and `#site-footer`.

## `learn/site_b/`

Two pages sharing a fixed `<header id="header">` and an `<aside>` element containing privacy/legal boilerplate text (no stable selector).  The aside is matched via the snippet path.

`expected.json` — the `Removals` that `learn` should produce: selector `#header` and the aside HTML snippet.

## `convert/`

Single-page conversion fixtures:

| File | Description |
|------|-------------|
| `basic_article.html` | Bare article with heading, paragraphs, a list, and a link |
| `basic_article.expected.md` | Expected Markdown output |
| `with_meta.html` | Article with `<title>`, `<meta>`, `<link rel="canonical">`, and `lang` attribute |
| `with_meta.expected.json` | Expected `ConvertResult` fields (title, lang, canonical_url, meta, content) |
| `tables_and_lists.html` | Article with a table and nested lists |
| `tables_and_lists.expected.md` | Expected Markdown output |

## `end_to_end/`

Full `learn → convert` pipeline fixture.

`pages/` — three pages from a fictional site with a shared header and footer boilerplate.

`expected.md` — the Markdown produced by running `learn` on all pages, then `convert` on a fourth page with those removals applied.
