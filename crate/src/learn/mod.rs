//! Cross-page boilerplate learning and removal.
//!
//! Given multiple HTML pages from the same site, [`learn`] discovers the
//! CSS selectors and verbatim HTML snippets that recur as boilerplate
//! (navigation, footers, cookie banners, etc.).  The resulting [`Removals`]
//! can be fed into [`crate::convert`] to strip that boilerplate before
//! converting to Markdown.
//!
//! ## Quick start
//!
//! ```rust
//! use boilerstrip::{learn, apply_removals, LearnOptions};
//!
//! let pages = vec![
//!     "<html><nav class=\"site-nav\">Menu</nav><main>Page 1</main></html>".to_string(),
//!     "<html><nav class=\"site-nav\">Menu</nav><main>Page 2</main></html>".to_string(),
//!     "<html><nav class=\"site-nav\">Menu</nav><main>Page 3</main></html>".to_string(),
//! ];
//! let removals = learn(&pages, &LearnOptions::default()).unwrap();
//! assert!(removals.css_selectors_to_remove.iter().any(|s| s == ".site-nav"));
//! ```

pub(crate) mod apply;
mod constants;
mod dom;
mod fingerprint;
mod selectors;
mod snippets;
pub mod types;

use rayon::prelude::*;
use scraper::Html;
use std::collections::{HashMap, HashSet};

pub use apply::{apply_removals, apply_removals_compiled, CompiledRemovals};
pub use types::{LearnError, LearnOptions, Removals};

use constants::SELECTOR_ALL_ELEMENTS;
use fingerprint::{normalize_whitespace, normalized_text_fingerprint};
use selectors::{
    collect_promoted_selectors, merge_ranked_and_promoted_selectors, selector_candidates,
    selector_priority,
};
use snippets::{
    collect_breadth_first_snippet_candidates, should_skip_element,
    snippet_contains_selected_selector,
};
use types::{LearnConfig, ScoredSelector, SelectorStats, SnippetCandidate};

/// Analyze a set of HTML pages from the same site and identify boilerplate elements.
///
/// Requires at least 2 pages. Returns [`LearnError::TooFewPages`] otherwise.
///
/// The algorithm has two stages:
/// 1. **Selector stage** — find CSS selectors (id/class/role) whose content
///    fingerprint is stable across pages.
/// 2. **Snippet stage** — BFS traversal to find shared text nodes that match
///    boilerplate patterns but lack stable selectors.
pub fn learn(pages: &[String], options: &LearnOptions) -> Result<Removals, LearnError> {
    if pages.len() < 2 {
        return Err(LearnError::TooFewPages(pages.len()));
    }

    let config = LearnConfig::from_options(options);
    let min_shared_pages = minimum_shared_page_count(pages.len());

    let selector_stats = collect_selector_stats(pages);

    let boilerplate_patterns = resolve_boilerplate_patterns(options);
    let snippet_candidates = collect_breadth_first_snippet_candidates(
        pages,
        min_shared_pages,
        &boilerplate_patterns,
        &config,
    );

    let ranked_selectors = rank_selectors(&selector_stats, min_shared_pages, &config);

    let promoted_selectors = collect_promoted_selectors(&snippet_candidates, min_shared_pages);
    let css_selectors_to_remove =
        merge_ranked_and_promoted_selectors(ranked_selectors, promoted_selectors);

    let selected_selectors: HashSet<_> = css_selectors_to_remove.iter().cloned().collect();

    let html_to_remove =
        filter_snippet_candidates(snippet_candidates, min_shared_pages, &selected_selectors);

    Ok(Removals {
        css_selectors_to_remove,
        html_to_remove,
    })
}

fn collect_selector_stats(pages: &[String]) -> HashMap<String, SelectorStats> {
    // Parse and score each page in parallel, then merge the results in a parallel reduction.
    type PageStats = Vec<(String, usize, String)>; // (selector, page_index, fingerprint)
    let per_page: Vec<PageStats> = pages
        .par_iter()
        .enumerate()
        .map(|(page_index, page_html)| {
            let document = Html::parse_document(page_html);
            let mut entries = Vec::new();
            for element in document.select(&SELECTOR_ALL_ELEMENTS) {
                if should_skip_element(&element) {
                    continue;
                }
                let text = normalize_whitespace(&element.text().collect::<Vec<_>>().join(" "));
                if text.is_empty() {
                    continue;
                }
                let fingerprint = normalized_text_fingerprint(&text);
                if fingerprint.is_empty() {
                    continue;
                }
                for selector in selector_candidates(&element) {
                    entries.push((selector, page_index, fingerprint.clone()));
                }
            }
            entries
        })
        .collect();

    per_page
        .into_par_iter()
        .fold(
            HashMap::new,
            |mut acc: HashMap<String, SelectorStats>, entries| {
                for (selector, page_index, fingerprint) in entries {
                    acc.entry(selector)
                        .or_insert_with(SelectorStats::new)
                        .record(page_index, &fingerprint);
                }
                acc
            },
        )
        .reduce(HashMap::new, |mut a, b| {
            for (k, v) in b {
                a.entry(k).or_insert_with(SelectorStats::new).merge(v);
            }
            a
        })
}

fn rank_selectors(
    selector_stats: &HashMap<String, SelectorStats>,
    min_shared_pages: usize,
    config: &LearnConfig,
) -> Vec<String> {
    let mut scored_css_selectors = Vec::new();
    for (selector, stats) in selector_stats {
        if let Some(score) = stats.score(min_shared_pages, config) {
            scored_css_selectors.push(ScoredSelector {
                selector: selector.clone(),
                score,
            });
        }
    }

    scored_css_selectors.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| {
                selector_priority(&right.selector).cmp(&selector_priority(&left.selector))
            })
            .then_with(|| left.selector.cmp(&right.selector))
    });

    scored_css_selectors
        .into_iter()
        .map(|item| item.selector)
        .collect::<Vec<_>>()
}

fn filter_snippet_candidates(
    snippet_candidates: HashMap<String, SnippetCandidate>,
    min_shared_pages: usize,
    selected_selectors: &HashSet<String>,
) -> Vec<String> {
    let mut html_to_remove = snippet_candidates
        .into_values()
        .filter(|candidate| {
            candidate.pages_seen.len() >= min_shared_pages
                && candidate.selectors_seen.is_disjoint(selected_selectors)
                && !snippet_contains_selected_selector(&candidate.snippet, selected_selectors)
        })
        .map(|candidate| candidate.snippet)
        .collect::<Vec<_>>();

    html_to_remove.sort_by(compare_snippets_for_removal);
    html_to_remove.dedup();

    html_to_remove
}

/// Return a list of patterns marking common boilerplate text.
pub fn default_boilerplate_patterns() -> Vec<String> {
    vec![
        "sign in".to_string(),
        "contact us".to_string(),
        "opens in a new window".to_string(),
        "privacy".to_string(),
        "terms".to_string(),
        "cookie".to_string(),
        "legal".to_string(),
        "disclaimer".to_string(),
        "all rights reserved".to_string(),
        "fdic".to_string(),
        "equal housing lender".to_string(),
        "member fdic".to_string(),
    ]
}

fn resolve_boilerplate_patterns(options: &LearnOptions) -> Vec<String> {
    match &options.boilerplate_patterns {
        None => default_boilerplate_patterns(),
        Some(patterns) => patterns.clone(),
    }
}

fn minimum_shared_page_count(page_count: usize) -> usize {
    // ≥⅔ of pages, rounded up, minimum 2
    let two_thirds = (page_count.saturating_mul(2).saturating_add(2)) / 3;
    two_thirds.max(2)
}

#[allow(clippy::ptr_arg)]
fn compare_snippets_for_removal(left: &String, right: &String) -> std::cmp::Ordering {
    right.len().cmp(&left.len()).then_with(|| left.cmp(right))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn learn_requires_at_least_two_pages() {
        let LearnError::TooFewPages(n) = learn(&[], &LearnOptions::default()).unwrap_err();
        assert_eq!(n, 0);

        let LearnError::TooFewPages(n) =
            learn(&["<html></html>".to_string()], &LearnOptions::default()).unwrap_err();
        assert_eq!(n, 1);
    }

    #[test]
    fn learn_detects_shared_nav_selector() {
        let pages = vec![
            "<html><body><nav class=\"site-nav\">Menu</nav><main>Page one</main></body></html>"
                .to_string(),
            "<html><body><nav class=\"site-nav\">Menu</nav><main>Page two</main></body></html>"
                .to_string(),
        ];
        let removals = learn(&pages, &LearnOptions::default()).unwrap();
        assert!(removals
            .css_selectors_to_remove
            .iter()
            .any(|s| s == ".site-nav"));
    }

    #[test]
    fn learn_accepts_custom_boilerplate_patterns() {
        let pages = vec![
            "<html><body><p class=\"ad-banner\">Buy now for great deals!</p><main>Content one</main></body></html>"
                .to_string(),
            "<html><body><p class=\"ad-banner\">Buy now for great deals!</p><main>Content two</main></body></html>"
                .to_string(),
        ];
        let options = LearnOptions {
            boilerplate_patterns: Some(vec!["buy now".to_string()]),
            ..Default::default()
        };
        let removals = learn(&pages, &options).unwrap();
        assert!(removals
            .css_selectors_to_remove
            .iter()
            .any(|s| s == ".ad-banner"));
    }

    #[test]
    fn learn_with_empty_boilerplate_patterns_yields_no_snippets() {
        let pages = vec![
            "<html><body><footer>Footer text</footer><main>Content</main></body></html>"
                .to_string(),
            "<html><body><footer>Footer text</footer><main>Content</main></body></html>"
                .to_string(),
        ];
        let options = LearnOptions {
            boilerplate_patterns: Some(vec![]),
            ..Default::default()
        };
        let removals = learn(&pages, &options).unwrap();
        assert!(removals.html_to_remove.is_empty());
    }

    #[test]
    fn compare_snippets_sorts_longer_then_lexical() {
        let longer = "<p>longer</p>".to_string();
        let shorter = "<p>short</p>".to_string();
        let left = "<p>a</p>".to_string();
        let right = "<p>b</p>".to_string();

        assert_eq!(
            compare_snippets_for_removal(&longer, &shorter),
            Ordering::Less
        );
        assert_eq!(
            compare_snippets_for_removal(&shorter, &longer),
            Ordering::Greater
        );
        assert_eq!(compare_snippets_for_removal(&left, &right), Ordering::Less);
    }

    #[test]
    fn minimum_shared_page_count_is_at_least_two() {
        assert_eq!(minimum_shared_page_count(2), 2);
        assert_eq!(minimum_shared_page_count(3), 2);
        assert_eq!(minimum_shared_page_count(6), 4);
    }

    #[test]
    fn default_boilerplate_patterns_are_non_empty() {
        assert!(!default_boilerplate_patterns().is_empty());
    }

    #[test]
    fn learn_skips_script_and_style_elements() {
        let pages = vec![
            "<html><head><script>var x=1;</script><style>.a{}</style></head><body><nav class=\"shared-nav\">Menu</nav></body></html>".to_string(),
            "<html><head><script>var x=2;</script><style>.b{}</style></head><body><nav class=\"shared-nav\">Menu</nav></body></html>".to_string(),
        ];
        let removals = learn(&pages, &LearnOptions::default()).unwrap();
        assert!(removals
            .css_selectors_to_remove
            .iter()
            .any(|s| s == ".shared-nav"));
    }

    #[test]
    fn learn_skips_elements_with_empty_fingerprint() {
        let pages = vec![
            "<html><body><p>...</p><nav class=\"shared-nav\">Menu</nav></body></html>".to_string(),
            "<html><body><p>---</p><nav class=\"shared-nav\">Menu</nav></body></html>".to_string(),
        ];
        let removals = learn(&pages, &LearnOptions::default()).unwrap();
        assert!(removals
            .css_selectors_to_remove
            .iter()
            .any(|s| s == ".shared-nav"));
    }

    #[test]
    fn learn_handles_selectors_with_no_score() {
        let pages = vec![
            "<html><body><div class=\"only-page-one\">Content unique to page one</div></body></html>".to_string(),
            "<html><body><div>Content unique to page two</div></body></html>".to_string(),
        ];
        let removals = learn(&pages, &LearnOptions::default()).unwrap();
        assert!(!removals
            .css_selectors_to_remove
            .contains(&".only-page-one".to_string()));
    }
}
