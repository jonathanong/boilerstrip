use regex::Regex;
use std::sync::LazyLock;

use super::types::Removals;

static WHITESPACE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+").expect("BUG: invalid whitespace regex"));
static TAG_OPEN_CONTENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r">([^<])").expect("BUG: invalid tag-open-content regex"));
static CONTENT_TAG_CLOSE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([^>])<").expect("BUG: invalid content-tag-close regex"));

/// Apply previously learned boilerplate removals to an HTML string.
///
/// Removes elements matching CSS selectors first (DOM-based), then strips
/// verbatim HTML snippets using flexible whitespace-tolerant regex matching.
pub fn apply_removals(html: &str, removals: &Removals) -> String {
    let mut cleaned = html.to_string();

    if !removals.css_selectors_to_remove.is_empty() {
        cleaned = apply_css_selector_removals(&cleaned, &removals.css_selectors_to_remove);
    }
    if !removals.html_to_remove.is_empty() {
        cleaned = apply_html_snippet_removals(&cleaned, &removals.html_to_remove);
    }

    cleaned.trim().to_string()
}

fn apply_css_selector_removals(html: &str, selectors: &[String]) -> String {
    let parsed: Vec<scraper::Selector> = selectors
        .iter()
        .filter_map(|s| scraper::Selector::parse(s).ok())
        .collect();
    crate::util::remove_matching(html, |el| parsed.iter().any(|sel| sel.matches(el)))
}

pub(crate) fn apply_html_snippet_removals(html: &str, snippets: &[String]) -> String {
    let mut cleaned = html.to_string();

    let mut sorted_snippets = snippets.to_vec();
    sorted_snippets.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));

    // Precompile all snippet regexes upfront — avoids Regex::new per iteration.
    let compiled: Vec<(String, Regex)> = sorted_snippets
        .iter()
        .filter(|s| !s.trim().is_empty())
        .filter_map(|s| build_snippet_regex(s).map(|re| (normalize_whitespace(s), re)))
        .collect();

    let mut normalized_cleaned = normalize_whitespace(&cleaned);
    for (normalized_snippet, re) in &compiled {
        if normalized_cleaned.contains(normalized_snippet.as_str()) {
            let next = re.replace_all(&cleaned, "").to_string();
            if next != cleaned {
                normalized_cleaned = normalize_whitespace(&next);
                cleaned = next;
            }
        }
    }

    cleaned
}

fn build_snippet_regex(snippet: &str) -> Option<Regex> {
    let normalized_snippet = snippet.trim();
    let escaped = regex::escape(normalized_snippet);
    let flexible = WHITESPACE_PATTERN.replace_all(&escaped, r"\s+");
    let with_tag_open = TAG_OPEN_CONTENT_RE
        .replace_all(&flexible, r">\s*$1")
        .to_string();
    let pattern = CONTENT_TAG_CLOSE_RE
        .replace_all(&with_tag_open, r"$1\s*<")
        .to_string();
    Regex::new(&format!("(?i){pattern}")).ok()
}

fn normalize_whitespace(text: &str) -> String {
    WHITESPACE_PATTERN
        .replace_all(text, " ")
        .trim()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_snippet_regex_is_skipped() {
        // An HTML snippet that would produce an invalid regex after escaping/flexible
        // replacement is silently skipped (build_snippet_regex returns None).
        let result = apply_html_snippet_removals("<p>Keep</p>", &["(".to_string()]);
        assert_eq!(result, "<p>Keep</p>");
    }

    #[test]
    fn apply_removals_with_empty_removals_returns_trimmed_html() {
        let removals = Removals::default();
        let result = apply_removals("  <p>hello</p>  ", &removals);
        assert_eq!(result, "<p>hello</p>");
    }

    #[test]
    fn apply_removals_strips_matching_css_selector() {
        let removals = Removals {
            css_selectors_to_remove: vec!["nav".to_string()],
            html_to_remove: vec![],
        };
        let html = "<nav>Menu</nav><main>Content</main>";
        let result = apply_removals(html, &removals);
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
        let result = apply_removals(html, &removals);
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
        let result = apply_removals(html, &removals);
        assert_eq!(result, "<p>Keep</p>");
    }

    #[test]
    fn apply_removals_skips_invalid_css_selector() {
        let removals = Removals {
            css_selectors_to_remove: vec!["[".to_string()],
            html_to_remove: vec![],
        };
        let result = apply_removals("<p>Keep</p>", &removals);
        assert_eq!(result, "<p>Keep</p>");
    }

    #[test]
    fn apply_removals_removes_multiple_css_selectors() {
        let removals = Removals {
            css_selectors_to_remove: vec!["nav".to_string()],
            html_to_remove: vec![],
        };
        let html = "<nav>Nav One</nav><main>Content</main><nav>Nav Two</nav>";
        let result = apply_removals(html, &removals);
        assert!(!result.contains("Nav One"));
        assert!(!result.contains("Nav Two"));
        assert!(result.contains("Content"));
    }

    #[test]
    fn apply_removals_with_same_length_snippets_uses_lexical_sort() {
        let removals = Removals {
            css_selectors_to_remove: vec![],
            html_to_remove: vec!["<p>aaaa</p>".to_string(), "<p>bbbb</p>".to_string()],
        };
        let html = "<p>aaaa</p><p>bbbb</p><p>Keep</p>";
        let result = apply_removals(html, &removals);
        assert!(!result.contains("aaaa"));
        assert!(!result.contains("bbbb"));
        assert!(result.contains("Keep"));
    }

    #[test]
    fn apply_html_snippet_removals_skips_snippet_not_in_html() {
        let removals = Removals {
            css_selectors_to_remove: vec![],
            html_to_remove: vec!["<p>This snippet is not in the html</p>".to_string()],
        };
        let html = "<p>Different content entirely</p>";
        let result = apply_removals(html, &removals);
        assert_eq!(result, html);
    }
}
