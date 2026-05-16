# boilerstrip

[![npm](https://img.shields.io/npm/v/boilerstrip.svg)](https://www.npmjs.com/package/boilerstrip)
[![CI](https://github.com/jonathanong/boilerstrip/actions/workflows/ci.yml/badge.svg)](https://github.com/jonathanong/boilerstrip/actions/workflows/ci.yml)

Learn site boilerplate selectors from a set of HTML pages and convert HTML to clean Markdown with the boilerplate stripped.

Given multiple HTML pages from the same website, `learn` discovers which CSS selectors and HTML snippets are boilerplate (navigation, footers, cookie banners, legal disclaimers, etc.) by finding elements whose text content is stable across pages. The resulting `Removals` can then be fed into `convert` or `convertMany`, which strips the boilerplate and converts the remaining content to Markdown.

## Install

```sh
npm install boilerstrip
```

## Migration: v0.1 → v0.2

- All HTML inputs are now `Buffer` (not `string`). Pass `Buffer.from(html)` or read files with `readFileSync` (which returns Buffer by default).
- `convert` now processes a single `Buffer` and returns a `ConvertResult` object (with `content`, `title`, `lang`, `meta`, `link`, `canonicalUrl` fields).
- `convertMany` is a new batched API for converting multiple pages in one call.

## Usage

```js
import { learn, convert, convertMany } from 'boilerstrip'

// Learn boilerplate from multiple pages of the same site
const removals = await learn([page1Buffer, page2Buffer, page3Buffer])

// Convert a single page
const result = await convert(htmlBuffer, { removals })
console.log(result.content)   // Markdown string
console.log(result.title)     // <title> text

// Convert many pages in one batched call (more efficient than N convert() calls)
const results = await convertMany([html1, html2, html3], { removals })
results.forEach(r => console.log(r.content))
```

## API

### `learn(pages, options?): Promise<Removals>`

Analyzes an array of HTML pages from the same site and returns the discovered boilerplate selectors and snippets. Requires at least 2 pages.

- `pages` — `Array<Buffer>` — at least 2 HTML pages to analyze
- `options` — optional `LearnOptions`

### `convert(html, options?): Promise<ConvertResult>`

Strips boilerplate and converts a single HTML page to Markdown.

- `html` — `Buffer` — HTML page to convert
- `options` — optional `ConvertOptions`

### `convertMany(htmls, options?): Promise<ConvertResult[]>`

Batched version of `convert`. Converts multiple HTML pages in a single N-API call. More efficient than calling `convert` in a loop when processing many pages.

- `htmls` — `Array<Buffer>` — HTML pages to convert
- `options` — optional `ConvertOptions` applied to all pages

### `ConvertResult`

```ts
interface ConvertResult {
  content: string          // cleaned Markdown
  title?: string           // <title> text
  lang?: string            // <html lang="...">
  canonicalUrl?: string    // <link rel="canonical" href="...">
  meta: Record<string, string>   // <meta name/property> map
  link: Record<string, string>   // <link rel> map
}
```

### `ConvertOptions`

```ts
interface ConvertOptions {
  removals?: Removals                  // from learn()
  cssSelectorsToRemove?: string[]      // additional CSS selectors to strip
  contentSelectors?: string[]          // CSS selectors for the main content root
  linkTextContentToRemove?: string[]   // remove <a>/<button> by visible text
  linkHrefsToRemove?: string[]         // remove <a> by href prefix (e.g. "javascript:")
  linkRelTokensToRemove?: string[]     // exclude <link rel="..."> from link map
  useTextDensityFilter?: boolean       // use text-density scoring to find main content
}
```

### `Removals`

```ts
interface Removals {
  cssSelectorsToRemove: string[]
  htmlToRemove: string[]
}
```

The serializable result of `learn`. Can be stored and reused across runs.

### `LearnOptions`

```ts
interface LearnOptions {
  boilerplatePatterns?: string[]       // override built-in patterns; [] disables snippet matching
  maxSelectorMatchesPerPage?: number   // default 20
  minSelectorAverageStableRatio?: number  // default 0.6
  minSelectorPerPageStableRatio?: number  // default 0.35
  minSnippetTextLength?: number        // default 40
  maxSnippetTextLength?: number        // default 240
}
```

## License

MIT
