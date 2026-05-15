use boilerstrip::{learn, LearnOptions, Removals};

fn fixtures_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("fixtures")
}

fn load_pages(dir: &str, count: usize) -> Vec<String> {
    let root = fixtures_root();
    (1..=count)
        .map(|i| {
            std::fs::read_to_string(root.join(format!("{dir}/page{i}.html")))
                .unwrap_or_else(|e| panic!("failed to read {dir}/page{i}.html: {e}"))
        })
        .collect()
}

fn load_expected_removals(dir: &str) -> Removals {
    let root = fixtures_root();
    let json = std::fs::read_to_string(root.join(format!("{dir}/expected.json")))
        .expect("missing expected.json");
    serde_json::from_str(&json).expect("invalid expected.json")
}

#[test]
fn site_a_detects_nav_and_footer_selectors() {
    let pages = load_pages("learn/site_a", 3);
    let removals = learn(&pages, &LearnOptions::default()).unwrap();
    let expected = load_expected_removals("learn/site_a");

    assert_eq!(
        removals.css_selectors_to_remove, expected.css_selectors_to_remove,
        "CSS selectors mismatch"
    );
    assert_eq!(
        removals.html_to_remove, expected.html_to_remove,
        "HTML snippets mismatch"
    );
}

#[test]
fn site_a_removals_clean_a_new_page() {
    let pages = load_pages("learn/site_a", 3);
    let removals = learn(&pages, &LearnOptions::default()).unwrap();

    // Apply removals to page1 and confirm nav + footer are gone
    let html = std::fs::read_to_string(fixtures_root().join("learn/site_a/page1.html")).unwrap();
    let cleaned = boilerstrip::apply_removals(&html, &removals);
    assert!(!cleaned.contains("site-nav"), "nav should be removed");
    assert!(!cleaned.contains("site-footer"), "footer should be removed");
    assert!(cleaned.contains("Pancakes"), "content should remain");
}

#[test]
fn site_b_detects_header_and_aside_snippet() {
    let pages = load_pages("learn/site_b", 2);
    let removals = learn(&pages, &LearnOptions::default()).unwrap();
    let expected = load_expected_removals("learn/site_b");

    assert_eq!(
        removals.css_selectors_to_remove, expected.css_selectors_to_remove,
        "CSS selectors mismatch"
    );
    assert_eq!(
        removals.html_to_remove.len(),
        expected.html_to_remove.len(),
        "HTML snippet count mismatch"
    );
}

#[test]
fn learn_too_few_pages_returns_error() {
    let err = learn(&["<html></html>".to_string()], &LearnOptions::default()).unwrap_err();
    assert!(err.to_string().contains("1"));
}
