use std::borrow::Cow;

use lol_html::html_content::Element;
use lol_html::{ElementContentHandlers, HtmlRewriter, Settings};

/// Single streaming pass over raw HTML that removes elements matching
/// `<script>`, `<style>`, and any additional selectors in `remove_selectors`.
///
/// Silently skips selectors that lol_html cannot parse.
pub fn strip_elements(html: &str, remove_selectors: &[String]) -> Vec<u8> {
    strip_elements_with_iter(html, remove_selectors.iter())
}

fn build_removal_handlers<'a, I, S>(
    remove_selectors: I,
) -> Vec<(Cow<'a, lol_html::Selector>, ElementContentHandlers<'a>)>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut handlers = vec![(
        Cow::Owned(
            "script, style"
                .parse::<lol_html::Selector>()
                .expect("BUG: hardcoded selector invalid"),
        ),
        ElementContentHandlers::default().element(|el: &mut Element<'_, '_, _>| {
            el.remove();
            Ok(())
        }),
    )];

    for sel_str in remove_selectors {
        if let Ok(selector) = sel_str.as_ref().parse::<lol_html::Selector>() {
            handlers.push((
                Cow::Owned(selector),
                ElementContentHandlers::default().element(|el: &mut Element<'_, '_, _>| {
                    el.remove();
                    Ok(())
                }),
            ));
        }
    }

    handlers
}

pub(crate) fn strip_elements_with_iter<I, S>(html: &str, remove_selectors: I) -> Vec<u8>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut output = Vec::with_capacity(html.len());

    let element_content_handlers = build_removal_handlers(remove_selectors);

    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers,
            ..Settings::default()
        },
        |c: &[u8]| output.extend_from_slice(c),
    );

    // Our handlers always return Ok.  Ignore any lol_html internal error rather
    // than panicking; `output` may be partial in that case, which is acceptable
    // (the DOM parse step that follows will still produce usable output).
    rewriter.write(html.as_bytes()).ok();
    rewriter.end().ok();
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_removes_script_and_style() {
        let html = "<html><head><script>evil()</script></head><body><p>Keep</p></body></html>";
        let out = String::from_utf8(strip_elements(html, &[])).unwrap();
        assert!(!out.contains("evil") && out.contains("Keep"));
    }

    #[test]
    fn strip_removes_matching_selector() {
        let html = "<html><body><nav class=\"nav\">Menu</nav><p>Content</p></body></html>";
        let out = String::from_utf8(strip_elements(html, &[".nav".to_string()])).unwrap();
        assert!(!out.contains("Menu") && out.contains("Content"));
    }

    #[test]
    fn strip_skips_invalid_selector() {
        // ">>" is not a valid CSS selector; lol_html should reject it, so strip gracefully skips it
        let out = String::from_utf8(strip_elements("<p>Keep</p>", &[">>".to_string()])).unwrap();
        assert!(out.contains("Keep"));
    }
}
