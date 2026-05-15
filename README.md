# boilerstrip

[![CI](https://github.com/jonathanong/boilerstrip/actions/workflows/ci.yml/badge.svg)](https://github.com/jonathanong/boilerstrip/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/jonathanong/boilerstrip/graph/badge.svg)](https://codecov.io/gh/jonathanong/boilerstrip)
[![Crates.io](https://img.shields.io/crates/v/boilerstrip.svg)](https://crates.io/crates/boilerstrip)
[![npm](https://img.shields.io/npm/v/boilerstrip.svg)](https://www.npmjs.com/package/boilerstrip)

Learn site boilerplate selectors from a set of HTML pages and convert HTML to clean Markdown with the boilerplate stripped.

## What it does

Given multiple HTML pages from the same website, `learn` discovers which CSS selectors and HTML snippets are boilerplate (navigation, footers, cookie banners, legal disclaimers, etc.) by finding elements whose text content is stable across pages. The resulting `Removals` can then be fed into `convert`, which strips the boilerplate and converts the remaining content to Markdown.

## Usage

### Rust

```toml
[dependencies]
boilerstrip = "0.1"
```

```rust
use boilerstrip::{learn, convert, LearnOptions, ConvertOptions};

let pages = vec![
    fetch("https://example.com/page1").await?,
    fetch("https://example.com/page2").await?,
    fetch("https://example.com/page3").await?,
];

let removals = learn(&pages, &LearnOptions::default())?;

let options = ConvertOptions {
    removals: Some(removals),
    ..Default::default()
};
let result = convert(&html, &options)?;
println!("{}", result.content);
```

See [`crate/README.md`](crate/README.md) for the full API reference.

### Node.js

```sh
npm install boilerstrip
```

```js
import { learn, convert } from 'boilerstrip'

const removals = learn(pages)
const markdown = convert(html, removals)
```

## Repository layout

```
crate/      Rust crate (publishable to crates.io)
package/    Node N-API package (publishable to npm)
fixtures/   Hand-written HTML test fixtures
```

## License

MIT
