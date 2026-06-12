use scraper::Html;
use boilerstrip::convert::markdown::element_to_markdown;

#[test]
fn test_thead_tbody_outside_table_no_panic() {
    let htmls = vec![
        r#"<math><thead><tr><td>Hello</td></tr></thead></math>"#,
        r#"<div><tbody><tr><td>Hello</td></tr></tbody></div>"#,
        r#"<article><tfoot><tr><td>Hello</td></tr></tfoot></article>"#,
    ];

    for html in htmls {
        let fragment = Html::parse_fragment(html);
        let md = element_to_markdown(fragment.root_element());
        assert_eq!(md.trim(), "Hello", "Should safely extract text without panicking");
    }
}
