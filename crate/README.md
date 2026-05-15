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
let result = convert(&html, &options)?;
println!("{}", result.content);
```

## API

### Headline functions

```rust
pub fn learn(pages: &[String], options: &LearnOptions) -> Result<Removals, LearnError>;
pub fn convert(html: &str, options: &ConvertOptions) -> Result<ConvertResult, ConvertError>;
pub fn apply_removals(html: &str, removals: &Removals) -> Result<String, ApplyError>;
```

### Types

```rust
pub struct Removals {
    pub css_selectors_to_remove: Vec<String>,
    pub html_to_remove: Vec<String>,
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

### Low-level building blocks

The following are also re-exported for power users:

```rust
// Metadata extraction
pub use convert::parser::{parse_html, extract_title, extract_meta_tags,
    extract_link_tags, extract_canonical_url, extract_lang};

// Element removal and content selection
pub use convert::selector::{remove_elements, remove_by_css_selectors, select_content_root};

// Text density and link filtering
pub use convert::filter::{apply_text_density_filter, filter_links};

// HTML → Markdown
pub use convert::markdown::html_to_markdown;

// Default boilerplate patterns
pub use learn::default_boilerplate_patterns;
```

## How the learning algorithm works

1. **Selector stage** — parses each page and collects CSS selectors (id, class, role) for every element.  Scores each selector by how stable its content fingerprint is across pages.  High-stability selectors become removal candidates.

2. **Snippet stage** — breadth-first traversal looking for text nodes that match boilerplate patterns (legal notices, cookie banners, sign-in prompts, etc.) but lack stable selectors.  Matching HTML is stored verbatim for flexible whitespace-tolerant removal.

## License

MIT
