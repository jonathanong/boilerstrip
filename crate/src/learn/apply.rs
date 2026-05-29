use regex::Regex;
use scraper::Selector;
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

/// Pre-compiled form of [`Removals`] for efficient batch application.
///
/// Build once from a `Removals`, then call [`apply_removals_compiled`] for each
/// page without recompiling selectors or regexes.
pub struct CompiledRemovals {
    pub(crate) selectors: Vec<Selector>,
    pub(crate) snippet_regexes: Vec<(String, regex::Regex)>,
}

impl CompiledRemovals {
    pub fn new(removals: &Removals) -> Self {
        let selectors = removals
            .css_selectors_to_remove
            .iter()
            .filter_map(|s| Selector::parse(s).ok())
            .collect();
        let mut sorted_snippets: Vec<&String> = removals.html_to_remove.iter().collect();
        sorted_snippets.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        let snippet_regexes = sorted_snippets
            .into_iter()
            .filter(|s| !s.trim().is_empty())
            .filter_map(|s| build_snippet_regex(s).map(|re| (normalize_whitespace(s), re)))
            .collect();
        Self {
            selectors,
            snippet_regexes,
        }
    }
}

/// Apply pre-compiled removals to an HTML string.
pub fn apply_removals_compiled(html: &str, compiled: &CompiledRemovals) -> String {
    let mut cleaned = if compiled.selectors.is_empty() {
        html.to_string()
    } else {
        crate::util::remove_matching(html, |el| {
            compiled.selectors.iter().any(|sel| sel.matches(el))
        })
    };

    if !compiled.snippet_regexes.is_empty() {
        let mut normalized_cleaned = normalize_whitespace(&cleaned);
        for (normalized_snippet, re) in &compiled.snippet_regexes {
            if normalized_cleaned.contains(normalized_snippet.as_str()) {
                let next = re.replace_all(&cleaned, "").to_string();
                normalized_cleaned = normalize_whitespace(&next);
                cleaned = next;
            }
        }
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

    let mut sorted_snippets: Vec<&String> = snippets.iter().collect();
    sorted_snippets.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));

    // Precompile all snippet regexes upfront — avoids Regex::new per iteration.
    let compiled: Vec<(String, Regex)> = sorted_snippets
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .filter_map(|s| build_snippet_regex(s).map(|re| (normalize_whitespace(s), re)))
        .collect();

    let mut normalized_cleaned = normalize_whitespace(&cleaned);
    for (normalized_snippet, re) in &compiled {
        if normalized_cleaned.contains(normalized_snippet.as_str()) {
            let next = re.replace_all(&cleaned, "").to_string();
            normalized_cleaned = normalize_whitespace(&next);
            cleaned = next;
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

    #[test]
    fn compiled_removals_applies_css_and_snippets() {
        let removals = Removals {
            css_selectors_to_remove: vec!["nav".to_string()],
            html_to_remove: vec!["<p>Footer</p>".to_string()],
        };
        let compiled = CompiledRemovals::new(&removals);
        let html = "<nav>Menu</nav><p>Footer</p><main>Content</main>";
        let result = apply_removals_compiled(html, &compiled);
        assert!(!result.contains("Menu"));
        assert!(!result.contains("Footer"));
        assert!(result.contains("Content"));
    }

    #[test]
    fn compiled_removals_with_empty_removals_returns_trimmed() {
        let removals = Removals::default();
        let compiled = CompiledRemovals::new(&removals);
        let result = apply_removals_compiled("  <p>hello</p>  ", &compiled);
        assert_eq!(result, "<p>hello</p>");
    }

    #[test]
    fn compiled_removals_skips_invalid_css_selector() {
        let removals = Removals {
            css_selectors_to_remove: vec!["[".to_string()],
            html_to_remove: vec![],
        };
        let compiled = CompiledRemovals::new(&removals);
        let result = apply_removals_compiled("<p>Keep</p>", &compiled);
        assert_eq!(result, "<p>Keep</p>");
    }

    #[test]
    fn compiled_removals_sorts_equal_length_snippets_lexically() {
        // Two same-length snippets trigger the then_with lexical tiebreak in sort_by
        let removals = Removals {
            css_selectors_to_remove: vec![],
            html_to_remove: vec!["<p>aaa</p>".to_string(), "<p>bbb</p>".to_string()],
        };
        let compiled = CompiledRemovals::new(&removals);
        let html = "<p>aaa</p><p>bbb</p><p>Keep</p>";
        let result = apply_removals_compiled(html, &compiled);
        assert!(!result.contains("aaa"));
        assert!(!result.contains("bbb"));
        assert!(result.contains("Keep"));
    }

    #[test]
    fn compiled_removals_skips_snippet_not_in_html() {
        // Snippet in compiled removals that doesn't appear in the HTML — if-body skipped
        let removals = Removals {
            css_selectors_to_remove: vec![],
            html_to_remove: vec!["<p>Missing snippet</p>".to_string()],
        };
        let compiled = CompiledRemovals::new(&removals);
        let html = "<p>Different content</p>";
        let result = apply_removals_compiled(html, &compiled);
        assert_eq!(result, html);
    }
}
