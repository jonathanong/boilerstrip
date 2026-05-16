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
    let text_length = text.len() as i32;

    let mut link_text_length = 0i32;
    let mut link_count = 0i32;
    for link in element.select(&SELECTOR_A) {
        let link_text: String = link.text().collect();
        link_text_length += link_text.len() as i32;
        link_count += 1;
    }

    text_length - link_text_length - (link_count * TEXT_DENSITY_LINK_PENALTY)
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
    let ids: Vec<_> = document
        .select(&SELECTOR_A_BUTTON)
        .filter(|el| should_remove_link(el, link_text_content_to_remove, link_hrefs_to_remove))
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
    link_hrefs_to_remove: &[String],
) -> bool {
    let text: String = el.text().collect();
    let href = el.value().attr("href");
    let has_image = el.select(&SELECTOR_IMG).next().is_some();
    // Keep image-only links: they have no text but contain visible content.
    (text.trim().is_empty() && !has_image)
        || href.is_none()
        || href.is_some_and(|h| h.starts_with('#'))
        || should_remove_by_text(&text, link_text_content_to_remove)
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
    let ids: Vec<_> = fragment
        .select(&SELECTOR_A_BUTTON)
        .filter(|el| should_remove_link(el, link_text_content_to_remove, link_hrefs_to_remove))
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

fn should_remove_by_text(text: &str, patterns: &[String]) -> bool {
    !patterns.is_empty() && {
        let text_lower = text.to_lowercase();
        patterns
            .iter()
            .any(|p| text_lower.contains(&p.to_lowercase()))
    }
}

fn should_remove_by_href(href: Option<&str>, patterns: &[String]) -> bool {
    !patterns.is_empty()
        && href.is_some_and(|h| patterns.iter().any(|pat| h.starts_with(pat.as_str())))
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
        let html = r#"<p><a href="/close">Close</a> <a href="/keep">Keep</a></p>"#;
        let result = filter_links(html, &["close".to_string()], &[]);
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
    fn filter_links_preserves_image_only_link() {
        let html = r#"<p><a href="/logo"><img src="logo.png" alt="Logo"></a></p>"#;
        let result = filter_links(html, &[], &[]);
        assert!(
            result.contains("logo.png"),
            "image-only link should be preserved"
        );
    }
}
