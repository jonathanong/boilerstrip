use scraper::ElementRef;
use std::sync::LazyLock;

static SELECTOR_ALL: LazyLock<scraper::Selector> =
    LazyLock::new(|| scraper::Selector::parse("*").expect("BUG: invalid SELECTOR_ALL in util"));

/// Remove all elements from `html` for which `should_remove` returns `true`.
///
/// Operates entirely in the DOM (parse → detach → serialize) so the removal is
/// safe regardless of how scraper re-serializes attribute order or entities.
pub(crate) fn remove_matching(
    html: &str,
    mut should_remove: impl FnMut(&ElementRef) -> bool,
) -> String {
    let mut fragment = scraper::Html::parse_fragment(html);
    let ids: Vec<_> = fragment
        .select(&SELECTOR_ALL)
        .filter(|el| should_remove(el))
        .map(|el| el.id())
        .collect();
    for id in ids {
        if let Some(mut node) = fragment.tree.get_mut(id) {
            node.detach();
        }
    }
    serialize_fragment_body(&fragment)
}

/// Serialize a parsed HTML fragment's body without the synthetic `<html>` wrapper.
/// `Html::parse_fragment` wraps content in `<html>...</html>` — this strips those wrappers.
pub(crate) fn serialize_fragment_body(fragment: &scraper::Html) -> String {
    let full_html = fragment.html();
    let stripped = full_html.strip_prefix("<html>").unwrap_or(&full_html);
    stripped
        .strip_suffix("</html>")
        .unwrap_or(stripped)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_html_wrappers_from_fragment() {
        let fragment = scraper::Html::parse_fragment("<p>hello</p>");
        let result = serialize_fragment_body(&fragment);
        assert!(result.contains("<p>hello</p>"));
        assert!(!result.starts_with("<html>"));
    }

    #[test]
    fn handles_document_without_html_prefix() {
        let fragment = scraper::Html::parse_fragment("");
        let result = serialize_fragment_body(&fragment);
        assert!(!result.starts_with("<html>"));
    }
}
