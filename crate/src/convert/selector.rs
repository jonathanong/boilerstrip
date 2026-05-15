use scraper::{ElementRef, Html, Selector};
use std::sync::LazyLock;

static SELECTOR_MAIN: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("main").expect("BUG: invalid 'main' selector"));
static SELECTOR_ARTICLE: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("article").expect("BUG: invalid 'article' selector"));
static SELECTOR_BODY: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("body").expect("BUG: invalid 'body' selector"));

/// Strip elements whose tag name matches any entry in `tags` from raw HTML.
///
/// Uses DOM-based removal so serialization differences between the input and
/// scraper's output don't cause silent no-ops.
pub fn remove_elements(html: &str, tags: &[&str]) -> String {
    crate::util::remove_matching(html, |el| tags.iter().any(|t| el.value().name() == *t))
}

/// Remove all elements matching the given CSS selectors.
///
/// Uses DOM-based removal so serialization differences between the input and
/// scraper's output don't cause silent no-ops.
pub fn remove_by_css_selectors(html: &str, selectors: Option<&[String]>) -> String {
    let Some(selectors) = selectors.filter(|s| !s.is_empty()) else {
        return html.to_string();
    };
    let parsed: Vec<Selector> = selectors
        .iter()
        .filter_map(|s| Selector::parse(s).ok())
        .collect();
    if parsed.is_empty() {
        return html.to_string();
    }
    crate::util::remove_matching(html, |el| parsed.iter().any(|sel| sel.matches(el)))
}

/// Narrow to a single content root element.
///
/// Resolution order:
/// 1. User-provided selectors — first match wins.
/// 2. Exactly one `<main>`, `<article>`, or `<body>` (in order).
/// 3. First child element of the document root.
pub fn select_content_root<'a>(
    document: &'a Html,
    content_selectors: Option<&[String]>,
) -> Option<ElementRef<'a>> {
    if let Some(selectors) = content_selectors {
        if let Some(element) = selectors
            .iter()
            .filter_map(|s| Selector::parse(s).ok())
            .find_map(|sel| document.select(&sel).next())
        {
            return Some(element);
        }
    }

    for selector in &[&SELECTOR_MAIN, &SELECTOR_ARTICLE, &SELECTOR_BODY] {
        let elements: Vec<_> = document.select(selector).collect();
        if elements.len() == 1 {
            return Some(elements[0]);
        }
    }

    document
        .root_element()
        .children()
        .find_map(ElementRef::wrap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_elements_strips_script_tags() {
        let html = r#"<p>Keep</p><script>var x=1;</script><p>Also keep</p>"#;
        let result = remove_elements(html, &["script"]);
        assert!(!result.contains("<script>"));
        assert!(result.contains("Keep"));
    }

    #[test]
    fn remove_elements_strips_style_tags() {
        let html = r#"<p>Keep</p><style>.a{color:red}</style>"#;
        let result = remove_elements(html, &["style"]);
        assert!(!result.contains("<style>"));
        assert!(result.contains("Keep"));
    }

    #[test]
    fn remove_elements_ignores_unknown_tag() {
        let html = "<p>Keep</p>";
        let result = remove_elements(html, &["unknown"]);
        assert_eq!(result, html);
    }

    #[test]
    fn remove_by_css_selectors_returns_unchanged_when_none() {
        let html = "<p>Keep</p>";
        assert_eq!(remove_by_css_selectors(html, None), html);
    }

    #[test]
    fn remove_by_css_selectors_returns_unchanged_when_empty() {
        let html = "<p>Keep</p>";
        assert_eq!(remove_by_css_selectors(html, Some(&[])), html);
    }

    #[test]
    fn remove_by_css_selectors_removes_matching_elements() {
        let html = "<nav>Menu</nav><main>Content</main>";
        let result = remove_by_css_selectors(html, Some(&["nav".to_string()]));
        assert!(!result.contains("<nav>"));
        assert!(result.contains("Content"));
    }

    #[test]
    fn remove_by_css_selectors_skips_invalid_selector() {
        let html = "<p>Keep</p>";
        let result = remove_by_css_selectors(html, Some(&["[".to_string()]));
        assert_eq!(result, html);
    }

    #[test]
    fn select_content_root_falls_back_to_first_fragment_child() {
        let document = Html::parse_fragment("<section>Only child</section>");
        let element = select_content_root(&document, Some(&["[".to_string()])).unwrap();
        assert_eq!(element.value().name(), "section");
    }

    #[test]
    fn select_content_root_uses_matching_custom_selector() {
        let document = Html::parse_document(
            "<html><body><main><article>Chosen</article></main></body></html>",
        );
        let element = select_content_root(&document, Some(&["article".to_string()])).unwrap();
        assert_eq!(element.value().name(), "article");
    }

    #[test]
    fn select_content_root_uses_single_semantic_element() {
        let document = Html::parse_document("<html><body><main>Chosen</main></body></html>");
        let element = select_content_root(&document, None).unwrap();
        assert_eq!(element.value().name(), "main");
    }

    #[test]
    fn remove_by_css_selectors_removes_multiple_matching_elements() {
        let html = "<div class=\"ad\">Ad A</div><p>Keep</p><div class=\"ad\">Ad B</div>";
        let result = remove_by_css_selectors(html, Some(&[".ad".to_string()]));
        assert!(!result.contains("Ad A"));
        assert!(!result.contains("Ad B"));
        assert!(result.contains("Keep"));
    }

    #[test]
    fn select_content_root_skips_ambiguous_semantic_elements() {
        let document = Html::parse_document(
            "<html><body><article>One</article><article>Two</article></body></html>",
        );
        let element = select_content_root(&document, None).unwrap();
        assert_eq!(element.value().name(), "body");
    }

    #[test]
    fn remove_elements_handles_script_with_close_tag_in_string() {
        // Script bodies that contain "</script" inside a string literal — DOM removal handles this safely.
        let html = r#"<p>Keep</p><script>var s = "</script";</script><p>Also</p>"#;
        let result = remove_elements(html, &["script"]);
        assert!(!result.contains("<script>"));
        assert!(result.contains("Keep"));
        assert!(result.contains("Also"));
    }

    #[test]
    fn remove_by_css_selectors_removes_duplicate_identical_elements() {
        let html =
            "<nav class=\"site-nav\">Menu</nav><p>Keep</p><nav class=\"site-nav\">Menu</nav>";
        let result = remove_by_css_selectors(html, Some(&[".site-nav".to_string()]));
        assert!(!result.contains("Menu"));
        assert!(result.contains("Keep"));
    }
}
