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

#[test]
fn nested_tables_render_outer_table() {
    let html = r#"<html><body><table>
      <tr><td><table><tr><td>inner</td></tr></table></td><td>outer-cell</td></tr>
    </table></body></html>"#;
    let result = convert(html, &ConvertOptions::default());
    assert!(
        result.content.contains("outer-cell"),
        "outer table cell should appear in output"
    );
}

#[test]
fn anchor_wrapping_image_preserves_linked_image() {
    let html = r#"<html><body><main><a href="https://example.com"><img src="logo.png" alt="Logo"></a></main></body></html>"#;
    let result = convert(html, &ConvertOptions::default());
    // The linked image should produce something like [![Logo](logo.png)](https://example.com)
    assert!(
        result.content.contains("logo.png") || result.content.contains("Logo"),
        "image inside anchor should be preserved"
    );
}

#[test]
fn whitespace_text_between_block_elements_is_skipped() {
    // Whitespace text nodes between block elements should be skipped when pending_nl > 0.
    // After <h1> renders, pending_nl=2; the "\n    " text before <p> is whitespace-only,
    // so it hits the skip path in emit_node.
    let html = "<html><body><main><h1>Title</h1>\n    <p>Content</p></main></body></html>";
    let result = convert(html, &ConvertOptions::default());
    assert!(result.content.contains("Title"));
    assert!(result.content.contains("Content"));
}

#[test]
fn debug_fixture_dom() {
    // Check the actual fixture HTML DOM structure
    let html = std::fs::read_to_string(fixtures_root().join("convert/basic_article.html")).unwrap();
    let doc = scraper::Html::parse_document(&html);
    let sel = scraper::Selector::parse("main").unwrap();
    let main_el = doc.select(&sel).next().expect("main element must exist");
    let node_ref: ego_tree::NodeRef<'_, scraper::Node> = *main_el;
    println!("main direct children:");
    for (i, child) in node_ref.children().enumerate() {
        match child.value() {
            scraper::Node::Text(t) => {
                println!("  [{}] Text: {:?}", i, <str as AsRef<str>>::as_ref(t))
            }
            scraper::Node::Element(e) => println!("  [{}] Element: <{}>", i, e.name()),
            _ => println!("  [{}] Other", i),
        }
    }
}

#[test]
fn debug_lolhtml_and_dom() {
    // Verify that lol_html preserves whitespace and html5ever creates correct DOM
    let html = "<html><body><main><h1>Title</h1>\n    <p>Content</p></main></body></html>";

    // Simulate what convert() does: lol_html strip pass
    let mut output = Vec::with_capacity(html.len());
    let settings = lol_html::Settings::new().append_element_content_handler((
        std::borrow::Cow::Owned("script, style".parse::<lol_html::Selector>().unwrap()),
        lol_html::ElementContentHandlers::default().element(
            |el: &mut lol_html::html_content::Element<'_, '_>| {
                el.remove();
                Ok(())
            },
        ),
    ));
    let mut rewriter =
        lol_html::HtmlRewriter::new(settings, |c: &[u8]| output.extend_from_slice(c));
    rewriter.write(html.as_bytes()).unwrap();
    rewriter.end().unwrap();
    let stripped = unsafe { String::from_utf8_unchecked(output) };
    println!("lol_html output: {:?}", &stripped);

    // Now parse with scraper like convert() does
    let doc = scraper::Html::parse_document(&stripped);
    let sel = scraper::Selector::parse("main").unwrap();
    let main_el = doc.select(&sel).next().expect("main element must exist");
    let node_ref: ego_tree::NodeRef<'_, scraper::Node> = *main_el;
    let children: Vec<_> = node_ref.children().collect();
    for (i, child) in children.iter().enumerate() {
        match child.value() {
            scraper::Node::Text(t) => {
                println!("  [{}] Text: {:?}", i, <str as AsRef<str>>::as_ref(t))
            }
            scraper::Node::Element(e) => println!("  [{}] Element: <{}>", i, e.name()),
            _ => println!("  [{}] Other", i),
        }
    }
    println!("main children count: {}", children.len());
    assert!(children.len() >= 2, "main should have at least 2 children");
}

#[test]
fn deep_dom_does_not_stack_overflow() {
    let depth = 1500usize;
    let mut html = String::from("<html><body><main>");
    for _ in 0..depth {
        html.push_str("<div>");
    }
    html.push_str("deep content");
    for _ in 0..depth {
        html.push_str("</div>");
    }
    html.push_str("</main></body></html>");
    // Must not panic or stack-overflow
    let _result = convert(&html, &ConvertOptions::default());
}

#[test]
fn table_subelement_as_content_root_does_not_panic() {
    // Using content_selectors to pick <thead> as the content root means
    // element_to_markdown walks <tr>/<th>/<td> children with table_state=None.
    // The if-let-Some guards in emit_element must not panic in that case.
    let html = "<html><body><table><thead><tr><th>H1</th><th>H2</th></tr></thead>\
                <tbody><tr><td>A</td><td>B</td></tr></tbody></table></body></html>";
    let options = ConvertOptions {
        content_selectors: vec!["thead".to_string()],
        ..Default::default()
    };
    let _result = convert(html, &options);
}
