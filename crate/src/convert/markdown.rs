use html2md as external_html2md;
use regex::Regex;
use std::sync::LazyLock;

static TRAILING_SPACE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[ \t]+$").expect("BUG: invalid TRAILING_SPACE_REGEX"));
static MULTIPLE_NEWLINES_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\n{3,}").expect("BUG: invalid MULTIPLE_NEWLINES_REGEX"));

/// Convert cleaned HTML to Markdown, then normalize whitespace.
///
/// 1. Remove `\r` (Windows line endings).
/// 2. Strip trailing spaces/tabs from each line.
/// 3. Collapse 3+ consecutive newlines to 2.
/// 4. Trim leading/trailing whitespace.
pub fn html_to_markdown(html: &str) -> String {
    let markdown = external_html2md::parse_html(html);
    let without_cr = markdown.replace('\r', "");
    let without_trailing = TRAILING_SPACE_REGEX.replace_all(&without_cr, "");
    let normalized = MULTIPLE_NEWLINES_REGEX.replace_all(&without_trailing, "\n\n");
    normalized.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_to_markdown_converts_heading() {
        let result = html_to_markdown("<h1>Hello</h1>");
        assert!(result.contains("Hello"));
    }

    #[test]
    fn html_to_markdown_strips_trailing_spaces() {
        let result = html_to_markdown("<p>text  </p>");
        for line in result.lines() {
            assert_eq!(
                line,
                line.trim_end(),
                "line has trailing whitespace: {line:?}"
            );
        }
    }

    #[test]
    fn html_to_markdown_collapses_excessive_newlines() {
        let html = "<p>a</p><p>b</p><p>c</p>";
        let result = html_to_markdown(html);
        assert!(!result.contains("\n\n\n"));
    }

    #[test]
    fn html_to_markdown_trims_output() {
        let result = html_to_markdown("   <p>text</p>   ");
        assert_eq!(result, result.trim());
    }
}
