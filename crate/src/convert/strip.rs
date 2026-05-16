use std::borrow::Cow;

use lol_html::html_content::Element;
use lol_html::{ElementContentHandlers, HtmlRewriter, Settings};

/// Single streaming pass over raw HTML that removes elements matching
/// `<script>`, `<style>`, and any additional selectors in `remove_selectors`.
///
/// Silently skips selectors that lol_html cannot parse.
/// Falls back to returning the original bytes on rewriting errors.
pub fn strip_elements(html: &str, remove_selectors: &[String]) -> Vec<u8> {
    match try_strip(html, remove_selectors) {
        Ok(out) => out,
        Err(_) => html.as_bytes().to_vec(),
    }
}

fn try_strip(
    html: &str,
    remove_selectors: &[String],
) -> Result<Vec<u8>, lol_html::errors::RewritingError> {
    let mut output = Vec::with_capacity(html.len());

    let mut element_content_handlers: Vec<(
        Cow<'_, lol_html::Selector>,
        ElementContentHandlers<'_>,
    )> = vec![(
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
        if let Ok(selector) = sel_str.parse::<lol_html::Selector>() {
            element_content_handlers.push((
                Cow::Owned(selector),
                ElementContentHandlers::default().element(|el: &mut Element<'_, '_, _>| {
                    el.remove();
                    Ok(())
                }),
            ));
        }
    }

    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers,
            ..Settings::default()
        },
        |c: &[u8]| output.extend_from_slice(c),
    );

    rewriter.write(html.as_bytes())?;
    rewriter.end()?;
    Ok(output)
}
