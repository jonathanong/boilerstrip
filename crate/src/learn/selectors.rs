use scraper::ElementRef;
use std::collections::{HashMap, HashSet};

use super::constants::{
    MAX_SELECTOR_DIGITS, MAX_SELECTOR_TOKEN_LENGTH, MIN_HEX_SEGMENT_LENGTH,
    MIN_SELECTOR_TOKEN_LENGTH,
};
use super::types::SnippetCandidate;

pub(super) fn selector_candidates(element: &ElementRef) -> HashSet<String> {
    let mut selectors = HashSet::new();

    if let Some(id_value) = element.value().id() {
        if is_stable_selector_token(id_value) {
            selectors.insert(format!("#{}", css_escape_identifier(id_value)));
        }
    }

    for class_name in element.value().classes() {
        if is_stable_selector_token(class_name) {
            selectors.insert(format!(".{}", css_escape_identifier(class_name)));
        }
    }

    if let Some(role) = element.value().attr("role") {
        if is_stable_selector_token(role) {
            // Role values go in attribute selectors; no escaping needed for the value
            // (it's a quoted string, not an identifier).
            selectors.insert(format!(r#"[role="{role}"]"#));
        }
    }

    selectors
}

/// Escape a value for use in a CSS selector (class name or ID).
/// Escapes characters that are valid in HTML attributes but need `\` in CSS selectors.
fn css_escape_identifier(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    for (i, ch) in value.char_indices() {
        match ch {
            // First character: digit must be escaped as \3X  (or unicode escape)
            // Actually, use numeric escape for leading digit: \XX
            _ if i == 0 && ch.is_ascii_digit() => {
                out.push('\\');
                out.push('3');
                out.push(ch);
                out.push(' ');
            }
            // Special chars that need backslash-escaping in CSS identifiers
            '!' | '"' | '#' | '$' | '%' | '&' | '\'' | '(' | ')' | '*' | '+' | ',' | '.' | '/'
            | ':' | ';' | '<' | '=' | '>' | '?' | '@' | '[' | '\\' | ']' | '^' | '`' | '{'
            | '|' | '}' | '~' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

pub(super) fn selector_priority(selector: &str) -> usize {
    if selector.starts_with('#') {
        return 30 + selector.len().min(20);
    }
    if selector.starts_with('.') {
        return 20 + selector.len().min(20);
    }
    if selector.starts_with('[') {
        return 10 + selector.len().min(20);
    }
    selector.len().min(20)
}

pub(super) fn is_stable_selector_token(value: &str) -> bool {
    if value.len() < MIN_SELECTOR_TOKEN_LENGTH || value.len() > MAX_SELECTOR_TOKEN_LENGTH {
        return false;
    }
    if !value.chars().any(|ch| ch.is_alphabetic()) {
        return false;
    }
    // Accept alphanumeric, hyphen, underscore, colon, slash, brackets, period
    // (covers Tailwind/utility-CSS class names like `md:hidden`, `w-1/2`)
    if !value
        .chars()
        .all(|ch| ch.is_alphanumeric() || matches!(ch, '-' | '_' | ':' | '/' | '[' | ']' | '.'))
    {
        return false;
    }
    let digit_count = value.chars().filter(char::is_ascii_digit).count();
    if digit_count >= MAX_SELECTOR_DIGITS {
        return false;
    }
    if value.split('-').any(is_hex_like_hash_segment) {
        return false;
    }
    true
}

fn is_hex_like_hash_segment(segment: &str) -> bool {
    segment.len() >= MIN_HEX_SEGMENT_LENGTH && segment.chars().all(|ch| ch.is_ascii_hexdigit())
}

pub(super) fn collect_promoted_selectors(
    snippet_candidates: &HashMap<String, SnippetCandidate>,
    min_shared_pages: usize,
) -> Vec<String> {
    let mut promotable = snippet_candidates
        .values()
        .filter(|c| c.pages_seen.len() >= min_shared_pages)
        .filter_map(|c| c.best_promotable_selector(min_shared_pages))
        .collect::<Vec<_>>();
    promotable.sort_by(|left, right| {
        let order = selector_priority(right).cmp(&selector_priority(left));
        if order == std::cmp::Ordering::Equal {
            left.cmp(right)
        } else {
            order
        }
    });
    promotable.dedup();
    promotable
}

pub(super) fn merge_ranked_and_promoted_selectors(
    ranked_selectors: Vec<String>,
    promoted_selectors: Vec<String>,
) -> Vec<String> {
    let mut selected = Vec::new();
    let mut selected_set = HashSet::new();

    for selector in ranked_selectors {
        if selected_set.insert(selector.clone()) {
            selected.push(selector);
        }
    }
    for selector in promoted_selectors {
        if selected_set.insert(selector.clone()) {
            selected.push(selector);
        }
    }
    selected
}

pub(super) fn shared_selectors_for_samples(
    samples: &[super::types::PathNodeSample],
) -> HashSet<String> {
    if samples.is_empty() {
        return HashSet::new();
    }
    let mut counts: HashMap<String, usize> = HashMap::new();
    for sample in samples {
        for selector in &sample.selectors {
            *counts.entry(selector.clone()).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .filter(|(_, count)| *count == samples.len())
        .map(|(sel, _)| sel)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use scraper::Html;

    use super::*;
    use crate::learn::types::{PathNodeSample, SnippetCandidate};

    #[test]
    fn validates_selector_token_rejection_paths() {
        assert!(!is_stable_selector_token("a")); // too short
        assert!(!is_stable_selector_token("123")); // no letters
        assert!(!is_stable_selector_token("bad!")); // invalid char
        assert!(!is_stable_selector_token("item-1234")); // >= 4 digits
        assert!(!is_stable_selector_token("hash-deadbeef")); // hex segment
        assert!(!is_stable_selector_token(&"a".repeat(65))); // too long
        assert!(is_stable_selector_token("site-nav")); // valid ASCII
        assert!(is_stable_selector_token("año-principal")); // valid Unicode
    }

    #[test]
    fn collects_id_class_and_role_selector_candidates() {
        let doc =
            Html::parse_fragment(r#"<div id="hero" class="card item-1234" role="banner"></div>"#);
        let el = doc
            .select(&scraper::Selector::parse("div").unwrap())
            .next()
            .unwrap();
        let selectors = selector_candidates(&el);
        assert!(selectors.contains("#hero"));
        assert!(selectors.contains(".card"));
        assert!(selectors.contains(r#"[role="banner"]"#));
        assert!(!selectors.contains(".item-1234")); // filtered out

        let doc2 = Html::parse_fragment(r#"<div role="123"></div>"#);
        let el2 = doc2
            .select(&scraper::Selector::parse("div").unwrap())
            .next()
            .unwrap();
        assert!(!selector_candidates(&el2).contains(r#"[role="123"]"#));

        // Element with an unstable id (single char — below MIN_SELECTOR_TOKEN_LENGTH)
        // covers the false branch of `if is_stable_selector_token(id_value)`
        let doc3 = Html::parse_fragment(r#"<div id="x" class="my-class"></div>"#);
        let el3 = doc3
            .select(&scraper::Selector::parse("div").unwrap())
            .next()
            .unwrap();
        let selectors3 = selector_candidates(&el3);
        assert!(!selectors3.contains("#x"));
        assert!(selectors3.contains(".my-class"));
    }

    #[test]
    fn ranks_and_merges_selectors_deterministically() {
        assert!(selector_priority("#hero") > selector_priority(".hero"));
        assert!(selector_priority(".hero") > selector_priority(r#"[role="main"]"#));
        assert_eq!(selector_priority("main"), 4);

        let mut candidates = HashMap::new();
        let mut c1 = SnippetCandidate::default();
        c1.record(
            0,
            "<div></div>".to_string(),
            &HashSet::from([".b".to_string()]),
        );
        c1.record(
            1,
            "<div></div>".to_string(),
            &HashSet::from([".b".to_string()]),
        );
        let mut c2 = SnippetCandidate::default();
        c2.record(
            0,
            "<div></div>".to_string(),
            &HashSet::from(["#a".to_string()]),
        );
        c2.record(
            1,
            "<div></div>".to_string(),
            &HashSet::from(["#a".to_string()]),
        );
        let mut c3 = SnippetCandidate::default();
        c3.record(
            0,
            "<div></div>".to_string(),
            &HashSet::from([".a".to_string()]),
        );
        c3.record(
            1,
            "<div></div>".to_string(),
            &HashSet::from([".a".to_string()]),
        );
        candidates.insert("fp1".to_string(), c1);
        candidates.insert("fp2".to_string(), c2);
        candidates.insert("fp3".to_string(), c3);

        assert_eq!(
            collect_promoted_selectors(&candidates, 2),
            vec!["#a".to_string(), ".a".to_string(), ".b".to_string()]
        );

        assert_eq!(
            merge_ranked_and_promoted_selectors(
                vec![".a".to_string(), ".a".to_string()],
                vec![".a".to_string(), ".b".to_string()]
            ),
            vec![".a".to_string(), ".b".to_string()]
        );
    }

    #[test]
    fn tailwind_class_names_are_accepted_with_escaping() {
        let doc = Html::parse_fragment(r#"<div class="md:hidden hover:text-blue-500"></div>"#);
        let el = doc
            .select(&scraper::Selector::parse("div").unwrap())
            .next()
            .unwrap();
        let selectors = selector_candidates(&el);
        // Both Tailwind classes should be in candidates (broadened filter)
        assert!(
            selectors.iter().any(|s| s.contains("md")),
            "md:hidden should be included"
        );
    }

    #[test]
    fn css_escape_identifier_escapes_colon_and_slash() {
        assert_eq!(css_escape_identifier("md:hidden"), "md\\:hidden");
        assert_eq!(css_escape_identifier("w-1/2"), "w-1\\/2");
    }

    #[test]
    fn css_escape_identifier_escapes_leading_digit() {
        let result = css_escape_identifier("4col");
        assert!(result.starts_with('\\'), "leading digit should be escaped");
    }

    #[test]
    fn shared_selectors_require_all_samples() {
        assert!(shared_selectors_for_samples(&[]).is_empty());

        let samples = vec![
            PathNodeSample {
                page_index: 0,
                text: String::new(),
                fingerprint: String::new(),
                snippet: String::new(),
                selectors: HashSet::from([".shared".to_string(), ".one".to_string()]),
            },
            PathNodeSample {
                page_index: 1,
                text: String::new(),
                fingerprint: String::new(),
                snippet: String::new(),
                selectors: HashSet::from([".shared".to_string()]),
            },
        ];
        assert_eq!(
            shared_selectors_for_samples(&samples),
            HashSet::from([".shared".to_string()])
        );
    }
}
