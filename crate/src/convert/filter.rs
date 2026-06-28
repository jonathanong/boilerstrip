use scraper::{ElementRef, Html, Selector};
use std::sync::LazyLock;

static SELECTOR_MAIN_ARTICLE_SECTION_DIV: LazyLock<Vec<Selector>> = LazyLock::new(|| {
    ["main", "article", "section", "div"]
        .iter()
        .filter_map(|s| Selector::parse(s).ok())
        .collect()
});

static SELECTOR_A: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("a").expect("BUG: invalid SELECTOR_A"));

static SELECTOR_A_BUTTON: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("a, button").expect("BUG: invalid SELECTOR_A_BUTTON"));

static SELECTOR_IMG: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("img").expect("BUG: invalid SELECTOR_IMG"));

/// Penalty applied per link in text density calculation.
const TEXT_DENSITY_LINK_PENALTY: i32 = 20;

/// Find the element with the highest text density score across `<main>`,
/// `<article>`, `<section>`, and `<div>` elements.
///
/// Score = total text length − link text length − (link count × 20).
///
/// Note: candidates include overlapping ancestors and descendants; outer
/// wrappers tend to win when they have the most total text.
pub fn apply_text_density_filter(document: &Html) -> Option<ElementRef<'_>> {
    SELECTOR_MAIN_ARTICLE_SECTION_DIV
        .iter()
        .flat_map(|selector| document.select(selector))
        .map(|el| {
            let score = calculate_text_density_score(&el);
            (el, score)
        })
        .reduce(|(best, best_score), (el, score)| {
            if score > best_score {
                (el, score)
            } else {
                (best, best_score)
            }
        })
        .map(|(el, _)| el)
}

fn calculate_text_density_score(element: &ElementRef) -> i32 {
    let text: String = element.text().collect();
    let text_length = text.len();

    let mut link_text_length = 0;
    let mut link_count = 0;
    for link in element.select(&SELECTOR_A) {
        let link_text: String = link.text().collect();
        link_text_length += link_text.len();
        link_count += 1;
    }

    compute_density_score(text_length, link_text_length, link_count)
}

fn compute_density_score(text_len: usize, link_text_len: usize, link_count: usize) -> i32 {
    let raw_score = text_len as i128
        - link_text_len as i128
        - (link_count as i128 * TEXT_DENSITY_LINK_PENALTY as i128);

    raw_score.clamp(i32::MIN as i128, i32::MAX as i128) as i32
}

/// Remove `<a>` and `<button>` elements in-place on an already-parsed document.
///
/// Preferred over [`filter_links`] when the document is already available,
/// as it avoids a serialization + re-parse round-trip.
pub fn filter_links_inplace(
    document: &mut Html,
    link_text_content_to_remove: &[String],
    link_hrefs_to_remove: &[String],
) {
    let lower_text_patterns = link_text_content_to_remove
        .iter()
        .map(|pattern| pattern.to_lowercase())
        .collect::<Vec<_>>();
    let ids: Vec<_> = document
        .select(&SELECTOR_A_BUTTON)
        .filter(|el| {
            should_remove_link(
                el,
                link_text_content_to_remove,
                &lower_text_patterns,
                link_hrefs_to_remove,
            )
        })
        .map(|el| el.id())
        .collect();
    for id in ids {
        document
            .tree
            .get_mut(id)
            .expect("BUG: collected node id not in tree")
            .detach();
    }
}

fn should_remove_link(
    el: &ElementRef<'_>,
    link_text_content_to_remove: &[String],
    lower_text_patterns: &[String],
    link_hrefs_to_remove: &[String],
) -> bool {
    let text: String = el.text().collect();
    let href = el.value().attr("href");
    let has_image = el.select(&SELECTOR_IMG).next().is_some();
    // Keep image-only links: they have no text but contain visible content.
    (text.trim().is_empty() && !has_image)
        || href.is_none()
        || href.is_some_and(|h| h.starts_with('#'))
        || should_remove_by_text(&text, link_text_content_to_remove, lower_text_patterns)
        || should_remove_by_href(href, link_hrefs_to_remove)
}

/// Remove `<a>` and `<button>` elements that are empty, anchor-only (`#`),
/// or match the caller's text/href blacklists.
///
/// Uses DOM-based removal so serialization differences between the input and
/// scraper's output don't cause silent no-ops.
pub fn filter_links(
    html: &str,
    link_text_content_to_remove: &[String],
    link_hrefs_to_remove: &[String],
) -> String {
    let mut fragment = Html::parse_fragment(html);
    let lower_text_patterns = link_text_content_to_remove
        .iter()
        .map(|pattern| pattern.to_lowercase())
        .collect::<Vec<_>>();
    let ids: Vec<_> = fragment
        .select(&SELECTOR_A_BUTTON)
        .filter(|el| {
            should_remove_link(
                el,
                link_text_content_to_remove,
                &lower_text_patterns,
                link_hrefs_to_remove,
            )
        })
        .map(|el| el.id())
        .collect();
    for id in ids {
        fragment
            .tree
            .get_mut(id)
            .expect("BUG: collected node id not in tree")
            .detach();
    }
    crate::util::serialize_fragment_body(&fragment)
}

fn should_remove_by_text(text: &str, patterns: &[String], lowered_patterns: &[String]) -> bool {
    if patterns.is_empty() {
        return false;
    }

    // Fast path: if both text and patterns are ASCII, we can do a case-insensitive
    // substring search without allocating a new String for `text_lower` or `pattern`.
    if text.is_ascii() {
        let text_bytes = text.as_bytes();
        patterns.iter().any(|p| {
            if p.is_ascii() {
                let p_bytes = p.as_bytes();
                if p_bytes.is_empty() {
                    return true;
                }
                if p_bytes.len() > text_bytes.len() {
                    return false;
                }
                text_bytes
                    .windows(p_bytes.len())
                    .any(|w| w.eq_ignore_ascii_case(p_bytes))
            } else {
                // Fallback for non-ASCII pattern against ASCII text
                false
            }
        })
    } else {
        // Fallback for non-ASCII text
        let text_lower = text.to_lowercase();
        lowered_patterns.iter().any(|p| text_lower.contains(p))
    }
}

fn should_remove_by_href(href: Option<&str>, patterns: &[String]) -> bool {
    !patterns.is_empty() && href.is_some_and(|h| patterns.iter().any(|pat| href_matches(h, pat)))
}

fn href_matches(href: &str, pattern: &str) -> bool {
    if href.starts_with(pattern) {
        return true;
    }

    if !is_scheme_prefix(pattern) {
        return false;
    }

    let normalized_pattern = normalize_href_scheme(pattern);
    !normalized_pattern.is_empty()
        && normalize_href_scheme(href).starts_with(normalized_pattern.as_str())
}

fn is_scheme_prefix(pattern: &str) -> bool {
    let Some(scheme) = pattern.strip_suffix(':') else {
        return false;
    };

    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

fn normalize_href_scheme(value: &str) -> String {
    value
        .chars()
        .take(1024)
        .filter(|c| !(c.is_control() || c.is_whitespace()))
        .map(|c| c.to_ascii_lowercase())
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_density_filter_selects_highest_scoring_element() {
        let doc = Html::parse_document(
            "<html><body><nav><a href=\"/a\">Link</a></nav><article>Long article body with prose.</article></body></html>",
        );
        let selected = apply_text_density_filter(&doc).expect("article should be selected");
        assert!(selected.html().contains("Long article body"));
    }

    #[test]
    fn text_density_filter_keeps_first_element_when_scores_tie() {
        let doc = Html::parse_document(
            "<html><body><main>Same length</main><article>Same length</article></body></html>",
        );
        let selected = apply_text_density_filter(&doc).expect("main should be selected");
        assert_eq!(selected.value().name(), "main");
    }

    #[test]
    fn text_density_filter_scores_links_and_replaces_lower_score() {
        let doc = Html::parse_document(
            "<html><body><main><a href=\"/nav\">Navigation link</a></main><article>Dense article text wins.</article></body></html>",
        );
        let selected = apply_text_density_filter(&doc).expect("article should be selected");
        assert_eq!(selected.value().name(), "article");
    }

    #[test]
    fn text_density_filter_returns_none_for_empty_document() {
        let doc = Html::parse_document("<html><body></body></html>");
        assert!(apply_text_density_filter(&doc).is_none());
    }

    #[test]
    fn filter_links_removes_empty_and_anchor_links() {
        let html =
            "<p><a href=\"/page\">Keep me</a> <a href=\"#\">Skip</a> <a href=\"/x\"></a></p>";
        let result = filter_links(html, &[], &[]);
        assert!(result.contains("Keep me"));
        assert!(!result.contains("href=\"#\""));
    }

    #[test]
    fn filter_links_removes_by_text_pattern() {
        let html =
            "<p><a href=\"/\">Log In</a> <a href=\"/y\">Read More</a> <a href=\"/z\">Share</a></p>";
        let patterns = vec!["log in".to_string(), "share".to_string()];
        let filtered = filter_links(html, &patterns, &[]);
        assert!(!filtered.contains("Log In"));
        assert!(!filtered.contains("Share"));
        assert!(filtered.contains("Read More"));
    }

    #[test]
    fn filter_links_removes_by_text_pattern_lowercase() {
        let html =
            "<p><a href=\"/\">log in</a> <a href=\"/y\">read more</a> <a href=\"/z\">share</a></p>";
        let patterns = vec!["log in".to_string(), "share".to_string()];
        let filtered = filter_links(html, &patterns, &[]);
        assert!(!filtered.contains("log in"));
        assert!(!filtered.contains("share"));
        assert!(filtered.contains("read more"));
    }

    #[test]
    fn filter_links_removes_by_text_pattern_non_ascii() {
        let html = "<p><a href=\"/\">Connexion</a> <a href=\"/y\">Lire la suite</a> <a href=\"/z\">Partager</a></p>";
        let patterns = vec!["connexion".to_string(), "partager".to_string()];
        let filtered = filter_links(html, &patterns, &[]);
        assert!(!filtered.contains("Connexion"));
        assert!(!filtered.contains("Partager"));
        assert!(filtered.contains("Lire la suite"));
    }

    #[test]
    fn filter_links_removes_by_text_pattern_non_ascii_lowercase() {
        let html = "<p><a href=\"/\">connexion</a> <a href=\"/y\">lire la suite</a> <a href=\"/z\">partager</a></p>";
        let patterns = vec!["connexion".to_string(), "partager".to_string()];
        let filtered = filter_links(html, &patterns, &[]);
        assert!(!filtered.contains("connexion"));
        assert!(!filtered.contains("partager"));
        assert!(filtered.contains("lire la suite"));
    }

    #[test]
    fn filter_links_removes_by_text_pattern_non_ascii_fallback() {
        let html = "<p><a href=\"/\">Log In\u{00A0}Now</a> <a href=\"/y\">Read More</a></p>";
        let patterns = vec!["log in\u{00A0}now".to_string()];
        let filtered = filter_links(html, &patterns, &[]);
        assert!(!filtered.contains("Log In\u{00A0}Now"));
        assert!(filtered.contains("Read More"));
    }

    #[test]
    fn filter_links_inplace_removes_empty_and_anchor_links() {
        let html = r##"<html><body><p><a href="/page">Keep me</a> <a href="#">Skip</a> <a href="/x"></a></p></body></html>"##;
        let mut document = Html::parse_document(html);
        filter_links_inplace(&mut document, &[], &[]);
        let result = crate::util::serialize_fragment_body(&document);
        assert!(result.contains("Keep me"));
        assert!(!result.contains("href=\"#\""));
        assert!(!result.contains("Skip"));
        assert!(!result.contains("href=\"/x\""));
    }

    #[test]
    fn filter_links_inplace_removes_by_href_prefix() {
        let html = r#"<html><body><p><a href="javascript:void(0)">Click</a> <a href="/safe">Safe</a></p></body></html>"#;
        let mut document = Html::parse_document(html);
        filter_links_inplace(&mut document, &[], &["javascript:".to_string()]);
        let result = crate::util::serialize_fragment_body(&document);
        assert!(!result.contains("javascript:"));
        assert!(!result.contains("Click"));
        assert!(result.contains("Safe"));
    }

    #[test]
    fn filter_links_inplace_removes_button_without_href() {
        let html = r#"<html><body><p><button>Click</button></p></body></html>"#;
        let mut document = Html::parse_document(html);
        filter_links_inplace(&mut document, &[], &[]);
        let result = crate::util::serialize_fragment_body(&document);
        assert!(!result.contains("<button>"));
        assert!(!result.contains("Click"));
    }

    #[test]
    fn filter_links_inplace_preserves_image_only_link() {
        let html = r#"<html><body><p><a href="/logo"><img src="logo.png" alt="Logo"></a></p></body></html>"#;
        let mut document = Html::parse_document(html);
        filter_links_inplace(&mut document, &[], &[]);
        let result = crate::util::serialize_fragment_body(&document);
        assert!(result.contains("href=\"/logo\""));
        assert!(
            result.contains("logo.png"),
            "image-only link should be preserved"
        );
    }

    #[test]
    fn filter_links_inplace_removes_by_text_pattern_case_insensitive() {
        let html = r#"<html><body><main><a href="/close">Close</a> <a href="/keep">Keep</a></main></body></html>"#;
        let mut document = Html::parse_document(html);

        filter_links_inplace(&mut document, &["close".to_string()], &[]);

        let result = crate::util::serialize_fragment_body(&document);
        assert!(!result.contains("Close"));
        assert!(result.contains("Keep"));
    }

    #[test]
    fn filter_links_removes_by_href_prefix() {
        let html = r#"<p><a href="javascript:void(0)">Click</a> <a href="/safe">Safe</a></p>"#;
        let result = filter_links(html, &[], &["javascript:".to_string()]);
        assert!(!result.contains("javascript:"));
        assert!(result.contains("Safe"));
    }

    #[test]
    fn filter_links_removes_button_without_href() {
        let html = "<p><button>Click</button></p>";
        let result = filter_links(html, &[], &[]);
        assert!(!result.contains("<button>"));
    }

    #[test]
    fn filter_links_removes_href_with_mixed_case_and_whitespace() {
        let html = r#"<p><a href="  JaVaScRiPt:alert(1)">Click</a></p>"#;
        let result = filter_links(html, &[], &["javascript:".to_string()]);
        assert!(!result.contains("JaVaScRiPt:"));
        assert!(!result.contains("Click"));
    }

    #[test]
    fn filter_links_removes_href_with_ascii_control_characters() {
        let html = "<p><a href=\"\x01java\nscript:alert(1)\">Click</a></p>";
        let result = filter_links(html, &[], &["javascript:".to_string()]);
        assert!(!result.contains("java"));
        assert!(!result.contains("Click"));
    }

    #[test]
    fn filter_links_normalizes_mixed_case_scheme_pattern() {
        let html = r#"<p><a href="javascript:void(0)">Click</a></p>"#;
        let result = filter_links(html, &[], &["JavaScript:".to_string()]);
        assert!(!result.contains("javascript:"));
        assert!(!result.contains("Click"));
    }

    #[test]
    fn filter_links_keeps_non_scheme_href_prefix_case_sensitive() {
        let html = r#"<p><a href="/Admin">Keep</a> <a href="/admin">Remove</a></p>"#;
        let result = filter_links(html, &[], &["/admin".to_string()]);
        assert!(result.contains("Keep"));
        assert!(!result.contains("Remove"));
    }

    #[test]
    fn filter_links_preserves_image_only_link() {
        let html = r#"<p><a href="/logo"><img src="logo.png" alt="Logo"></a></p>"#;
        let result = filter_links(html, &[], &[]);
        assert!(
            result.contains("logo.png"),
            "image-only link should be preserved"
        );
    }

    #[test]
    fn filter_links_removes_by_text_pattern_ascii_mixed_case() {
        let html = r#"<p><a href="/close">ClOsE</a> <a href="/keep">Keep</a></p>"#;
        let result = filter_links(html, &["close".to_string()], &[]);
        assert!(!result.contains("ClOsE"));
        assert!(result.contains("Keep"));
    }

    #[test]
    fn filter_links_removes_by_text_pattern_ascii_lower_case() {
        let html = r#"<p><a href="/close">close</a> <a href="/keep">keep</a></p>"#;
        let result = filter_links(html, &["close".to_string()], &[]);
        assert!(!result.contains(">close<"));
        assert!(result.contains(">keep<"));
    }

    #[test]
    fn filter_links_removes_by_text_pattern_unicode() {
        let html = r#"<p><a href="/close">Cłose</a> <a href="/keep">Keep</a></p>"#;
        let result = filter_links(html, &["cłose".to_string()], &[]);
        assert!(!result.contains("Cłose"));
        assert!(result.contains("Keep"));
    }

    #[test]
    fn filter_links_removes_by_text_pattern_unicode_lowercase() {
        let html = r#"<p><a href="/close">cłose</a> <a href="/keep">keep</a></p>"#;
        let result = filter_links(html, &["cłose".to_string()], &[]);
        assert!(!result.contains(">cłose<"));
        assert!(result.contains(">keep<"));
    }

    #[test]
    fn filter_links_keeps_when_empty_pattern() {
        let html = r#"<p><a href="/close">close</a></p>"#;
        let result = filter_links(html, &[], &[]);
        assert!(result.contains(">close<"));
    }

    #[test]
    fn test_normalize_href_scheme_bypasses() {
        assert_eq!(normalize_href_scheme("java\u{0085}script:"), "javascript:");
        assert_eq!(normalize_href_scheme("java\u{2002}script:"), "javascript:");
        assert_eq!(normalize_href_scheme("java\u{00A0}script:"), "javascript:");
        assert_eq!(normalize_href_scheme("javascript:"), "javascript:");
        assert_eq!(normalize_href_scheme("  \t java\nscript: "), "javascript:");
    }

    #[test]
    fn filter_links_handles_empty_pattern_in_text() {
        let html = r#"<p><a href="/close">close</a></p>"#;
        // An empty pattern should effectively be ignored or removed depending on logic.
        // Wait, should_remove_by_text returns true if pattern is empty AFTER the initial guard?
        // The initial guard is:
        // if patterns.is_empty() { return false; }
        // But if `patterns` contains `""` as an element:
        // patterns.iter().any(|p| ...)
        // A pattern of `""` will cause `is_empty()` to be true and return `true`, meaning it removes it.
        let result = filter_links(html, &["".to_string()], &[]);
        // original logic: text.to_lowercase().contains("") returns true.
        // so it removes everything.
        assert!(!result.contains(">close<"));
        assert!(!result.contains("<a"));
    }

    #[test]
    fn filter_links_handles_pattern_longer_than_text() {
        let html = r#"<p><a href="/close">a</a></p>"#;
        let result = filter_links(html, &["ab".to_string()], &[]);
        assert!(result.contains(">a<"));
    }

    #[test]
    fn calculate_text_density_score_handles_non_a_elements_on_close() {
        let html = "<html><body><main>Test <b>bold</b> <a href=\"#\">link</a> <span>more</span></main></body></html>";
        let doc = Html::parse_document(html);
        let selected = apply_text_density_filter(&doc).expect("main should be selected");
        assert_eq!(selected.value().name(), "main");
    }

    #[test]
    fn filter_links_inplace_keeps_when_empty_pattern() {
        let html = r#"<html><body><p><a href="/close">close</a></p></body></html>"#;
        let mut document = Html::parse_document(html);
        filter_links_inplace(&mut document, &[], &[]);
        let result = crate::util::serialize_fragment_body(&document);
        assert!(result.contains(">close<"));
    }

    #[test]
    fn compute_density_score_handles_normal_values() {
        let score = compute_density_score(100, 20, 1); // 100 - 20 - 20 = 60
        assert_eq!(score, 60);
    }

    #[test]
    fn compute_density_score_handles_extremely_large_text() {
        // text_len exceeds i32::MAX. Clamp the full formula result.
        let score = compute_density_score(usize::MAX, 0, 0);
        assert_eq!(score, i32::MAX);
    }

    #[test]
    fn compute_density_score_clamps_after_full_formula() {
        // A tiny link adjustment should not change ordering relative to overflowed text lengths.
        let score = compute_density_score(i32::MAX as usize + 1_000, 500, 0);
        assert_eq!(score, i32::MAX);
    }

    #[test]
    fn compute_density_score_handles_extremely_large_links() {
        // link_count exceeds i32::MAX.
        let score = compute_density_score(1000, 0, usize::MAX);
        assert_eq!(score, i32::MIN);
    }

    #[test]
    fn compute_density_score_handles_extremely_large_link_text() {
        // link_text_len exceeds i32::MAX.
        let score = compute_density_score(1000, usize::MAX, 0);
        assert_eq!(score, i32::MIN);
    }

    #[test]
    fn compute_density_score_handles_everything_extremely_large() {
        let score = compute_density_score(usize::MAX, usize::MAX, usize::MAX);
        assert_eq!(score, i32::MIN);
    }
}
