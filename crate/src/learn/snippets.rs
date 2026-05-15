use scraper::{ElementRef, Html};
use std::collections::{HashMap, HashSet, VecDeque};

use super::constants::{MAX_SNIPPET_TEXT_LENGTH, MIN_SNIPPET_TEXT_LENGTH, SELECTOR_ALL_ELEMENTS};
use super::dom::{element_children, resolve_element_by_path};
use super::fingerprint::{normalize_whitespace, normalized_text_fingerprint};
use super::selectors::{selector_candidates, shared_selectors_for_samples};
use super::types::{PathNodeSample, SnippetCandidate};

pub(super) fn collect_breadth_first_snippet_candidates(
    html_pages: &[String],
    min_shared_pages: usize,
    boilerplate_patterns: &[String],
) -> HashMap<String, SnippetCandidate> {
    let documents = html_pages
        .iter()
        .map(|html| Html::parse_document(html))
        .collect::<Vec<_>>();
    let mut candidates: HashMap<String, SnippetCandidate> = HashMap::new();
    let mut queue = VecDeque::new();
    queue.push_back(Vec::<usize>::new());

    while let Some(path) = queue.pop_front() {
        let mut samples = Vec::new();
        for (page_index, document) in documents.iter().enumerate() {
            if let Some(element) = resolve_element_by_path(document, &path) {
                let text = normalize_whitespace(&element.text().collect::<Vec<_>>().join(" "));
                samples.push(PathNodeSample {
                    page_index,
                    fingerprint: normalized_text_fingerprint(&text),
                    text,
                    snippet: element.html(),
                    selectors: selector_candidates(&element),
                });
            }
        }

        let shared_selectors = shared_selectors_for_samples(&samples);
        let shared_fingerprint = shared_fingerprint_for_samples(&samples, min_shared_pages);
        let is_match = !shared_selectors.is_empty() || shared_fingerprint.is_some();

        if let Some(fingerprint) = shared_fingerprint.filter(|_| !path.is_empty() && is_match) {
            let longest_text = samples
                .iter()
                .max_by_key(|s| s.text.len())
                .map(|s| s.text.clone())
                .unwrap_or_default();

            if is_boilerplate_text_candidate(&longest_text, boilerplate_patterns) {
                let candidate = candidates.entry(fingerprint).or_default();
                for sample in samples {
                    candidate.record(sample.page_index, sample.snippet, &sample.selectors);
                }
                // matched — do not recurse into children
                continue;
            }
        }

        if path.is_empty() || !is_match {
            let mut child_index_counts = HashMap::<usize, usize>::new();
            for element in documents
                .iter()
                .filter_map(|document| resolve_element_by_path(document, &path))
            {
                let child_count = element_children(&element).len();
                for child_index in 0..child_count {
                    *child_index_counts.entry(child_index).or_insert(0) += 1;
                }
            }

            let mut next_child_indices = child_index_counts
                .into_iter()
                .filter(|(_, count)| *count >= min_shared_pages)
                .map(|(child_index, _)| child_index)
                .collect::<Vec<_>>();
            next_child_indices.sort_unstable();

            for child_index in next_child_indices {
                let mut child_path = path.clone();
                child_path.push(child_index);
                queue.push_back(child_path);
            }
        }
    }

    candidates
}

pub(super) fn shared_fingerprint_for_samples(
    samples: &[PathNodeSample],
    min_shared_pages: usize,
) -> Option<String> {
    let mut fingerprint_counts: HashMap<String, usize> = HashMap::new();
    for sample in samples {
        *fingerprint_counts
            .entry(sample.fingerprint.clone())
            .or_insert(0) += 1;
    }
    fingerprint_counts
        .into_iter()
        .filter(|(_, count)| *count >= min_shared_pages)
        .max_by_key(|(_, count)| *count)
        .map(|(fp, _)| fp)
}

pub(super) fn should_skip_element(element: &ElementRef) -> bool {
    matches!(
        element.value().name(),
        "script" | "style" | "template" | "noscript"
    )
}

pub(super) fn is_boilerplate_text_candidate(text: &str, patterns: &[String]) -> bool {
    if text.len() < MIN_SNIPPET_TEXT_LENGTH || text.len() > MAX_SNIPPET_TEXT_LENGTH {
        return false;
    }
    let text_lower = text.to_ascii_lowercase();
    patterns
        .iter()
        .any(|pattern| text_lower.contains(&pattern.to_ascii_lowercase()))
}

pub(super) fn snippet_contains_selected_selector(
    snippet: &str,
    selected_selectors: &HashSet<String>,
) -> bool {
    let doc = Html::parse_fragment(snippet);
    doc.select(&SELECTOR_ALL_ELEMENTS)
        .any(|el| !selector_candidates(&el).is_disjoint(selected_selectors))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learn::types::PathNodeSample;

    #[test]
    fn snippet_candidate_helpers_cover_edge_paths() {
        assert!(!is_boilerplate_text_candidate(
            "short",
            &["short".to_string()]
        ));
        assert!(!is_boilerplate_text_candidate(
            &"x".repeat(241),
            &["x".to_string()]
        ));
        assert!(is_boilerplate_text_candidate(
            "This privacy footer boilerplate text is long enough to qualify",
            &["privacy".to_string()]
        ));

        let html = vec![
            "<html><body><script>bad</script><div>Privacy footer boilerplate text long enough</div></body></html>".to_string(),
            "<html><body><style>bad</style><div>Privacy footer boilerplate text long enough</div></body></html>".to_string(),
        ];
        let candidates =
            collect_breadth_first_snippet_candidates(&html, 2, &["privacy".to_string()]);
        assert!(!candidates.is_empty());

        let sparse = vec![
            "<html><body><main><p>Only one child path is shared</p></main></body></html>"
                .to_string(),
            "<html><body></body></html>".to_string(),
        ];
        assert!(
            collect_breadth_first_snippet_candidates(&sparse, 2, &["privacy".to_string()])
                .is_empty()
        );
        assert!(
            collect_breadth_first_snippet_candidates(&sparse, 3, &["privacy".to_string()])
                .is_empty()
        );

        let unshared = vec![
            "<html><body><main><p>Only page one has this</p></main></body></html>".to_string(),
            "<html><body><main></main></body></html>".to_string(),
        ];
        assert!(
            collect_breadth_first_snippet_candidates(&unshared, 2, &["privacy".to_string()])
                .is_empty()
        );

        let partial_child = vec![
            "<html><body><main><p>Privacy footer boilerplate text long enough</p></main></body></html>"
                .to_string(),
            "<html><body><main><p>Privacy footer boilerplate text long enough</p></main></body></html>"
                .to_string(),
            "<html><body></body></html>".to_string(),
        ];
        assert!(!collect_breadth_first_snippet_candidates(
            &partial_child,
            2,
            &["privacy".to_string()]
        )
        .is_empty());

        let non_boilerplate = vec![
            "<html><body><div class=\"shared\">Shared non matching content that is long enough</div></body></html>".to_string(),
            "<html><body><div class=\"shared\">Different non matching content that is long enough</div></body></html>".to_string(),
        ];
        assert!(collect_breadth_first_snippet_candidates(
            &non_boilerplate,
            2,
            &["privacy".to_string()]
        )
        .is_empty());

        let selector_only = vec![
            "<html><body><aside role=\"contentinfo\">First unique footer text that is long enough</aside></body></html>".to_string(),
            "<html><body><aside role=\"contentinfo\">Second unique footer text that is long enough</aside></body></html>".to_string(),
        ];
        assert!(collect_breadth_first_snippet_candidates(
            &selector_only,
            2,
            &["privacy".to_string()]
        )
        .is_empty());

        assert!(shared_fingerprint_for_samples(&[], 2).is_none());
        assert_eq!(
            shared_fingerprint_for_samples(
                &[
                    PathNodeSample {
                        page_index: 0,
                        text: String::new(),
                        fingerprint: "fp".to_string(),
                        snippet: String::new(),
                        selectors: HashSet::new(),
                    },
                    PathNodeSample {
                        page_index: 1,
                        text: String::new(),
                        fingerprint: "fp".to_string(),
                        snippet: String::new(),
                        selectors: HashSet::new(),
                    },
                ],
                2,
            ),
            Some("fp".to_string())
        );

        assert!(snippet_contains_selected_selector(
            r#"<div class="shared">text</div>"#,
            &HashSet::from([".shared".to_string()])
        ));
        assert!(!snippet_contains_selected_selector(
            "<p>text</p>",
            &HashSet::from([".x".to_string()])
        ));
    }
}
