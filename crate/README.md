# boilerstrip

Learn site boilerplate selectors from a set of HTML pages and convert HTML to clean Markdown with the boilerplate stripped.

## What it does

Given multiple HTML pages from the same website, `learn` discovers which CSS selectors and HTML snippets are boilerplate (navigation, footers, cookie banners, legal disclaimers, etc.) by finding elements whose text content is stable across pages.  The resulting `Removals` can then be fed into `convert`, which strips the boilerplate and converts the remaining content to Markdown.

```rust
use boilerstrip::{learn, convert, LearnOptions, ConvertOptions};

let pages = vec![
    fetch("https://example.com/page1").await?,
    fetch("https://example.com/page2").await?,
    fetch("https://example.com/page3").await?,
];

// Learn which selectors are boilerplate.
let removals = learn(&pages, &LearnOptions::default())?;

// Convert a page to Markdown with boilerplate stripped.
let options = ConvertOptions {
    removals: Some(removals),
    ..Default::default()
};
let result = convert(&html, &options);
println!("{}", result.content);
```

## API

### Headline functions

```rust
pub fn learn(pages: &[String], options: &LearnOptions) -> Result<Removals, LearnError>;
pub fn convert(html: &str, options: &ConvertOptions) -> ConvertResult;
pub fn apply_removals(html: &str, removals: &Removals) -> String;
```

### Types

```rust
pub struct Removals {
    pub css_selectors_to_remove: Vec<String>,
    pub html_to_remove: Vec<String>,
}

pub struct LearnOptions {
    pub boilerplate_patterns: Option<Vec<String>>,
    // Tuning knobs (all optional, default to built-in values):
    pub max_selector_matches_per_page: Option<usize>,      // default: 20
    pub min_selector_average_stable_ratio: Option<f64>,    // default: 0.6
    pub min_selector_per_page_stable_ratio: Option<f64>,   // default: 0.35
    pub min_snippet_text_length: Option<usize>,            // default: 40
    pub max_snippet_text_length: Option<usize>,            // default: 240
}

pub struct ConvertOptions {
    pub removals: Option<Removals>,
    pub css_selectors_to_remove: Option<Vec<String>>,
    pub content_selectors: Option<Vec<String>>,
    pub link_text_content_to_remove: Option<Vec<String>>,
    pub link_hrefs_to_remove: Option<Vec<String>>,
    pub link_rel_tokens_to_remove: Option<Vec<String>>,
    pub use_text_density_filter: Option<bool>,
}

pub struct ConvertResult {
    pub title: Option<String>,
    pub lang: Option<String>,
    pub canonical_url: Option<String>,
    pub meta: serde_json::Map<String, serde_json::Value>,
    pub links: serde_json::Map<String, serde_json::Value>,
    pub content: String,
}
```

### Internals feature

Internal building blocks are available with the `internals` feature for power users who need direct access to the parsing, removal, and Markdown pipelines:

```toml
[dependencies]
boilerstrip = { version = "0.1", features = ["internals"] }
```

This exposes `convert::filter`, `convert::parser`, `convert::selector`, and `convert::markdown` re-exports. These APIs are not subject to stability guarantees outside of major version bumps.

## How the learning algorithm works

1. **Selector stage** — parses each page and collects CSS selectors (id, class, role) for every element.  Scores each selector by how stable its content fingerprint is across pages.  High-stability selectors become removal candidates.

2. **Snippet stage** — breadth-first traversal looking for text nodes that match boilerplate patterns (legal notices, cookie banners, sign-in prompts, etc.) but lack stable selectors.  Matching HTML is stored verbatim for flexible whitespace-tolerant removal.

## License

MIT
