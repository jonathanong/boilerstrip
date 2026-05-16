# boilerstrip

[![npm](https://img.shields.io/npm/v/boilerstrip.svg)](https://www.npmjs.com/package/boilerstrip)
[![CI](https://github.com/jonathanong/boilerstrip/actions/workflows/ci.yml/badge.svg)](https://github.com/jonathanong/boilerstrip/actions/workflows/ci.yml)

Learn site boilerplate selectors from a set of HTML pages and convert HTML to clean Markdown with the boilerplate stripped.

Given multiple HTML pages from the same website, `learn` discovers which CSS selectors and HTML snippets are boilerplate (navigation, footers, cookie banners, legal disclaimers, etc.) by finding elements whose text content is stable across pages. The resulting `Removals` can then be fed into `convert`, which strips the boilerplate and converts the remaining content to Markdown.

## Install

```sh
npm install boilerstrip
```

## Usage

```js
import { learn, convert } from 'boilerstrip'

// pages and htmls can be Buffer[] or string[] (or mixed)
const removals = await learn(pages)               // Promise<Removals>
const markdowns = await convert(htmls, removals)  // Promise<Buffer[]>

// Synchronous variants
import { learnSync, convertSync } from 'boilerstrip'
const removals = learnSync(pages)
const markdowns = convertSync(htmls, removals)
```

## API

### `learn(pages): Promise<Removals>`

Analyzes an array of HTML pages from the same site and returns the discovered boilerplate selectors and snippets.

- `pages` — `Array<Buffer | string>` — HTML pages to analyze

### `learnSync(pages): Removals`

Synchronous version of `learn`.

### `convert(htmls, removals?): Promise<Buffer[]>`

Strips boilerplate and converts HTML to Markdown.

- `htmls` — `Array<Buffer | string>` — HTML pages to convert
- `removals` — optional `Removals` from `learn`; if omitted, converts without stripping

Returns `Buffer[]` — one Markdown buffer per input.

### `convertSync(htmls, removals?): Buffer[]`

Synchronous version of `convert`.

### `Removals`

```ts
interface Removals {
  cssSelectorsToRemove: string[]
  htmlToRemove: string[]
}
```

The serializable result of `learn`. Can be stored and reused across runs.

## License

MIT
