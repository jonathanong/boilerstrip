//! `boilerstrip` — learn site boilerplate selectors from multiple pages and
//! convert HTML to clean Markdown with them stripped.
//!
//! ## Two-step workflow
//!
//! ```rust
//! use boilerstrip::{learn, convert, LearnOptions, ConvertOptions};
//!
//! let pages = vec![
//!     "<html><nav class=\"site-nav\">Menu</nav><main>Page 1 content</main></html>".to_string(),
//!     "<html><nav class=\"site-nav\">Menu</nav><main>Page 2 content</main></html>".to_string(),
//!     "<html><nav class=\"site-nav\">Menu</nav><main>Page 3 content</main></html>".to_string(),
//! ];
//!
//! let removals = learn(&pages, &LearnOptions::default()).unwrap();
//!
//! let html = "<html><nav class=\"site-nav\">Menu</nav><main><h1>Article</h1></main></html>";
//! let result = convert(html, &ConvertOptions {
//!     removals: Some(removals),
//!     ..Default::default()
//! }).unwrap();
//!
//! assert!(result.content.contains("Article"));
//! assert!(!result.content.contains("Menu"));
//! ```

pub mod convert;
pub mod learn;
pub(crate) mod util;

// ── Headline API ─────────────────────────────────────────────────────────────
pub use convert::convert;
pub use learn::learn;

// ── Types most callers need ───────────────────────────────────────────────────
pub use convert::types::{ConvertError, ConvertOptions, ConvertResult};
pub use learn::types::{ApplyError, LearnError, LearnOptions, Removals};

// ── Low-level building blocks (re-exported for power users) ──────────────────
pub use convert::filter::{apply_text_density_filter, filter_links};
pub use convert::markdown::html_to_markdown;
pub use convert::parser::{
    extract_canonical_url, extract_lang, extract_link_tags, extract_meta_tags, extract_title,
    parse_html,
};
pub use convert::selector::{remove_by_css_selectors, remove_elements, select_content_root};
pub use learn::apply_removals;
pub use learn::default_boilerplate_patterns;
