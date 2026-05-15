//! HTML-to-Markdown conversion with configurable content extraction.
//!
//! [`convert`] processes raw HTML through a multi-stage pipeline:
//!
//! 1. Extract metadata (title, meta tags, link tags, canonical URL, lang).
//! 2. Remove `<script>` and `<style>` tags.
//! 3. Optionally apply learned [`Removals`][crate::learn::types::Removals] and extra CSS selectors.
//! 4. Locate the main content root (text-density, user selectors, or semantic elements).
//! 5. Filter unwanted links.
//! 6. Convert cleaned HTML to Markdown.
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
pub mod types;

pub use types::{ConvertOptions, ConvertResult};

use crate::learn::apply_removals;

/// Convert raw HTML into Markdown with extracted metadata.
pub fn convert(html: &str, options: &ConvertOptions) -> ConvertResult {
    // Step 1: Extract metadata from the original HTML before any content removal.
    let document = parser::parse_html(html);
    let title = parser::extract_title(&document);
    let meta = parser::extract_meta_tags(&document);
    let link = parser::extract_link_tags(&document, options.link_rel_tokens_to_remove.as_deref());
    let canonical_url = parser::extract_canonical_url(&document);
    let lang = parser::extract_lang(&document);

    // Step 2: Remove scripts and styles.
    let cleaned_html = selector::remove_elements(html, &["script", "style"]);

    // Step 3: Apply learned removals + extra CSS selectors.
    let mut working_html = cleaned_html;

    if let Some(removals) = &options.removals {
        working_html = apply_removals(&working_html, removals);
    }

    working_html = selector::remove_by_css_selectors(
        &working_html,
        options.css_selectors_to_remove.as_deref(),
    );

    // Step 4: Find the main content root. Re-parse from working_html so removals are reflected.
    let working_document = parser::parse_html(&working_html);
    working_html = if let Some(true) = options.use_text_density_filter {
        filter::apply_text_density_filter(&working_document)
            .map(|el| el.html())
            .unwrap_or(working_html)
    } else {
        selector::select_content_root(&working_document, options.content_selectors.as_deref())
            .expect("BUG: parsed HTML document should always yield a content root")
            .html()
    };

    // Step 5: Filter unwanted links.
    working_html = filter::filter_links(
        &working_html,
        options.link_text_content_to_remove.as_deref(),
        options.link_hrefs_to_remove.as_deref(),
    );

    // Step 6: Convert to Markdown.
    let content = markdown::html_to_markdown(&working_html);

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
            use_text_density_filter: Some(true),
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
    fn convert_text_density_filter_respects_removals() {
        // Boilerplate footer is long enough to win density scoring without removals.
        // With both use_text_density_filter and a CSS removal selector, the boilerplate
        // must be absent (regression test for the ordering bug in step 4).
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
            use_text_density_filter: Some(true),
            ..Default::default()
        };
        let result = convert(&html, &options);
        assert!(!result.content.contains("Terms of service"));
        assert!(result.content.contains("Real content"));
    }
}
