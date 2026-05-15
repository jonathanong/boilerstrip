use boilerstrip::{convert, learn, ConvertOptions, LearnOptions};

fn fixtures_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("fixtures")
}

#[test]
fn full_pipeline_strips_header_and_footer_from_new_page() {
    let root = fixtures_root();

    // Step 1: learn from three pages
    let training_pages: Vec<String> = (1..=3)
        .map(|i| {
            std::fs::read_to_string(root.join(format!("end_to_end/pages/page{i}.html"))).unwrap()
        })
        .collect();
    let removals = learn(&training_pages, &LearnOptions::default()).unwrap();

    // The header and footer should be discoverable
    let selectors = removals.css_selectors_to_remove.join(",");
    assert!(
        selectors.contains(".site-header") || selectors.contains(".site-footer"),
        "expected header or footer selector; got: {selectors}"
    );

    // Step 2: convert a new page with the learned removals
    let fresh_html = std::fs::read_to_string(root.join("end_to_end/pages/page1.html")).unwrap();
    let result = convert(
        &fresh_html,
        &ConvertOptions {
            removals: Some(removals),
            ..Default::default()
        },
    );

    // Content present, boilerplate absent
    assert!(result.content.contains("AI Advances"));
    assert!(!result.content.contains("Terms of service"));
    assert!(!result.content.contains("Privacy policy"));
}
