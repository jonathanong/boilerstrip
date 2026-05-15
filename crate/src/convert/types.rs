use serde_json::{Map, Value};

use crate::learn::types::Removals;

/// Configuration for HTML-to-Markdown conversion.
#[derive(Clone, Debug, Default)]
pub struct ConvertOptions {
    /// Boilerplate removals learned from a set of pages; applied before conversion.
    pub removals: Option<Removals>,
    /// CSS selectors whose matching elements are removed before conversion.
    pub css_selectors_to_remove: Option<Vec<String>>,
    /// CSS selectors that identify the main content root (first match wins).
    pub content_selectors: Option<Vec<String>>,
    /// Link visible-text patterns whose matching `<a>`/`<button>` elements are removed.
    pub link_text_content_to_remove: Option<Vec<String>>,
    /// Link href prefixes whose matching elements are removed (e.g. `"javascript:"`).
    pub link_hrefs_to_remove: Option<Vec<String>>,
    /// `<link rel="...">` tokens to exclude from the extracted `link` map.
    pub link_rel_tokens_to_remove: Option<Vec<String>>,
    /// When `true`, use text-density scoring to locate the main content element
    /// instead of CSS selectors or semantic elements.
    pub use_text_density_filter: Option<bool>,
}

/// The result of converting a single HTML page.
#[derive(Clone, Debug)]
pub struct ConvertResult {
    /// Page title from `<title>`.
    pub title: Option<String>,
    /// `<meta name/property>` map.
    pub meta: Map<String, Value>,
    /// `<link rel>` map.
    pub link: Map<String, Value>,
    /// Cleaned Markdown content.
    pub content: String,
    /// Canonical URL from `<link rel="canonical">`.
    pub canonical_url: Option<String>,
    /// Language code from `<html lang="...">`.
    pub lang: Option<String>,
}
