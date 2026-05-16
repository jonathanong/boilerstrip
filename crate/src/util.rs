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
        fragment
            .tree
            .get_mut(id)
            .expect("BUG: collected node id not in tree")
            .detach();
    }
    serialize_fragment_body(&fragment)
}

/// Serialize a parsed HTML fragment's body without the synthetic `<html>` wrapper.
/// `Html::parse_fragment` wraps content in an `<html>` element — this walks its
/// children directly instead of stripping literal prefixes/suffixes.
pub(crate) fn serialize_fragment_body(fragment: &scraper::Html) -> String {
    use scraper::Selector;
    use std::sync::LazyLock;
    static SELECTOR_HTML: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse("html").expect("BUG: invalid SELECTOR_HTML"));
    fragment
        .select(&SELECTOR_HTML)
        .next()
        .map(|html_el| html_el.inner_html())
        .expect("BUG: Html::parse_fragment always produces an <html> root")
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
