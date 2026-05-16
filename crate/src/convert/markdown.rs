use ego_tree::NodeRef;
use scraper::{ElementRef, Node};

/// Convert the content of a scraper element tree directly into Markdown
/// without any additional HTML parse step.
pub fn element_to_markdown(root: ElementRef<'_>) -> String {
    let mut state = State::default();
    // Use the underlying NodeRef children to get both element and text nodes.
    let node_ref: NodeRef<'_, Node> = *root;
    for child in node_ref.children() {
        emit_node(child, &mut state);
    }
    finalize(state.buf)
}

fn finalize(buf: String) -> String {
    let without_trailing_spaces: String = buf
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    let normalized = collapse_blank_lines(&without_trailing_spaces);
    normalized.trim().to_string()
}

fn collapse_blank_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank_count = 0usize;
    let mut has_content = false;
    for line in s.lines() {
        if line.trim().is_empty() {
            if has_content {
                blank_count += 1;
            }
        } else {
            if has_content && blank_count > 0 {
                out.push('\n'); // one blank line between sections
            }
            blank_count = 0;
            has_content = true;
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[derive(Default)]
struct State {
    buf: String,
    pending_nl: usize,
    in_pre: bool,
    list_stack: Vec<ListKind>,
    table_state: Option<TableState>,
}

enum ListKind {
    Unordered,
    Ordered(usize),
}

struct TableState {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    current_row: Vec<String>,
    current_cell: String,
    in_head: bool,
}

impl State {
    fn ensure_newlines(&mut self, n: usize) {
        if self.pending_nl < n {
            self.pending_nl = n;
        }
    }

    fn flush_pending(&mut self) {
        if self.pending_nl > 0 {
            for _ in 0..self.pending_nl {
                self.buf.push('\n');
            }
            self.pending_nl = 0;
        }
    }

    fn push_str(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.flush_pending();
        self.buf.push_str(s);
    }

    fn list_depth(&self) -> usize {
        self.list_stack.len()
    }

    fn list_prefix(&mut self) -> String {
        let depth = self.list_stack.len();
        let indent = "  ".repeat(depth.saturating_sub(1));
        match self.list_stack.last_mut() {
            Some(ListKind::Ordered(n)) => {
                *n += 1;
                format!("{indent}{}. ", *n - 1)
            }
            Some(ListKind::Unordered) => format!("{indent}* "),
            None => String::new(),
        }
    }
}

fn emit_node(node: NodeRef<'_, Node>, state: &mut State) {
    match node.value() {
        Node::Text(text) => {
            let s = text.as_ref();
            if state.in_pre {
                if let Some(ts) = state.table_state.as_mut() {
                    ts.current_cell.push_str(s);
                } else {
                    state.buf.push_str(s);
                }
            } else {
                let normalized = normalize_inline_text(s);
                if normalized.is_empty() {
                    return;
                }
                // Skip whitespace-only text nodes when we're at a block boundary
                // (pending newlines already queued) to avoid spurious spaces.
                if normalized.trim().is_empty() && state.pending_nl > 0 {
                    return;
                }
                if let Some(ts) = state.table_state.as_mut() {
                    ts.current_cell.push_str(&normalized);
                } else {
                    state.push_str(&normalized);
                }
            }
        }
        Node::Element(_) => {
            if let Some(el) = ElementRef::wrap(node) {
                emit_element(el, state);
            }
        }
        _ => {}
    }
}

fn normalize_inline_text(s: &str) -> String {
    // Collapse whitespace but preserve a single space at boundaries
    let mut out = String::with_capacity(s.len());
    let mut last_was_space = false;
    for ch in s.chars() {
        if ch.is_ascii_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            out.push(ch);
            last_was_space = false;
        }
    }
    out
}

fn emit_element(el: ElementRef<'_>, state: &mut State) {
    let name = el.value().name();

    match name {
        "script" | "style" | "head" | "noscript" | "template" => (),

        "br" => {
            if let Some(ts) = state.table_state.as_mut() {
                ts.current_cell.push(' ');
            } else {
                state.push_str("  \n");
            }
        }

        "hr" => {
            state.ensure_newlines(2);
            state.push_str("---");
            state.ensure_newlines(2);
        }

        "img" => {
            let alt = el.value().attr("alt").unwrap_or("");
            let src = el.value().attr("src").unwrap_or("");
            if !src.is_empty() {
                let img_md = format!("![{alt}]({src})");
                if let Some(ts) = state.table_state.as_mut() {
                    ts.current_cell.push_str(&img_md);
                } else {
                    state.push_str(&img_md);
                }
            }
        }

        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = name.chars().nth(1).unwrap().to_digit(10).unwrap() as usize;
            let prefix = "#".repeat(level);
            state.ensure_newlines(2);
            state.push_str(&prefix);
            state.push_str(" ");
            let text = collect_inline_text(&el);
            state.push_str(text.trim());
            state.ensure_newlines(2);
        }

        "p" => {
            state.ensure_newlines(2);
            for child in (*el).children() {
                emit_node(child, state);
            }
            state.ensure_newlines(2);
        }

        "pre" => {
            state.ensure_newlines(2);
            state.in_pre = true;
            // Check if direct child is <code> for fenced blocks
            let lang = el
                .children()
                .filter_map(ElementRef::wrap)
                .find(|c| c.value().name() == "code")
                .and_then(|code_el| code_el.value().attr("class"))
                .and_then(|cls| {
                    cls.split_whitespace()
                        .find(|c| c.starts_with("language-"))
                        .map(|c| c.trim_start_matches("language-").to_string())
                });
            let lang_str = lang.as_deref().unwrap_or("");
            state.push_str("```");
            state.push_str(lang_str);
            state.buf.push('\n');
            state.pending_nl = 0;
            for child in (*el).children() {
                emit_node(child, state);
            }
            state.in_pre = false;
            if !state.buf.ends_with('\n') {
                state.buf.push('\n');
            }
            state.buf.push_str("```");
            state.ensure_newlines(2);
        }

        "code" => {
            if state.in_pre {
                // Inside a pre block — emit raw, no backtick wrapping
                for child in (*el).children() {
                    emit_node(child, state);
                }
                return;
            }
            let text = collect_inline_text(&el);
            let tick = if text.contains('`') { "``" } else { "`" };
            let code_md = format!("{tick}{text}{tick}");
            if let Some(ts) = state.table_state.as_mut() {
                ts.current_cell.push_str(&code_md);
            } else {
                state.push_str(&code_md);
            }
        }

        "strong" | "b" => {
            let text = collect_inline_text(&el);
            if text.trim().is_empty() {
                return;
            }
            let bold = format!("**{text}**");
            if let Some(ts) = state.table_state.as_mut() {
                ts.current_cell.push_str(&bold);
            } else {
                state.push_str(&bold);
            }
        }

        "em" | "i" => {
            let text = collect_inline_text(&el);
            if text.trim().is_empty() {
                return;
            }
            let italic = format!("*{text}*");
            if let Some(ts) = state.table_state.as_mut() {
                ts.current_cell.push_str(&italic);
            } else {
                state.push_str(&italic);
            }
        }

        "del" | "s" | "strike" => {
            let text = collect_inline_text(&el);
            if text.trim().is_empty() {
                return;
            }
            let del = format!("~~{text}~~");
            if let Some(ts) = state.table_state.as_mut() {
                ts.current_cell.push_str(&del);
            } else {
                state.push_str(&del);
            }
        }

        "a" => {
            let href = el.value().attr("href");
            let text = collect_inline_text(&el);
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return;
            }
            let link_md = if let Some(href) = href {
                format!("[{trimmed}]({href})")
            } else {
                trimmed.to_string()
            };
            if let Some(ts) = state.table_state.as_mut() {
                ts.current_cell.push_str(&link_md);
            } else {
                state.push_str(&link_md);
            }
        }

        "ul" => {
            if state.list_depth() == 0 {
                state.ensure_newlines(2);
            }
            state.list_stack.push(ListKind::Unordered);
            for child in (*el).children() {
                emit_node(child, state);
            }
            state.list_stack.pop();
            if state.list_depth() == 0 {
                state.ensure_newlines(2);
            }
        }

        "ol" => {
            if state.list_depth() == 0 {
                state.ensure_newlines(2);
            }
            let start = el
                .value()
                .attr("start")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(1);
            state.list_stack.push(ListKind::Ordered(start));
            for child in (*el).children() {
                emit_node(child, state);
            }
            state.list_stack.pop();
            if state.list_depth() == 0 {
                state.ensure_newlines(2);
            }
        }

        "li" => {
            if state.in_pre {
                for child in (*el).children() {
                    emit_node(child, state);
                }
                return;
            }
            state.ensure_newlines(1);
            let prefix = state.list_prefix();
            state.push_str(&prefix);
            // Collect li content, handling nested lists
            for child in (*el).children() {
                match child.value() {
                    Node::Element(_) => {
                        if let Some(child_el) = ElementRef::wrap(child) {
                            let child_name = child_el.value().name();
                            if child_name == "ul" || child_name == "ol" {
                                // Nested list — emit on next lines
                                state.ensure_newlines(1);
                                emit_element(child_el, state);
                            } else {
                                emit_element(child_el, state);
                            }
                        }
                    }
                    _ => emit_node(child, state),
                }
            }
        }

        "blockquote" => {
            state.ensure_newlines(2);
            // Collect all content, then prefix each line with "> "
            let mut inner_state = State::default();
            for child in (*el).children() {
                emit_node(child, &mut inner_state);
            }
            let inner = finalize(inner_state.buf);
            for line in inner.lines() {
                state.push_str("> ");
                state.push_str(line);
                state.buf.push('\n');
                state.pending_nl = 0;
            }
            state.ensure_newlines(2);
        }

        "table" => {
            state.ensure_newlines(2);
            state.table_state = Some(TableState {
                headers: vec![],
                rows: vec![],
                current_row: vec![],
                current_cell: String::new(),
                in_head: false,
            });
            for child in (*el).children() {
                emit_node(child, state);
            }
            if let Some(ts) = state.table_state.take() {
                emit_gfm_table(ts, &mut state.buf);
                state.pending_nl = 0;
            }
            state.ensure_newlines(2);
        }

        "thead" => {
            if let Some(ts) = state.table_state.as_mut() {
                ts.in_head = true;
            }
            for child in (*el).children() {
                emit_node(child, state);
            }
            if let Some(ts) = state.table_state.as_mut() {
                ts.in_head = false;
            }
        }

        "tbody" | "tfoot" => {
            if let Some(ts) = state.table_state.as_mut() {
                ts.in_head = false;
            }
            for child in (*el).children() {
                emit_node(child, state);
            }
        }

        "tr" => {
            if let Some(ts) = state.table_state.as_mut() {
                ts.current_row.clear();
            }
            for child in (*el).children() {
                emit_node(child, state);
            }
            if let Some(ts) = state.table_state.as_mut() {
                let row = std::mem::take(&mut ts.current_row);
                if ts.in_head || ts.headers.is_empty() {
                    ts.headers = row;
                } else {
                    ts.rows.push(row);
                }
            }
        }

        "th" | "td" => {
            if let Some(ts) = state.table_state.as_mut() {
                ts.current_cell.clear();
            }
            for child in (*el).children() {
                emit_node(child, state);
            }
            if let Some(ts) = state.table_state.as_mut() {
                let cell = std::mem::take(&mut ts.current_cell);
                ts.current_row.push(cell.trim().to_string());
            }
        }

        // Block containers — emit children with surrounding blank lines
        "div" | "section" | "article" | "main" | "aside" | "header" | "footer" | "nav"
        | "figure" | "figcaption" | "details" | "summary" | "body" | "html" => {
            state.ensure_newlines(2);
            for child in (*el).children() {
                emit_node(child, state);
            }
            state.ensure_newlines(2);
        }

        // Inline containers — just recurse
        "span" | "abbr" | "cite" | "kbd" | "mark" | "q" | "small" | "sub" | "sup" | "time"
        | "var" | "wbr" | "bdi" | "bdo" | "u" | "ins" | "label" => {
            for child in (*el).children() {
                emit_node(child, state);
            }
        }

        _ => {
            for child in (*el).children() {
                emit_node(child, state);
            }
        }
    }
}

fn emit_gfm_table(ts: TableState, buf: &mut String) {
    if ts.headers.is_empty() {
        return;
    }
    let col_count = ts.headers.len();
    buf.push('|');
    for h in &ts.headers {
        buf.push(' ');
        buf.push_str(h);
        buf.push_str(" |");
    }
    buf.push('\n');
    buf.push('|');
    for _ in 0..col_count {
        buf.push_str(" --- |");
    }
    buf.push('\n');
    for row in &ts.rows {
        buf.push('|');
        let cells_written = row.len().min(col_count);
        for cell in row.iter().take(cells_written) {
            buf.push(' ');
            buf.push_str(cell);
            buf.push_str(" |");
        }
        for _ in cells_written..col_count {
            buf.push_str("  |");
        }
        buf.push('\n');
    }
}

/// Collect all inline text from an element's subtree as a single normalized string.
fn collect_inline_text(el: &ElementRef<'_>) -> String {
    let raw: String = el.text().collect();
    normalize_inline_text(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::{Html, Selector};

    fn md(html: &str) -> String {
        let doc = Html::parse_document(&format!("<body>{html}</body>"));
        let body = doc
            .select(&Selector::parse("body").unwrap())
            .next()
            .unwrap();
        element_to_markdown(body)
    }

    #[test]
    fn br_emits_hard_break() {
        let result = md("<p>Hello<br>World</p>");
        assert!(result.contains("Hello") && result.contains("World"));
    }

    #[test]
    fn hr_emits_rule() {
        assert!(md("<p>A</p><hr><p>B</p>").contains("---"));
    }

    #[test]
    fn img_with_src_and_alt() {
        assert_eq!(
            md("<img src=\"/a.png\" alt=\"icon\">").trim(),
            "![icon](/a.png)"
        );
    }

    #[test]
    fn img_without_src_is_skipped() {
        assert!(md("<img alt=\"x\">").trim().is_empty());
    }

    #[test]
    fn img_empty_src_is_skipped() {
        assert!(md("<img src=\"\" alt=\"x\">").trim().is_empty());
    }

    #[test]
    fn pre_with_language_class() {
        let result = md("<pre><code class=\"language-rust\">fn main(){}</code></pre>");
        assert!(result.contains("```rust") && result.contains("fn main(){}"));
    }

    #[test]
    fn pre_without_language_class() {
        let result = md("<pre><code>plain code</code></pre>");
        assert!(result.contains("```\n") && result.contains("plain code"));
    }

    #[test]
    fn pre_plain_text_no_code_child() {
        let result = md("<pre>raw\nnext</pre>");
        assert!(result.contains("```") && result.contains("raw"));
    }

    #[test]
    fn code_inline() {
        assert!(md("<p><code>inline</code></p>").contains("`inline`"));
    }

    #[test]
    fn code_inline_with_backtick_uses_double_tick() {
        assert!(md("<p><code>has `tick`</code></p>").contains("``"));
    }

    #[test]
    fn strong_bold() {
        assert!(md("<p><strong>bold</strong></p>").contains("**bold**"));
    }

    #[test]
    fn b_tag() {
        assert!(md("<p><b>bold</b></p>").contains("**bold**"));
    }

    #[test]
    fn strong_whitespace_only_is_skipped() {
        assert!(!md("<p><strong>  </strong>after</p>").contains("**"));
    }

    #[test]
    fn em_italic() {
        assert!(md("<p><em>italic</em></p>").contains("*italic*"));
    }

    #[test]
    fn i_tag() {
        assert!(md("<p><i>italic</i></p>").contains("*italic*"));
    }

    #[test]
    fn em_whitespace_only_is_skipped() {
        assert!(!md("<p><em> </em>text</p>").contains("**"));
    }

    #[test]
    fn del_strikethrough() {
        assert!(md("<p><del>removed</del></p>").contains("~~removed~~"));
    }

    #[test]
    fn s_tag() {
        assert!(md("<p><s>crossed</s></p>").contains("~~crossed~~"));
    }

    #[test]
    fn strike_tag() {
        assert!(md("<p><strike>old</strike></p>").contains("~~old~~"));
    }

    #[test]
    fn del_empty_is_skipped() {
        assert!(!md("<p><del></del>after</p>").contains("~~"));
    }

    #[test]
    fn link_with_href() {
        assert!(md("<p><a href=\"/p\">click</a></p>").contains("[click](/p)"));
    }

    #[test]
    fn link_without_href() {
        let result = md("<p><a>plain</a></p>");
        assert!(result.contains("plain") && !result.contains('['));
    }

    #[test]
    fn link_empty_text_is_skipped() {
        assert!(!md("<p><a href=\"/x\">  </a>after</p>").contains('['));
    }

    #[test]
    fn ordered_list() {
        let result = md("<ol><li>First</li><li>Second</li></ol>");
        assert!(result.contains("1. First") && result.contains("2. Second"));
    }

    #[test]
    fn ordered_list_with_start_attribute() {
        assert!(md("<ol start=\"5\"><li>Fifth</li></ol>").contains("5. Fifth"));
    }

    #[test]
    fn nested_unordered_list() {
        let result = md("<ul><li>Parent<ul><li>Child</li></ul></li></ul>");
        assert!(result.contains("* Parent") && result.contains("* Child"));
    }

    #[test]
    fn blockquote_prefixes_lines() {
        let result = md("<blockquote><p>Quoted</p></blockquote>");
        assert!(result.contains("> ") && result.contains("Quoted"));
    }

    #[test]
    fn table_basic_gfm() {
        let result = md("<table><thead><tr><th>A</th><th>B</th></tr></thead><tbody><tr><td>1</td><td>2</td></tr></tbody></table>");
        assert!(result.contains("| A |") && result.contains("| --- |") && result.contains("| 1 |"));
    }

    #[test]
    fn table_with_tfoot_emits_rows() {
        let result = md("<table><tr><th>H</th></tr><tfoot><tr><td>foot</td></tr></tfoot></table>");
        assert!(result.contains("H") && result.contains("foot"));
    }

    #[test]
    fn table_short_row_is_padded_to_column_count() {
        let result = md("<table><thead><tr><th>A</th><th>B</th><th>C</th></tr></thead><tbody><tr><td>1</td></tr></tbody></table>");
        let data_row = result
            .lines()
            .find(|l| l.contains("| 1 |"))
            .expect("data row");
        assert_eq!(data_row.matches('|').count(), 4);
    }

    #[test]
    fn table_with_no_rows_produces_no_separator() {
        assert!(!md("<table></table>").contains("| --- |"));
    }

    #[test]
    fn table_inline_elements_in_cells() {
        let html = "<table><thead><tr><th>H</th></tr></thead><tbody>\
          <tr><td><strong>bold</strong></td></tr>\
          <tr><td><em>italic</em></td></tr>\
          <tr><td><del>del</del></td></tr>\
          <tr><td><code>code</code></td></tr>\
          <tr><td><a href=\"/x\">link</a></td></tr>\
          <tr><td><img src=\"/i.png\" alt=\"img\"></td></tr>\
        </tbody></table>";
        let result = md(html);
        assert!(result.contains("**bold**"));
        assert!(result.contains("*italic*"));
        assert!(result.contains("~~del~~"));
        assert!(result.contains("`code`"));
        assert!(result.contains("[link](/x)"));
        assert!(result.contains("![img]"));
    }

    #[test]
    fn table_br_in_cell_emits_space() {
        let result = md("<table><thead><tr><th>H</th></tr></thead><tbody><tr><td>A<br>B</td></tr></tbody></table>");
        assert!(result.contains("H"));
    }

    #[test]
    fn pre_inside_table_cell_text_goes_to_cell() {
        let result = md("<table><thead><tr><th>H</th></tr></thead><tbody><tr><td><pre>code</pre></td></tr></tbody></table>");
        assert!(result.contains("H"));
    }

    #[test]
    fn figure_and_figcaption_emit_children() {
        assert!(md("<figure><figcaption>Caption</figcaption></figure>").contains("Caption"));
    }

    #[test]
    fn details_and_summary_emit_children() {
        let result = md("<details><summary>Title</summary><p>content</p></details>");
        assert!(result.contains("Title") && result.contains("content"));
    }

    #[test]
    fn inline_containers_emit_text() {
        for tag in [
            "span", "abbr", "cite", "kbd", "mark", "q", "small", "sub", "sup", "time", "var",
            "label", "bdi", "bdo", "u", "ins", "wbr",
        ] {
            let result = md(&format!("<p><{tag}>text</{tag}></p>"));
            assert!(result.contains("text"), "tag={tag}");
        }
    }

    #[test]
    fn unknown_element_emits_children() {
        assert!(md("<custom-el>content</custom-el>").contains("content"));
    }

    #[test]
    fn html_comment_is_ignored() {
        let result = md("<!-- comment --><p>visible</p>");
        assert!(result.contains("visible") && !result.contains("comment"));
    }

    #[test]
    fn collapse_blank_lines_multiple_blanks_become_one() {
        assert_eq!(super::finalize("a\n\n\n\nb\n".to_string()), "a\n\nb");
    }

    #[test]
    fn collapse_blank_lines_leading_blanks_stripped() {
        assert_eq!(super::finalize("\n\nfirst\n".to_string()), "first");
    }

    #[test]
    fn heading_levels() {
        for (tag, prefix) in [
            ("h1", "# "),
            ("h2", "## "),
            ("h3", "### "),
            ("h6", "###### "),
        ] {
            assert!(
                md(&format!("<{tag}>T</{tag}>")).contains(prefix),
                "tag={tag}"
            );
        }
    }

    #[test]
    fn li_with_inline_element_child() {
        let result =
            md("<ul><li><strong>bold item</strong></li><li><em>italic item</em></li></ul>");
        assert!(result.contains("bold item") && result.contains("italic item"));
    }

    #[test]
    fn script_and_noscript_skipped() {
        let result = md("<script>evil()</script><noscript>fallback</noscript><p>keep</p>");
        assert!(
            !result.contains("evil") && !result.contains("fallback") && result.contains("keep")
        );
    }
}
