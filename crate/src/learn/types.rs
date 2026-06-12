use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::constants::{
    MAX_SELECTOR_MATCHES_PER_PAGE, MAX_SNIPPET_TEXT_LENGTH, MIN_SELECTOR_AVERAGE_STABLE_RATIO,
    MIN_SELECTOR_PER_PAGE_STABLE_RATIO, MIN_SNIPPET_TEXT_LENGTH,
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

    /// Override the maximum times a selector can match per page before it is
    /// considered too broad. Defaults to `20`.
    pub max_selector_matches_per_page: Option<usize>,

    /// Override the minimum average stable-match ratio across all pages.
    /// Defaults to `0.6`.
    pub min_selector_average_stable_ratio: Option<f64>,

    /// Override the minimum per-page stable-match ratio.
    /// Defaults to `0.35`.
    pub min_selector_per_page_stable_ratio: Option<f64>,

    /// Override the minimum text length for a snippet to qualify as boilerplate.
    /// Defaults to `40`.
    pub min_snippet_text_length: Option<usize>,

    /// Override the maximum text length for a snippet to qualify as boilerplate.
    /// Defaults to `240`.
    pub max_snippet_text_length: Option<usize>,
}

/// Resolved configuration derived from [`LearnOptions`] and module-level defaults.
pub(super) struct LearnConfig {
    pub max_selector_matches_per_page: usize,
    pub min_selector_average_stable_ratio: f64,
    pub min_selector_per_page_stable_ratio: f64,
    pub min_snippet_text_length: usize,
    pub max_snippet_text_length: usize,
}

impl LearnConfig {
    pub fn from_options(options: &LearnOptions) -> Self {
        Self {
            max_selector_matches_per_page: options
                .max_selector_matches_per_page
                .unwrap_or(MAX_SELECTOR_MATCHES_PER_PAGE),
            min_selector_average_stable_ratio: options
                .min_selector_average_stable_ratio
                .unwrap_or(MIN_SELECTOR_AVERAGE_STABLE_RATIO),
            min_selector_per_page_stable_ratio: options
                .min_selector_per_page_stable_ratio
                .unwrap_or(MIN_SELECTOR_PER_PAGE_STABLE_RATIO),
            min_snippet_text_length: options
                .min_snippet_text_length
                .unwrap_or(MIN_SNIPPET_TEXT_LENGTH),
            max_snippet_text_length: options
                .max_snippet_text_length
                .unwrap_or(MAX_SNIPPET_TEXT_LENGTH),
        }
    }
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

    pub fn merge(&mut self, other: SelectorStats) {
        for (page_index, count) in other.page_total_matches {
            *self.page_total_matches.entry(page_index).or_insert(0) += count;
        }
        for (page_index, fingerprint_matches) in other.page_fingerprint_matches {
            let entry = self.page_fingerprint_matches.entry(page_index).or_default();
            for (fp, count) in fingerprint_matches {
                *entry.entry(fp).or_insert(0) += count;
            }
        }
    }

    pub fn score(&self, min_shared_pages: usize, config: &LearnConfig) -> Option<u64> {
        if self.page_total_matches.len() < min_shared_pages {
            return None;
        }

        if self
            .page_total_matches
            .values()
            .any(|count| *count > config.max_selector_matches_per_page)
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
        if average_ratio < config.min_selector_average_stable_ratio
            || min_ratio < config.min_selector_per_page_stable_ratio
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
        let mut page_counts: HashMap<&String, usize> = HashMap::new();
        for fingerprint_matches in self.page_fingerprint_matches.values() {
            for fingerprint in fingerprint_matches.keys() {
                *page_counts.entry(fingerprint).or_insert(0) += 1;
            }
        }
        page_counts
            .into_iter()
            .filter(|(_, count)| *count >= min_shared_pages)
            .map(|(fp, _)| fp.clone())
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
            if !self.selectors_seen.contains(selector) {
                self.selectors_seen.insert(selector.clone());
            }
            if let Some(pages) = self.selector_pages.get_mut(selector) {
                pages.insert(page_index);
            } else {
                self.selector_pages
                    .insert(selector.clone(), HashSet::from([page_index]));
            }
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

    fn default_config() -> LearnConfig {
        LearnConfig::from_options(&LearnOptions::default())
    }

    #[test]
    fn selector_stats_rejects_no_shared_fingerprints() {
        let cfg = default_config();
        let mut stats = SelectorStats::new();

        // Add matches to 2 different pages, but with different fingerprints
        stats.record(0, "fp1");
        stats.record(1, "fp2");

        // They share no fingerprints, so it should return None
        assert!(stats.score(2, &cfg).is_none());
    }

    #[test]
    fn selector_stats_scores_and_rejects_unstable_inputs() {
        let cfg = default_config();
        let mut stats = SelectorStats::new();
        assert!(stats.score(2, &cfg).is_none());
        stats.record(0, "same");
        stats.record(1, "same");
        assert!(stats.score(2, &cfg).is_some());

        let mut too_many = SelectorStats::new();
        for _ in 0..21 {
            too_many.record(0, "same");
        }
        too_many.record(1, "same");
        assert!(too_many.score(2, &cfg).is_none());
    }

    #[test]
    fn selector_stats_rejects_low_average_ratio() {
        let cfg = default_config();
        let mut stats = SelectorStats::new();
        for i in 0..10 {
            stats.record(0, &format!("unique-{i}"));
        }
        stats.record(1, "same");
        assert!(stats.score(2, &cfg).is_none());
    }

    #[test]
    fn selector_stats_rejects_low_min_ratio_with_good_average() {
        // 3 pages: pages 1 and 2 are perfectly stable (ratio=1.0), page 0 has
        // ratio=0.1 (1 shared match out of 10 total).  Average ≈ 0.7 ≥ 0.6
        // (first condition is false) but min=0.1 < 0.35 (second condition is
        // true) → triggers the `|| min_ratio < min_selector_per_page_stable_ratio` branch.
        let cfg = default_config();
        let mut stats = SelectorStats::new();
        stats.record(0, "shared");
        for i in 0..9 {
            stats.record(0, &format!("noise-{i}"));
        }
        stats.record(1, "shared");
        stats.record(2, "shared");
        assert!(stats.score(2, &cfg).is_none());
    }

    #[test]
    fn learn_config_overrides_defaults() {
        // A selector that matches 25 times (> default 20) is accepted when the
        // override raises the cap.
        let options = LearnOptions {
            max_selector_matches_per_page: Some(30),
            ..Default::default()
        };
        let cfg = LearnConfig::from_options(&options);
        let mut stats = SelectorStats::new();
        for _ in 0..25 {
            stats.record(0, "same");
        }
        stats.record(1, "same");
        assert!(stats.score(2, &cfg).is_some());
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

    #[test]
    fn selector_stats_merge_combines_counts() {
        let mut a = SelectorStats::new();
        a.record(0, "fp1");
        a.record(0, "fp1");
        a.record(1, "fp2");

        let mut b = SelectorStats::new();
        b.record(0, "fp1");
        b.record(2, "fp3");

        a.merge(b);

        // page 0 total = 2 + 1 = 3
        assert_eq!(*a.page_total_matches.get(&0).unwrap(), 3);
        // page 1 total = 1
        assert_eq!(*a.page_total_matches.get(&1).unwrap(), 1);
        // page 2 total = 1
        assert_eq!(*a.page_total_matches.get(&2).unwrap(), 1);
        // page 0 fingerprint fp1 = 2 + 1 = 3
        assert_eq!(
            *a.page_fingerprint_matches
                .get(&0)
                .unwrap()
                .get("fp1")
                .unwrap(),
            3
        );
    }
}
