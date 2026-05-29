use scraper::{Html, Selector};
use serde_json::{Map, Value};
use std::sync::LazyLock;

const DEFAULT_ALTERNATE_LINK_TYPE: &str = "unspecified";

/// Link rel values that are resource hints or stylesheets, not page metadata.
const EXCLUDED_REL_VALUES: &[&str] = &[
    "dns-prefetch",
    "preconnect",
    "prefetch",
    "prerender",
    "preload",
    "modulepreload",
    "stylesheet",
];

static SELECTOR_TITLE: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("title").expect("BUG: invalid 'title' selector"));
static SELECTOR_META: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("meta[name], meta[property]").expect("BUG: invalid meta selector")
});
static SELECTOR_LINK_REL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("link[rel]").expect("BUG: invalid 'link[rel]' selector"));
static SELECTOR_CANONICAL: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("link[rel=\"canonical\"]").expect("BUG: invalid canonical selector")
});
static SELECTOR_HTML: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("html").expect("BUG: invalid 'html' selector"));

pub fn parse_html(html: &str) -> Html {
    Html::parse_document(html)
}

/// Extract the text of the first `<title>` element; returns `None` if absent or blank.
pub fn extract_title(document: &Html) -> Option<String> {
    document
        .select(&SELECTOR_TITLE)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
}

/// URL-valued meta properties/names whose content must pass `is_safe_url`.
const URL_META_KEYS: &[&str] = &[
    "og:url",
    "og:image",
    "og:image:url",
    "og:image:secure_url",
    "og:video",
    "og:video:url",
    "og:video:secure_url",
    "og:audio",
    "og:audio:url",
    "og:audio:secure_url",
    "twitter:url",
    "twitter:image",
    "twitter:image:src",
    "twitter:player",
];

/// Collect `<meta name/property>` into a JSON map. First occurrence wins.
/// URL-typed meta values (e.g. `og:url`) are filtered through `is_safe_url`.
pub fn extract_meta_tags(document: &Html) -> Map<String, Value> {
    let mut meta = Map::new();
    for element in document.select(&SELECTOR_META) {
        let key = element
            .value()
            .attr("name")
            .or_else(|| element.value().attr("property"));
        let content = element.value().attr("content");
        if let (Some(k), Some(c)) = (key, content) {
            let is_url_key = URL_META_KEYS.iter().any(|uk| uk.eq_ignore_ascii_case(k));
            if is_url_key && !is_safe_url(c) {
                continue;
            }
            meta.entry(k.to_string())
                .or_insert_with(|| Value::String(c.to_string()));
        }
    }
    meta
}

/// Extract `<link rel>` tags into a map of rel-token → metadata.
///
/// Splits each `rel` into tokens, filters excluded/custom tokens, keeps the
/// first href for ordinary rels, and stores every `alternate` href in type
/// buckets.
pub fn extract_link_tags(
    document: &Html,
    link_rel_tokens_to_remove: Option<&[String]>,
) -> Map<String, Value> {
    let mut links = Map::new();
    let custom_remove: Vec<String> = link_rel_tokens_to_remove
        .map(|v| v.iter().map(|t| t.to_lowercase()).collect())
        .unwrap_or_default();

    for (element, rel) in document
        .select(&SELECTOR_LINK_REL)
        .filter_map(|el| el.value().attr("rel").map(|rel| (el, rel)))
    {
        let Some(href) = element.value().attr("href") else {
            continue;
        };
        let alternate_link_type = normalize_link_type(element.value().attr("type"));

        if !is_safe_url(href) {
            continue;
        }

        for token in rel.split_whitespace() {
            let normalized_token = token.to_lowercase();
            if EXCLUDED_REL_VALUES.contains(&normalized_token.as_str())
                || custom_remove.contains(&normalized_token)
            {
                continue;
            }
            if normalized_token == "alternate" {
                insert_alternate_link(&mut links, alternate_link_type.as_deref(), href);
                continue;
            }
            links
                .entry(normalized_token)
                .or_insert_with(|| Value::String(href.to_string()));
        }
    }
    links
}

fn normalize_link_type(link_type: Option<&str>) -> Option<String> {
    link_type
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
}

fn insert_alternate_link(links: &mut Map<String, Value>, link_type: Option<&str>, href: &str) {
    let alternate_entry = links
        .entry("alternate".to_string())
        .or_insert_with(|| Value::Object(Map::new()));

    let Value::Object(alternate_links) = alternate_entry else {
        return;
    };

    let bucket_key = link_type.unwrap_or(DEFAULT_ALTERNATE_LINK_TYPE).to_string();
    let bucket_entry = alternate_links
        .entry(bucket_key)
        .or_insert_with(|| Value::Array(Vec::new()));

    let Value::Array(hrefs) = bucket_entry else {
        return;
    };

    if !hrefs.iter().any(|v| v.as_str() == Some(href)) {
        hrefs.push(Value::String(href.to_string()));
    }
}

/// Return the href of `<link rel="canonical">` if it has a safe scheme.
pub fn extract_canonical_url(document: &Html) -> Option<String> {
    document
        .select(&SELECTOR_CANONICAL)
        .next()
        .and_then(|el| el.value().attr("href"))
        .filter(|url| is_safe_url(url))
        .map(std::string::ToString::to_string)
}

/// Extract the `lang` attribute from `<html>`, capped at 35 chars.
pub fn extract_lang(document: &Html) -> Option<String> {
    document
        .select(&SELECTOR_HTML)
        .next()
        .and_then(|el| el.value().attr("lang"))
        .map(|s| s.trim().chars().take(35).collect::<String>())
        .filter(|s| !s.is_empty())
}

pub(crate) fn is_safe_url(url: &str) -> bool {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return false;
    }
    let scheme_end = trimmed
        .char_indices()
        .find_map(|(i, ch)| match ch {
            ':' => Some(Some(i)),
            '/' | '?' | '#' => Some(None),
            _ => None,
        })
        .flatten();

    match scheme_end {
        None => true,
        Some(end) => {
            let scheme = trimmed[..end].to_ascii_lowercase();
            scheme == "http" || scheme == "https"
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value};

    use super::*;

    #[test]
    fn extract_title_returns_none_for_blank() {
        let doc = Html::parse_document("<html><head><title>   </title></head></html>");
        assert_eq!(extract_title(&doc), None);
    }

    #[test]
    fn extract_title_returns_trimmed_text() {
        let doc = Html::parse_document("<html><head><title>  My Page  </title></head></html>");
        assert_eq!(extract_title(&doc), Some("My Page".to_string()));
    }

    #[test]
    fn extract_meta_tags_first_occurrence_wins() {
        let doc = Html::parse_document(
            r#"<html><head>
              <meta name="description" content="first">
              <meta name="description" content="second">
            </head></html>"#,
        );
        let meta = extract_meta_tags(&doc);
        assert_eq!(
            meta.get("description"),
            Some(&Value::String("first".to_string()))
        );
    }

    #[test]
    fn extract_meta_tags_supports_property_attribute() {
        let doc = Html::parse_document(
            r#"<html><head><meta property="og:title" content="OG Title"></head></html>"#,
        );
        let meta = extract_meta_tags(&doc);
        assert!(meta.contains_key("og:title"));
    }

    #[test]
    fn alternate_link_insert_handles_unexpected_shapes() {
        let mut links = Map::new();
        links.insert("alternate".to_string(), Value::String("bad".to_string()));
        insert_alternate_link(&mut links, Some("rss"), "/feed");
        assert_eq!(
            links.get("alternate"),
            Some(&Value::String("bad".to_string()))
        );

        let mut links = Map::new();
        let mut alt_map = Map::new();
        alt_map.insert("rss".to_string(), Value::String("bad".to_string()));
        links.insert("alternate".to_string(), Value::Object(alt_map));
        insert_alternate_link(&mut links, Some("rss"), "/feed");
        assert!(links
            .get("alternate")
            .and_then(|v| v.as_object())
            .and_then(|m| m.get("rss"))
            .and_then(|v| v.as_str())
            .is_some());

        let mut links = Map::new();
        insert_alternate_link(&mut links, None, "/feed");
        insert_alternate_link(&mut links, None, "/feed");
        assert_eq!(
            links
                .get("alternate")
                .and_then(|v| v.as_object())
                .and_then(|m| m.get(DEFAULT_ALTERNATE_LINK_TYPE))
                .and_then(|v| v.as_array())
                .map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn safe_url_rejects_empty_and_dangerous_schemes() {
        assert!(!is_safe_url(" "));
        assert!(!is_safe_url("data:text/plain,bad"));
        assert!(is_safe_url("/relative:path"));
        assert!(is_safe_url("https://example.com"));
        assert!(!is_safe_url("javascript:alert(1)"));
    }

    #[test]
    fn extract_lang_caps_at_35_chars() {
        let long_lang = "a".repeat(40);
        let doc = Html::parse_document(&format!("<html lang=\"{long_lang}\"></html>"));
        let lang = extract_lang(&doc).expect("lang should be extracted");
        assert_eq!(lang.len(), 35);
    }

    #[test]
    fn extract_lang_returns_none_for_blank() {
        let doc = Html::parse_document("<html lang=\"  \"></html>");
        assert_eq!(extract_lang(&doc), None);
    }

    #[test]
    fn extract_link_tags_excludes_stylesheet_and_preload() {
        let doc = Html::parse_document(
            r#"<html><head>
              <link rel="stylesheet" href="/style.css">
              <link rel="canonical" href="https://example.com">
              <link rel="preload" href="/font.woff2">
            </head></html>"#,
        );
        let links = extract_link_tags(&doc, None);
        assert!(!links.contains_key("stylesheet"));
        assert!(!links.contains_key("preload"));
        assert!(links.contains_key("canonical"));
    }

    #[test]
    fn extract_link_tags_respects_custom_removal_tokens() {
        let doc = Html::parse_document(
            r#"<html><head><link rel="canonical" href="https://example.com"></head></html>"#,
        );
        let links = extract_link_tags(&doc, Some(&["canonical".to_string()]));
        assert!(!links.contains_key("canonical"));
    }

    #[test]
    fn extract_canonical_url_rejects_dangerous_scheme() {
        let doc = Html::parse_document(
            r#"<html><head><link rel="canonical" href="javascript:void(0)"></head></html>"#,
        );
        assert_eq!(extract_canonical_url(&doc), None);
    }

    #[test]
    fn extract_link_tags_skips_link_without_href() {
        let doc = Html::parse_document(r#"<html><head><link rel="canonical"></head></html>"#);
        let links = extract_link_tags(&doc, None);
        assert!(!links.contains_key("canonical"));
    }

    #[test]
    fn extract_meta_tags_skips_meta_without_content() {
        // <meta name="..."> without content= should be silently ignored.
        let doc = Html::parse_document(
            r#"<html><head><meta name="viewport"><meta name="description" content="ok"></head></html>"#,
        );
        let meta = extract_meta_tags(&doc);
        // "viewport" has no content, so it must be absent
        assert!(!meta.contains_key("viewport"));
        assert!(meta.contains_key("description"));
    }

    #[test]
    fn extract_link_tags_handles_alternate_with_type() {
        let doc = Html::parse_document(
            r#"<html><head>
              <link rel="alternate" type="application/rss+xml" href="/feed.xml">
              <link rel="alternate" type="" href="/empty-type">
              <link rel="alternate" href="/no-type">
            </head></html>"#,
        );
        let links = extract_link_tags(&doc, None);
        let alternate = links
            .get("alternate")
            .and_then(|v| v.as_object())
            .expect("alternate key should exist");
        // RSS type entry
        assert!(alternate.contains_key("application/rss+xml"));
        // Empty type is filtered by normalize_link_type → stored under DEFAULT_ALTERNATE_LINK_TYPE
        assert!(alternate.contains_key(DEFAULT_ALTERNATE_LINK_TYPE));
    }

    #[test]
    fn alternate_link_insert_handles_unexpected_nested_bucket_shape() {
        let mut links = Map::new();
        let mut alternate_map = Map::new();

        // Setup the bucket itself as a non-array value
        alternate_map.insert("rss".to_string(), Value::Bool(true));
        links.insert("alternate".to_string(), Value::Object(alternate_map));

        insert_alternate_link(&mut links, Some("rss"), "/feed3");

        let rss_val = links
            .get("alternate")
            .and_then(|v| v.as_object())
            .and_then(|m| m.get("rss"))
            .expect("rss key should exist");

        // The else branch should be hit and the value should remain unchanged as Bool.
        assert_eq!(rss_val.as_bool(), Some(true));
    }

    #[test]
    fn alternate_link_insert_handles_unexpected_shapes_structured() {
        // When the alternate entry is already a non-object value, skip gracefully
        let mut links = Map::new();
        links.insert("alternate".to_string(), Value::String("bad".to_string()));
        insert_alternate_link(&mut links, Some("rss"), "/feed");
        // Unchanged because the existing "alternate" value is not an Object
        assert_eq!(
            links.get("alternate"),
            Some(&Value::String("bad".to_string()))
        );

        // When the bucket key already exists but is not an Array, skip gracefully
        let mut links = Map::new();
        let mut alt_map = Map::new();
        alt_map.insert("rss".to_string(), Value::String("not-an-array".to_string()));
        links.insert("alternate".to_string(), Value::Object(alt_map));
        insert_alternate_link(&mut links, Some("rss"), "/feed2");
        let rss_val = links
            .get("alternate")
            .and_then(|v| v.as_object())
            .and_then(|m| m.get("rss"))
            .expect("rss key should exist");
        // Still the old non-array value; insert was skipped
        assert!(rss_val.as_str().is_some());

        // Deduplication: inserting the same href twice keeps only one entry
        let mut links = Map::new();
        insert_alternate_link(&mut links, None, "/feed");
        insert_alternate_link(&mut links, None, "/feed");
        assert_eq!(
            links
                .get("alternate")
                .and_then(|v| v.as_object())
                .and_then(|m| m.get(DEFAULT_ALTERNATE_LINK_TYPE))
                .and_then(|v| v.as_array())
                .map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn extract_meta_tags_filters_dangerous_url_meta() {
        let doc = Html::parse_document(
            r#"<html><head>
              <meta property="og:url" content="javascript:alert(1)">
              <meta property="og:title" content="Safe title">
              <meta property="og:image" content="data:image/png,bad">
              <meta property="og:image" content="https://example.com/img.png">
            </head></html>"#,
        );
        let meta = extract_meta_tags(&doc);
        assert!(
            !meta.contains_key("og:url"),
            "dangerous og:url should be filtered"
        );
        assert!(
            meta.contains_key("og:title"),
            "non-URL meta should pass through"
        );
        assert_eq!(
            meta.get("og:image").and_then(|v| v.as_str()),
            Some("https://example.com/img.png"),
            "dangerous data: image filtered, safe https: image accepted"
        );
    }

    #[test]
    fn extract_link_tags_filters_dangerous_href() {
        let doc = Html::parse_document(
            r#"<html><head>
              <link rel="canonical" href="javascript:void(0)">
              <link rel="alternate" href="javascript:void(0)">
            </head></html>"#,
        );
        let links = extract_link_tags(&doc, None);
        assert!(
            !links.contains_key("canonical"),
            "dangerous canonical href filtered"
        );
        assert!(
            !links.contains_key("alternate"),
            "dangerous alternate href filtered"
        );
    }

    #[test]
    fn parse_html_alias_works() {
        let doc = parse_html("<html><head><title>T</title></head></html>");
        assert_eq!(extract_title(&doc), Some("T".to_string()));
    }

    #[test]
    fn base_href_does_not_affect_canonical_extraction() {
        // <base href> is not resolved — canonical is read verbatim.
        // This is the documented behaviour; callers must resolve if needed.
        let doc = Html::parse_document(
            r#"<html><head>
              <base href="https://cdn.example.com/">
              <link rel="canonical" href="/page">
            </head></html>"#,
        );
        assert_eq!(
            extract_canonical_url(&doc),
            Some("/page".to_string()),
            "canonical is the verbatim href, not base-resolved"
        );
    }
}
