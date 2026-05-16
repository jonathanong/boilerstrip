//! HTML-to-Markdown conversion with configurable content extraction.
//!
//! [`convert`] processes raw HTML through a two-phase pipeline:
//!
//! 1. **Strip phase** (streaming, via `lol_html`): remove `<script>`/`<style>`
//!    and boilerplate CSS selectors in one pass.  Then text-based snippet
//!    removal (regex, O(n)).
//! 2. **DOM phase** (single scraper parse): extract metadata, filter links
//!    in-place, locate the content root, emit Markdown via a custom tree walker.
//!
//! ## Quick start
//!
//! ```rust
//! use boilerstrip::{convert, ConvertOptions};
//!
//! let html = "<html><head><title>My Page</title></head><body><h1>Hello</h1></body></html>";
//! let result = convert(html, &ConvertOptions::default());
//! assert_eq!(result.title, Some("My Page".to_string()));
//! assert!(result.content.contains("Hello"));
//! ```

pub mod filter;
pub mod markdown;
pub mod parser;
pub mod selector;
pub mod strip;
pub mod types;

pub use types::{ConvertOptions, ConvertResult};

use crate::learn::apply::apply_html_snippet_removals;

/// Convert raw HTML into Markdown with extracted metadata.
pub fn convert(html: &str, options: &ConvertOptions) -> ConvertResult {
    // Collect all CSS selectors to strip in the streaming pass.
    let mut remove_selectors: Vec<String> = Vec::new();
    if let Some(removals) = &options.removals {
        remove_selectors.extend(removals.css_selectors_to_remove.iter().cloned());
    }
    remove_selectors.extend(options.css_selectors_to_remove.iter().cloned());

    // Phase 1a — lol_html streaming pass: remove script/style + CSS selectors.
    let stripped_bytes = strip::strip_elements(html, &remove_selectors);
    // SAFETY: input was already &str (valid UTF-8); lol_html only removes whole
    // elements and their content, never splits a multi-byte sequence, so the
    // output is still valid UTF-8.
    let mut working_html = unsafe { String::from_utf8_unchecked(stripped_bytes) };

    // Phase 1b — text-based snippet removal (regex, O(n)).
    if let Some(removals) = &options.removals {
        if !removals.html_to_remove.is_empty() {
            working_html = apply_html_snippet_removals(&working_html, &removals.html_to_remove);
        }
    }

    // Phase 2 — single scraper DOM parse.
    let mut document = scraper::Html::parse_document(&working_html);

    // Extract metadata from this already-stripped DOM (title/meta/link still present).
    let title = parser::extract_title(&document);
    let meta = parser::extract_meta_tags(&document);
    let link_rel_filter = if options.link_rel_tokens_to_remove.is_empty() {
        None
    } else {
        Some(options.link_rel_tokens_to_remove.as_slice())
    };
    let link = parser::extract_link_tags(&document, link_rel_filter);
    let canonical_url = parser::extract_canonical_url(&document);
    let lang = parser::extract_lang(&document);

    // Filter links in-place (no re-parse).
    filter::filter_links_inplace(
        &mut document,
        &options.link_text_content_to_remove,
        &options.link_hrefs_to_remove,
    );

    // Select content root.
    let content_selectors_opt = if options.content_selectors.is_empty() {
        None
    } else {
        Some(options.content_selectors.as_slice())
    };
    let content_root = if options.use_text_density_filter {
        filter::apply_text_density_filter(&document)
    } else {
        selector::select_content_root(&document, content_selectors_opt)
    };

    // Emit Markdown via custom tree walker (no additional parse).
    let content = content_root
        .map(|el| markdown::element_to_markdown(el))
        .unwrap_or_default();

    ConvertResult {
        title,
        meta,
        link,
        content,
        canonical_url,
        lang,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_returns_title_and_content() {
        let html = "<html><head><title>My Page</title></head><body><h1>Hello</h1></body></html>";
        let result = convert(html, &ConvertOptions::default());
        assert_eq!(result.title, Some("My Page".to_string()));
        assert!(result.content.contains("Hello"));
    }

    #[test]
    fn convert_strips_scripts() {
        let html = "<html><body><script>evil();</script><p>Content</p></body></html>";
        let result = convert(html, &ConvertOptions::default());
        assert!(!result.content.contains("evil"));
    }

    #[test]
    fn convert_applies_learned_removals() {
        use crate::learn::types::Removals;
        let html = "<html><body><nav class=\"site-nav\">Menu</nav><main><p>Content</p></main></body></html>";
        let options = ConvertOptions {
            removals: Some(Removals {
                css_selectors_to_remove: vec![".site-nav".to_string()],
                html_to_remove: vec![],
            }),
            ..Default::default()
        };
        let result = convert(html, &options);
        assert!(!result.content.contains("Menu"));
        assert!(result.content.contains("Content"));
    }

    #[test]
    fn convert_uses_text_density_filter() {
        let html = "<html><body><nav><a href=\"/x\">Link</a></nav><article>Long prose article text that should win.</article></body></html>";
        let options = ConvertOptions {
            use_text_density_filter: true,
            ..Default::default()
        };
        let result = convert(html, &options);
        assert!(result.content.contains("prose"));
    }

    #[test]
    fn convert_extracts_canonical_url_and_lang() {
        let html = r#"<html lang="en"><head><link rel="canonical" href="https://example.com"></head><body><p>hi</p></body></html>"#;
        let result = convert(html, &ConvertOptions::default());
        assert_eq!(
            result.canonical_url,
            Some("https://example.com".to_string())
        );
        assert_eq!(result.lang, Some("en".to_string()));
    }

    #[test]
    fn convert_strips_via_css_selectors_option() {
        let html = "<html><body><nav class=\"nav\">Menu</nav><p>Content</p></body></html>";
        let options = ConvertOptions {
            css_selectors_to_remove: vec![".nav".to_string()],
            ..Default::default()
        };
        let result = convert(html, &options);
        assert!(!result.content.contains("Menu") && result.content.contains("Content"));
    }

    #[test]
    fn convert_strips_html_snippet_from_removals() {
        use crate::learn::types::Removals;
        let html =
            "<html><body><div><p>Footer text</p></div><main><p>Content</p></main></body></html>";
        let options = ConvertOptions {
            removals: Some(Removals {
                css_selectors_to_remove: vec![],
                html_to_remove: vec!["<p>Footer text</p>".to_string()],
            }),
            ..Default::default()
        };
        let result = convert(html, &options);
        assert!(!result.content.contains("Footer text") && result.content.contains("Content"));
    }

    #[test]
    fn convert_removes_links_by_href_prefix() {
        let html = "<html><body><main><a href=\"javascript:void(0)\">JS</a><a href=\"/safe\">Safe</a></main></body></html>";
        let options = ConvertOptions {
            link_hrefs_to_remove: vec!["javascript:".to_string()],
            ..Default::default()
        };
        let result = convert(html, &options);
        assert!(!result.content.contains("JS") && result.content.contains("Safe"));
    }

    #[test]
    fn convert_removes_links_by_text_content() {
        let html = "<html><body><main><a href=\"/close\">Close</a><a href=\"/keep\">Keep</a></main></body></html>";
        let options = ConvertOptions {
            link_text_content_to_remove: vec!["close".to_string()],
            ..Default::default()
        };
        let result = convert(html, &options);
        assert!(!result.content.contains("Close") && result.content.contains("Keep"));
    }

    #[test]
    fn convert_filters_link_rel_tokens() {
        let html = r#"<html><head><link rel="me" href="https://example.com/author"><link rel="canonical" href="https://example.com/page"></head><body><p>hi</p></body></html>"#;
        let options = ConvertOptions {
            link_rel_tokens_to_remove: vec!["me".to_string()],
            ..Default::default()
        };
        let result = convert(html, &options);
        assert!(result.link.get("me").is_none(), "me link should be filtered out");
        assert!(result.link.get("canonical").is_some(), "canonical link should remain");
    }

    #[test]
    fn convert_uses_content_selectors() {
        let html = "<html><body><nav>Nav</nav><article><h1>Article</h1></article></body></html>";
        let options = ConvertOptions {
            content_selectors: vec!["article".to_string()],
            ..Default::default()
        };
        let result = convert(html, &options);
        assert!(result.content.contains("Article"), "article content should be selected");
        assert!(!result.content.contains("Nav"), "nav should be excluded");
    }

    #[test]
    fn convert_text_density_filter_respects_removals() {
        use crate::learn::types::Removals;
        let footer_text = "Terms of service. ".repeat(20);
        let html = format!(
            "<html><body><footer class=\"site-footer\">{footer_text}</footer><article>Real content here</article></body></html>",
        );
        let options = ConvertOptions {
            removals: Some(Removals {
                css_selectors_to_remove: vec![".site-footer".to_string()],
                html_to_remove: vec![],
            }),
            use_text_density_filter: true,
            ..Default::default()
        };
        let result = convert(&html, &options);
        assert!(!result.content.contains("Terms of service"));
        assert!(result.content.contains("Real content"));
    }
}
