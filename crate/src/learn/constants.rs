use scraper::Selector;
use std::sync::LazyLock;

pub(super) static SELECTOR_ALL_ELEMENTS: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("*").expect("BUG: invalid SELECTOR_ALL_ELEMENTS"));
pub(super) static SELECTOR_BODY: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("body").expect("BUG: invalid SELECTOR_BODY"));

/// Minimum text length for a snippet to qualify as boilerplate (shorter = likely noise).
pub(super) const MIN_SNIPPET_TEXT_LENGTH: usize = 40;
/// Maximum text length for a snippet to qualify as boilerplate (longer = likely content).
pub(super) const MAX_SNIPPET_TEXT_LENGTH: usize = 240;

/// Maximum times a selector can match per page before being considered too broad.
pub(super) const MAX_SELECTOR_MATCHES_PER_PAGE: usize = 20;
/// Minimum average ratio of stable matches across all pages (0.6 = 60% stable).
pub(super) const MIN_SELECTOR_AVERAGE_STABLE_RATIO: f64 = 0.6;
/// Minimum ratio of stable matches for any single page (0.35 = 35% stable).
pub(super) const MIN_SELECTOR_PER_PAGE_STABLE_RATIO: f64 = 0.35;

/// Maximum words to include in a text fingerprint.
pub(super) const MAX_FINGERPRINT_WORDS: usize = 32;
/// Maximum digit count allowed in a stable selector token (4+ suggests a dynamic ID).
pub(super) const MAX_SELECTOR_DIGITS: usize = 4;
/// Minimum hex segment length to be considered a hash (e.g. `"a1b2c3d4"`).
pub(super) const MIN_HEX_SEGMENT_LENGTH: usize = 8;

/// Minimum length for a stable selector token (too short = too generic).
pub(super) const MIN_SELECTOR_TOKEN_LENGTH: usize = 2;
/// Maximum length for a stable selector token (too long = likely obfuscated).
pub(super) const MAX_SELECTOR_TOKEN_LENGTH: usize = 64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_body_matches_body_element() {
        let document = scraper::Html::parse_document("<html><body></body></html>");
        assert_eq!(document.select(&SELECTOR_BODY).count(), 1);
    }

    #[test]
    fn selector_all_elements_matches_every_element() {
        let document = scraper::Html::parse_document("<html><body><p>hi</p></body></html>");
        assert!(document.select(&SELECTOR_ALL_ELEMENTS).count() > 0);
    }
}
