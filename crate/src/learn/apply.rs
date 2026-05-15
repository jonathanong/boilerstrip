use regex::Regex;
use scraper::{Html, Selector};
use std::sync::LazyLock;

use super::types::{ApplyError, Removals};
use crate::util::serialize_fragment_body;

static WHITESPACE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+").expect("BUG: invalid whitespace regex"));
static TAG_OPEN_CONTENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r">([^<])").expect("BUG: invalid tag-open-content regex"));
static CONTENT_TAG_CLOSE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([^>])<").expect("BUG: invalid content-tag-close regex"));

/// Apply previously learned boilerplate removals to an HTML string.
///
/// Removes elements matching CSS selectors first, then strips verbatim HTML
/// snippets using flexible whitespace-tolerant regex matching.
pub fn apply_removals(html: &str, removals: &Removals) -> Result<String, ApplyError> {
    let mut cleaned = html.to_string();

    if !removals.css_selectors_to_remove.is_empty() {
        cleaned = apply_css_selector_removals(&cleaned, &removals.css_selectors_to_remove);
    }
    if !removals.html_to_remove.is_empty() {
        cleaned = apply_html_snippet_removals(&cleaned, &removals.html_to_remove);
    }

    Ok(cleaned.trim().to_string())
}

fn apply_css_selector_removals(html: &str, selectors: &[String]) -> String {
    let mut fragment = Html::parse_fragment(html);

    for selector_str in selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            let ids_to_remove: Vec<_> = fragment.select(&selector).map(|el| el.id()).collect();
            for id in ids_to_remove {
                let mut node = fragment
                    .tree
                    .get_mut(id)
                    .expect("BUG: selected element id should exist in fragment tree");
                node.detach();
            }
        }
    }

    serialize_fragment_body(&fragment)
}

fn apply_html_snippet_removals(html: &str, snippets: &[String]) -> String {
    let mut cleaned = html.to_string();

    let mut sorted_snippets = snippets.to_vec();
    sorted_snippets.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));

    let normalized_snippets: Vec<(String, &String)> = sorted_snippets
        .iter()
        .filter(|s| !s.trim().is_empty())
        .map(|s| (normalize_whitespace(s), s))
        .collect();

    for (normalized_snippet, original_snippet) in &normalized_snippets {
        let normalized_cleaned = normalize_whitespace(&cleaned);
        if normalized_cleaned.contains(normalized_snippet.as_str()) {
            cleaned = remove_html_snippet(&cleaned, original_snippet);
        }
    }

    cleaned
}

fn normalize_whitespace(text: &str) -> String {
    WHITESPACE_PATTERN
        .replace_all(text, " ")
        .trim()
        .to_lowercase()
}

fn remove_html_snippet(html: &str, snippet: &str) -> String {
    let normalized_snippet = snippet.trim();
    let escaped = regex::escape(normalized_snippet);
    let flexible = WHITESPACE_PATTERN.replace_all(&escaped, r"\s+");
    let with_tag_open = TAG_OPEN_CONTENT_RE
        .replace_all(&flexible, r">\s*$1")
        .to_string();
    let pattern = CONTENT_TAG_CLOSE_RE
        .replace_all(&with_tag_open, r"$1\s*<")
        .to_string();
    apply_snippet_regex(html, &pattern)
}

fn apply_snippet_regex(html: &str, pattern: &str) -> String {
    Regex::new(&format!("(?i){pattern}")).map_or_else(
        |_| html.to_string(),
        |re| re.replace_all(html, "").to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_dynamic_snippet_regex_leaves_html_unchanged() {
        assert_eq!(apply_snippet_regex("<p>Keep</p>", "("), "<p>Keep</p>");
    }

    #[test]
    fn apply_removals_with_empty_removals_returns_trimmed_html() {
        let removals = Removals::default();
        let result = apply_removals("  <p>hello</p>  ", &removals).unwrap();
        assert_eq!(result, "<p>hello</p>");
    }

    #[test]
    fn apply_removals_strips_matching_css_selector() {
        let removals = Removals {
            css_selectors_to_remove: vec!["nav".to_string()],
            html_to_remove: vec![],
        };
        let html = "<nav>Menu</nav><main>Content</main>";
        let result = apply_removals(html, &removals).unwrap();
        assert!(!result.contains("<nav>"));
        assert!(result.contains("Content"));
    }

    #[test]
    fn apply_removals_strips_html_snippet_flexibly() {
        let removals = Removals {
            css_selectors_to_remove: vec![],
            html_to_remove: vec!["<p>Footer text here</p>".to_string()],
        };
        let html = "<div><p>Content</p></div><p>Footer  text  here</p>";
        let result = apply_removals(html, &removals).unwrap();
        assert!(!result.contains("Footer"));
        assert!(result.contains("Content"));
    }

    #[test]
    fn apply_removals_skips_blank_snippets() {
        let removals = Removals {
            css_selectors_to_remove: vec![],
            html_to_remove: vec!["   ".to_string()],
        };
        let html = "<p>Keep</p>";
        let result = apply_removals(html, &removals).unwrap();
        assert_eq!(result, "<p>Keep</p>");
    }

    #[test]
    fn apply_removals_skips_invalid_css_selector() {
        let removals = Removals {
            // "[" is an unclosed attribute selector — definitely invalid
            css_selectors_to_remove: vec!["[".to_string()],
            html_to_remove: vec![],
        };
        let result = apply_removals("<p>Keep</p>", &removals).unwrap();
        assert_eq!(result, "<p>Keep</p>");
    }

    #[test]
    fn apply_removals_removes_multiple_css_selectors() {
        // Exercises the detach loop with 2 matching elements
        let removals = Removals {
            css_selectors_to_remove: vec!["nav".to_string()],
            html_to_remove: vec![],
        };
        let html = "<nav>Nav One</nav><main>Content</main><nav>Nav Two</nav>";
        let result = apply_removals(html, &removals).unwrap();
        assert!(!result.contains("Nav One"));
        assert!(!result.contains("Nav Two"));
        assert!(result.contains("Content"));
    }

    #[test]
    fn apply_removals_with_same_length_snippets_uses_lexical_sort() {
        // Two snippets of equal length: exercises the then_with closure in sort_by
        let removals = Removals {
            css_selectors_to_remove: vec![],
            html_to_remove: vec![
                "<p>aaaa</p>".to_string(), // same length as below
                "<p>bbbb</p>".to_string(),
            ],
        };
        let html = "<p>aaaa</p><p>bbbb</p><p>Keep</p>";
        let result = apply_removals(html, &removals).unwrap();
        assert!(!result.contains("aaaa"));
        assert!(!result.contains("bbbb"));
        assert!(result.contains("Keep"));
    }

    #[test]
    fn apply_html_snippet_removals_skips_snippet_not_in_html() {
        // Exercises the false branch of normalized_cleaned.contains(...)
        let removals = Removals {
            css_selectors_to_remove: vec![],
            html_to_remove: vec!["<p>This snippet is not in the html</p>".to_string()],
        };
        let html = "<p>Different content entirely</p>";
        let result = apply_removals(html, &removals).unwrap();
        assert_eq!(result, html);
    }
}
