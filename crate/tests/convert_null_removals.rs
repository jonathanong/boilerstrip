use boilerstrip::{convert, ConvertOptions};

#[test]
fn convert_handles_none_removals() {
    let html = "<html><body><p>Test</p></body></html>";
    let options = ConvertOptions {
        removals: None,
        ..Default::default()
    };
    let result = convert(html, &options);
    assert!(result.content.contains("Test"));
}
