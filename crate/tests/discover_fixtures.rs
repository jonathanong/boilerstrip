fn fixtures_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("fixtures")
}

#[test]
fn discover_site_a_removals() {
    let root = fixtures_root();
    let pages: Vec<String> = (1..=3)
        .map(|i| std::fs::read_to_string(root.join(format!("learn/site_a/page{i}.html"))).unwrap())
        .collect();
    let removals = boilerstrip::learn(&pages, &boilerstrip::LearnOptions::default()).unwrap();
    println!("{}", serde_json::to_string_pretty(&removals).unwrap());
}

#[test]
fn discover_site_b_removals() {
    let root = fixtures_root();
    let pages: Vec<String> = (1..=2)
        .map(|i| std::fs::read_to_string(root.join(format!("learn/site_b/page{i}.html"))).unwrap())
        .collect();
    let removals = boilerstrip::learn(&pages, &boilerstrip::LearnOptions::default()).unwrap();
    println!("{}", serde_json::to_string_pretty(&removals).unwrap());
}

#[test]
fn discover_convert_basic_article() {
    let root = fixtures_root();
    let html = std::fs::read_to_string(root.join("convert/basic_article.html")).unwrap();
    let result = boilerstrip::convert(&html, &boilerstrip::ConvertOptions::default()).unwrap();
    println!("--- TITLE ---\n{:?}", result.title);
    println!("--- CONTENT ---\n{}", result.content);
}

#[test]
fn discover_convert_with_meta() {
    let root = fixtures_root();
    let html = std::fs::read_to_string(root.join("convert/with_meta.html")).unwrap();
    let result = boilerstrip::convert(&html, &boilerstrip::ConvertOptions::default()).unwrap();
    println!("--- TITLE ---\n{:?}", result.title);
    println!("--- LANG ---\n{:?}", result.lang);
    println!("--- CANONICAL ---\n{:?}", result.canonical_url);
    println!(
        "--- META ---\n{}",
        serde_json::to_string_pretty(&result.meta).unwrap()
    );
    println!("--- CONTENT ---\n{}", result.content);
}
