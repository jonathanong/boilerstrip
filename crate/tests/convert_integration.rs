use boilerstrip::{convert, ConvertOptions};

fn fixtures_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("fixtures")
}

#[test]
fn basic_article_matches_expected_markdown() {
    let root = fixtures_root();
    let html = std::fs::read_to_string(root.join("convert/basic_article.html")).unwrap();
    let expected_md =
        std::fs::read_to_string(root.join("convert/basic_article.expected.md")).unwrap();

    let result = convert(&html, &ConvertOptions::default());
    assert_eq!(result.title, Some("Getting Started".to_string()));
    assert_eq!(result.content.trim(), expected_md.trim());
}

#[test]
fn with_meta_matches_expected_json() {
    let root = fixtures_root();
    let html = std::fs::read_to_string(root.join("convert/with_meta.html")).unwrap();
    let expected: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("convert/with_meta.expected.json")).unwrap(),
    )
    .unwrap();

    let result = convert(&html, &ConvertOptions::default());
    assert_eq!(result.title, expected["title"].as_str().map(str::to_string));
    assert_eq!(result.lang, expected["lang"].as_str().map(str::to_string));
    assert_eq!(
        result.canonical_url,
        expected["canonical_url"].as_str().map(str::to_string)
    );
    assert_eq!(
        result.meta.get("description").and_then(|v| v.as_str()),
        expected["meta"]["description"].as_str()
    );
    assert_eq!(
        result.content.trim(),
        expected["content"].as_str().unwrap().trim()
    );
}

#[test]
fn tables_and_lists_conversion_succeeds() {
    let root = fixtures_root();
    let html = std::fs::read_to_string(root.join("convert/tables_and_lists.html")).unwrap();
    let result = convert(&html, &ConvertOptions::default());
    assert!(result.content.contains("Comparison"));
    assert!(result.content.contains("Install the package") || result.content.contains("1."));
}
