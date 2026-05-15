use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::constants::{
    MAX_SELECTOR_MATCHES_PER_PAGE, MIN_SELECTOR_AVERAGE_STABLE_RATIO,
    MIN_SELECTOR_PER_PAGE_STABLE_RATIO,
};
use super::selectors::selector_priority;

/// Configuration for the boilerplate learning step.
#[derive(Clone, Debug, Default)]
pub struct LearnOptions {
    /// Text patterns (case-insensitive) that suggest boilerplate content.
    ///
    /// - `None` (default): use the built-in patterns.
    /// - `Some(patterns)`: replace the defaults entirely.
    /// - `Some(vec![])`: disable pattern matching.
    pub boilerplate_patterns: Option<Vec<String>>,
}

/// Boilerplate selectors and snippets discovered from a set of HTML pages.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Removals {
    /// CSS selectors that match boilerplate elements (e.g. `".footer"`, `"#nav"`).
    pub css_selectors_to_remove: Vec<String>,
    /// Verbatim HTML snippets that appear across pages but lack stable selectors.
    pub html_to_remove: Vec<String>,
}

/// Error returned by [`learn`][super::learn].
#[derive(Debug, Error)]
pub enum LearnError {
    #[error("learn requires at least 2 HTML pages, got {0}")]
    TooFewPages(usize),
}

/// Error returned by [`apply_removals`][super::apply_removals].
#[derive(Debug, Error)]
pub enum ApplyError {}

// ── Internal scoring types ───────────────────────────────────────────────────

pub(super) struct SelectorStats {
    pub page_total_matches: HashMap<usize, usize>,
    pub page_fingerprint_matches: HashMap<usize, HashMap<String, usize>>,
}

impl SelectorStats {
    pub fn new() -> Self {
        Self {
            page_total_matches: HashMap::new(),
            page_fingerprint_matches: HashMap::new(),
        }
    }

    pub fn record(&mut self, page_index: usize, fingerprint: &str) {
        *self.page_total_matches.entry(page_index).or_insert(0) += 1;
        *self
            .page_fingerprint_matches
            .entry(page_index)
            .or_default()
            .entry(fingerprint.to_string())
            .or_insert(0) += 1;
    }

    pub fn score(&self, min_shared_pages: usize) -> Option<u64> {
        if self.page_total_matches.len() < min_shared_pages {
            return None;
        }

        if self
            .page_total_matches
            .values()
            .any(|count| *count > MAX_SELECTOR_MATCHES_PER_PAGE)
        {
            return None;
        }

        let shared_fingerprints = self.shared_fingerprints(min_shared_pages);
        if shared_fingerprints.is_empty() {
            return None;
        }

        let mut ratio_sum = 0.0_f64;
        let mut min_ratio = 1.0_f64;
        for (page_index, total_matches) in &self.page_total_matches {
            let stable_matches =
                self.page_fingerprint_matches
                    .get(page_index)
                    .map_or(0, |fingerprint_matches| {
                        shared_fingerprints
                            .iter()
                            .map(|fp| fingerprint_matches.get(fp).copied().unwrap_or(0))
                            .sum::<usize>()
                    });

            let stable_ratio = stable_matches as f64 / (*total_matches as f64);
            ratio_sum += stable_ratio;
            min_ratio = min_ratio.min(stable_ratio);
        }

        let average_ratio = ratio_sum / (self.page_total_matches.len() as f64);
        if average_ratio < MIN_SELECTOR_AVERAGE_STABLE_RATIO
            || min_ratio < MIN_SELECTOR_PER_PAGE_STABLE_RATIO
        {
            return None;
        }

        Some(
            (self.page_total_matches.len() as u64) * 1_000
                + (shared_fingerprints.len() as u64) * 100
                + (average_ratio * 100.0) as u64,
        )
    }

    fn shared_fingerprints(&self, min_shared_pages: usize) -> HashSet<String> {
        let mut page_counts: HashMap<String, usize> = HashMap::new();
        for fingerprint_matches in self.page_fingerprint_matches.values() {
            for fingerprint in fingerprint_matches.keys() {
                *page_counts.entry(fingerprint.clone()).or_insert(0) += 1;
            }
        }
        page_counts
            .into_iter()
            .filter(|(_, count)| *count >= min_shared_pages)
            .map(|(fp, _)| fp)
            .collect()
    }
}

#[derive(Default)]
pub(super) struct SnippetCandidate {
    pub pages_seen: HashSet<usize>,
    pub selectors_seen: HashSet<String>,
    pub selector_pages: HashMap<String, HashSet<usize>>,
    pub snippet: String,
}

impl SnippetCandidate {
    pub fn record(&mut self, page_index: usize, snippet: String, selectors: &HashSet<String>) {
        self.pages_seen.insert(page_index);
        for selector in selectors {
            self.selectors_seen.insert(selector.clone());
            self.selector_pages
                .entry(selector.clone())
                .or_default()
                .insert(page_index);
        }
        if snippet.len() > self.snippet.len() {
            self.snippet = snippet;
        }
    }

    pub fn best_promotable_selector(&self, min_shared_pages: usize) -> Option<String> {
        self.selector_pages
            .iter()
            .filter(|(_, pages)| pages.len() >= min_shared_pages)
            .map(|(selector, _)| selector.clone())
            .max_by(|left, right| {
                let order = selector_priority(left).cmp(&selector_priority(right));
                if order == std::cmp::Ordering::Equal {
                    right.cmp(left)
                } else {
                    order
                }
            })
    }
}

pub(super) struct ScoredSelector {
    pub selector: String,
    pub score: u64,
}

pub(super) struct PathNodeSample {
    pub page_index: usize,
    pub text: String,
    pub fingerprint: String,
    pub snippet: String,
    pub selectors: HashSet<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_stats_scores_and_rejects_unstable_inputs() {
        let mut stats = SelectorStats::new();
        assert!(stats.score(2).is_none());
        stats.record(0, "same");
        stats.record(1, "same");
        assert!(stats.score(2).is_some());

        let mut too_many = SelectorStats::new();
        for _ in 0..21 {
            too_many.record(0, "same");
        }
        too_many.record(1, "same");
        assert!(too_many.score(2).is_none());
    }

    #[test]
    fn selector_stats_rejects_low_average_ratio() {
        let mut stats = SelectorStats::new();
        // page 0: 10 total matches, 1 stable (10% ratio, below MIN_SELECTOR_PER_PAGE_STABLE_RATIO)
        for i in 0..10 {
            stats.record(0, &format!("unique-{i}"));
        }
        stats.record(1, "same");
        // Not enough stable ratio on page 0
        assert!(stats.score(2).is_none());
    }

    #[test]
    fn selector_stats_rejects_low_min_ratio_with_good_average() {
        // 3 pages: pages 1 and 2 are perfectly stable (ratio=1.0), page 0 has
        // ratio=0.1 (1 shared match out of 10 total).  Average ≈ 0.7 ≥ 0.6
        // (first condition is false) but min=0.1 < 0.35 (second condition is
        // true) → triggers the `|| min_ratio < MIN_SELECTOR_PER_PAGE_STABLE_RATIO`
        // branch at types.rs:108.
        let mut stats = SelectorStats::new();
        stats.record(0, "shared");
        for i in 0..9 {
            stats.record(0, &format!("noise-{i}"));
        }
        stats.record(1, "shared");
        stats.record(2, "shared");
        assert!(stats.score(2).is_none());
    }

    #[test]
    fn snippet_candidate_records_and_selects_best_promotable_selector() {
        let mut candidate = SnippetCandidate::default();
        candidate.record(
            0,
            "<p>a</p>".to_string(),
            &HashSet::from([".a".to_string()]),
        );
        candidate.record(
            1,
            "<section>longer</section>".to_string(),
            &HashSet::from([
                "#hero".to_string(),
                ".a".to_string(),
                r#"[role="main"]"#.to_string(),
            ]),
        );
        candidate.record(
            2,
            "<section>longest text here</section>".to_string(),
            &HashSet::from(["#hero".to_string(), r#"[role="main"]"#.to_string()]),
        );
        assert_eq!(candidate.snippet, "<section>longest text here</section>");
        assert_eq!(
            candidate.best_promotable_selector(2),
            Some("#hero".to_string())
        );

        candidate.selector_pages = HashMap::from([
            (".b".to_string(), HashSet::from([0, 1])),
            (".a".to_string(), HashSet::from([0, 1])),
        ]);
        assert_eq!(
            candidate.best_promotable_selector(2),
            Some(".a".to_string())
        );

        candidate.selector_pages = HashMap::new();
        assert!(candidate.best_promotable_selector(2).is_none());
    }

    #[test]
    fn learn_error_display() {
        let err = LearnError::TooFewPages(1);
        assert!(err.to_string().contains("1"));
    }
}
