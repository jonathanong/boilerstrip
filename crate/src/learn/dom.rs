use scraper::{ElementRef, Html};

use super::constants::SELECTOR_BODY;
use super::snippets::should_skip_element;

pub(super) fn resolve_element_by_path<'a>(
    document: &'a Html,
    path: &[usize],
) -> Option<ElementRef<'a>> {
    let mut current = root_element(document);
    for &child_index in path {
        let children = element_children(&current);
        let child = children.get(child_index)?;
        current = *child;
    }
    Some(current)
}

pub(super) fn root_element(document: &Html) -> ElementRef<'_> {
    document
        .select(&SELECTOR_BODY)
        .next()
        .expect("BUG: scraper document should contain a body element")
}

pub(super) fn element_children<'a>(element: &ElementRef<'a>) -> Vec<ElementRef<'a>> {
    element
        .children()
        .filter_map(ElementRef::wrap)
        .filter(|child| !should_skip_element(child))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_element_by_path_returns_root_for_empty_path() {
        let doc = Html::parse_document("<html><body><p>hello</p></body></html>");
        let el = resolve_element_by_path(&doc, &[]).unwrap();
        assert_eq!(el.value().name(), "body");
    }

    #[test]
    fn resolve_element_by_path_follows_child_indices() {
        let doc = Html::parse_document("<html><body><p>one</p><p>two</p></body></html>");
        let el = resolve_element_by_path(&doc, &[1]).unwrap();
        assert_eq!(el.value().name(), "p");
        let text: String = el.text().collect();
        assert_eq!(text, "two");
    }

    #[test]
    fn resolve_element_by_path_returns_none_for_out_of_bounds() {
        let doc = Html::parse_document("<html><body><p>only</p></body></html>");
        assert!(resolve_element_by_path(&doc, &[5]).is_none());
    }

    #[test]
    fn element_children_skips_script_and_style() {
        let doc = Html::parse_document("<html><body><script>js</script><p>keep</p></body></html>");
        let root = root_element(&doc);
        let children = element_children(&root);
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].value().name(), "p");
    }
}
