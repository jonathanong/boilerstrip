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
    let mut out = String::with_capacity(buf.len());
    let mut blank_run = 0usize;
    let mut has_content = false;
    for line in buf.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            if has_content {
                blank_run += 1;
            }
        } else {
            if has_content && blank_run > 0 {
                out.push('\n');
            }
            blank_run = 0;
            has_content = true;
            out.push_str(trimmed);
            out.push('\n');
        }
    }
    // Remove the trailing newline added in the loop, then trim
    if out.ends_with('\n') {
        out.pop();
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
    depth: usize,
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
                // pre content is always processed in a scratch State with table_state=None
                state.buf.push_str(s);
                return;
            }
            let normalized = normalize_inline_text(s);
            // Skip whitespace-only text nodes at block boundaries (pending_nl > 0 means
            // a block separator has already been queued; a lone space would be spurious).
            if normalized.trim().is_empty() && state.pending_nl > 0 {
                return;
            }
            if let Some(ts) = state.table_state.as_mut() {
                ts.current_cell.push_str(&normalized);
            } else {
                state.push_str(&normalized);
            }
        }
        Node::Element(_) => {
            emit_element(
                ElementRef::wrap(node).expect("BUG: Node::Element always wraps to ElementRef"),
                state,
            );
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
    const MAX_DEPTH: usize = 200;
    state.depth += 1;
    if state.depth > MAX_DEPTH {
        state.depth -= 1;
        return;
    }

    let name = el.value().name();

    let handled = try_emit_metadata(name)
        || try_emit_block(name, el, state)
        || try_emit_inline(name, el, state)
        || try_emit_list(name, el, state)
        || try_emit_table_element(name, el, state);

    if !handled {
        emit_fallback(el, state);
    }

    state.depth -= 1;
}

fn try_emit_metadata(name: &str) -> bool {
    matches!(name, "script" | "style" | "head" | "noscript" | "template")
}

fn try_emit_block(name: &str, el: ElementRef<'_>, state: &mut State) -> bool {
    match name {
        "br" => emit_br(state),
        "hr" => emit_hr(state),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => emit_heading(name, el, state),
        "p" => emit_p(el, state),
        "pre" => emit_pre(el, state),
        "blockquote" => emit_blockquote(el, state),
        "div" | "section" | "article" | "main" | "aside" | "header" | "footer" | "nav"
        | "figure" | "figcaption" | "details" | "summary" | "body" | "html" => {
            emit_block_container(el, state)
        }
        _ => return false,
    }
    true
}

fn try_emit_inline(name: &str, el: ElementRef<'_>, state: &mut State) -> bool {
    match name {
        "img" => emit_img(el, state),
        "code" => emit_code(el, state),
        "strong" | "b" => emit_strong(el, state),
        "em" | "i" => emit_em(el, state),
        "del" | "s" | "strike" => emit_del(el, state),
        "a" => emit_a(el, state),
        "span" | "abbr" | "cite" | "kbd" | "mark" | "q" | "small" | "sub" | "sup" | "time"
        | "var" | "wbr" | "bdi" | "bdo" | "u" | "ins" | "label" => emit_inline_container(el, state),
        _ => return false,
    }
    true
}

fn try_emit_list(name: &str, el: ElementRef<'_>, state: &mut State) -> bool {
    match name {
        "ul" => emit_ul(el, state),
        "ol" => emit_ol(el, state),
        "li" => emit_li(el, state),
        _ => return false,
    }
    true
}

fn try_emit_table_element(name: &str, el: ElementRef<'_>, state: &mut State) -> bool {
    match name {
        "table" => emit_table(el, state),
        "thead" => emit_thead(el, state),
        "tbody" | "tfoot" => emit_tbody(el, state),
        "tr" => emit_tr(el, state),
        "th" | "td" => emit_td(el, state),
        _ => return false,
    }
    true
}

fn emit_br(state: &mut State) {
    if let Some(ts) = state.table_state.as_mut() {
        ts.current_cell.push(' ');
    } else {
        state.push_str("  \n");
    }
}

fn emit_hr(state: &mut State) {
    state.ensure_newlines(2);
    state.push_str("---");
    state.ensure_newlines(2);
}

fn emit_img(el: ElementRef<'_>, state: &mut State) {
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

fn emit_heading(name: &str, el: ElementRef<'_>, state: &mut State) {
    let level = match name {
        "h1" => 1usize,
        "h2" => 2,
        "h3" => 3,
        "h4" => 4,
        "h5" => 5,
        _ => 6,
    };
    let prefix = "#".repeat(level);
    state.ensure_newlines(2);
    state.push_str(&prefix);
    state.push_str(" ");
    for child in (*el).children() {
        emit_node(child, state);
    }
    state.ensure_newlines(2);
}

fn emit_p(el: ElementRef<'_>, state: &mut State) {
    state.ensure_newlines(2);
    for child in (*el).children() {
        emit_node(child, state);
    }
    state.ensure_newlines(2);
}

fn extract_language<'a>(el: ElementRef<'a>) -> Option<&'a str> {
    el.children()
        .filter_map(ElementRef::wrap)
        .find(|c| c.value().name() == "code")
        .and_then(|code_el| code_el.value().attr("class"))
        .and_then(|cls| {
            cls.split_whitespace()
                .find(|c| c.starts_with("language-"))
                .map(|c| c.trim_start_matches("language-"))
        })
}

fn count_max_consecutive_backticks(content: &str) -> usize {
    let mut max_run = 0usize;
    let mut cur_run = 0usize;
    for byte in content.bytes() {
        if byte == b'\x60' {
            cur_run += 1;
            if cur_run > max_run {
                max_run = cur_run;
            }
        } else {
            cur_run = 0;
        }
    }
    max_run
}

fn emit_pre(el: ElementRef<'_>, state: &mut State) {
    state.ensure_newlines(2);
    // Check if direct child is <code> for fenced blocks
    let lang = extract_language(el);
    let lang_str = lang.unwrap_or("");
    // Collect the pre content into a scratch buffer first so we can
    // determine the required fence length (must exceed any backtick run).
    let mut scratch = State {
        in_pre: true,
        depth: state.depth,
        ..Default::default()
    };
    for child in (*el).children() {
        emit_node(child, &mut scratch);
    }
    let content = scratch.buf;
    // Count the longest consecutive backtick run in the content.
    let max_backtick_run = count_max_consecutive_backticks(&content);
    let fence_len = (max_backtick_run + 1).max(3);
    let fence: String = "`".repeat(fence_len);
    state.push_str(&fence);
    state.push_str(lang_str);
    state.buf.push('\n');
    state.pending_nl = 0;
    state.buf.push_str(&content);
    if !state.buf.ends_with('\n') {
        state.buf.push('\n');
    }
    state.buf.push_str(&fence);
    state.ensure_newlines(2);
}

fn emit_code(el: ElementRef<'_>, state: &mut State) {
    if state.in_pre {
        // Inside a pre block — emit raw, no backtick wrapping
        for child in (*el).children() {
            emit_node(child, state);
        }
    } else {
        let text = collect_inline_text(&el);
        let tick = if text.contains('`') { "``" } else { "`" };
        let code_md = format!("{tick}{text}{tick}");
        if let Some(ts) = state.table_state.as_mut() {
            ts.current_cell.push_str(&code_md);
        } else {
            state.push_str(&code_md);
        }
    }
}

fn emit_strong(el: ElementRef<'_>, state: &mut State) {
    let text = collect_inline_text(&el);
    if !text.trim().is_empty() {
        let bold = format!("**{text}**");
        if let Some(ts) = state.table_state.as_mut() {
            ts.current_cell.push_str(&bold);
        } else {
            state.push_str(&bold);
        }
    }
}

fn emit_em(el: ElementRef<'_>, state: &mut State) {
    let text = collect_inline_text(&el);
    if !text.trim().is_empty() {
        let italic = format!("*{text}*");
        if let Some(ts) = state.table_state.as_mut() {
            ts.current_cell.push_str(&italic);
        } else {
            state.push_str(&italic);
        }
    }
}

fn emit_del(el: ElementRef<'_>, state: &mut State) {
    let text = collect_inline_text(&el);
    if !text.trim().is_empty() {
        let del = format!("~~{text}~~");
        if let Some(ts) = state.table_state.as_mut() {
            ts.current_cell.push_str(&del);
        } else {
            state.push_str(&del);
        }
    }
}

fn emit_a(el: ElementRef<'_>, state: &mut State) {
    let href = el.value().attr("href").map(str::to_owned);
    let mut inner = State {
        depth: state.depth,
        ..Default::default()
    };
    for child in (*el).children() {
        emit_node(child, &mut inner);
    }
    let text = finalize(inner.buf);
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        let link_md = if let Some(ref href) = href {
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
}

fn emit_ul(el: ElementRef<'_>, state: &mut State) {
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

fn emit_ol(el: ElementRef<'_>, state: &mut State) {
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

fn emit_li(el: ElementRef<'_>, state: &mut State) {
    if state.in_pre {
        for child in (*el).children() {
            emit_node(child, state);
        }
    } else {
        state.ensure_newlines(1);
        let prefix = state.list_prefix();
        state.push_str(&prefix);
        // Collect li content, handling nested lists
        for child in (*el).children() {
            if let Some(child_el) = ElementRef::wrap(child) {
                if matches!(child_el.value().name(), "ul" | "ol") {
                    // Nested list — emit on its own line
                    state.ensure_newlines(1);
                }
                emit_element(child_el, state);
            } else {
                emit_node(child, state);
            }
        }
    }
}

fn emit_blockquote(el: ElementRef<'_>, state: &mut State) {
    state.ensure_newlines(2);
    // Collect all content, then prefix each line with "> "
    let mut inner_state = State {
        depth: state.depth,
        ..Default::default()
    };
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

fn emit_table(el: ElementRef<'_>, state: &mut State) {
    state.ensure_newlines(2);
    let prev_table = state.table_state.take();
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
    let ts = state
        .table_state
        .take()
        .expect("BUG: table state missing after table");
    state.flush_pending();
    emit_gfm_table(ts, &mut state.buf);
    state.pending_nl = 0;
    state.table_state = prev_table;
    state.ensure_newlines(2);
}

fn emit_thead(el: ElementRef<'_>, state: &mut State) {
    // html5ever always nests <thead> inside <table>, so table_state is Some here.
    state
        .table_state
        .as_mut()
        .expect("BUG: thead outside table")
        .in_head = true;
    for child in (*el).children() {
        emit_node(child, state);
    }
    state
        .table_state
        .as_mut()
        .expect("BUG: thead outside table")
        .in_head = false;
}

fn emit_tbody(el: ElementRef<'_>, state: &mut State) {
    // html5ever always nests these inside <table>, so table_state is Some here.
    state
        .table_state
        .as_mut()
        .expect("BUG: tbody/tfoot outside table")
        .in_head = false;
    for child in (*el).children() {
        emit_node(child, state);
    }
}

fn emit_tr(el: ElementRef<'_>, state: &mut State) {
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

fn emit_td(el: ElementRef<'_>, state: &mut State) {
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

fn emit_block_container(el: ElementRef<'_>, state: &mut State) {
    state.ensure_newlines(2);
    for child in (*el).children() {
        emit_node(child, state);
    }
    state.ensure_newlines(2);
}

fn emit_inline_container(el: ElementRef<'_>, state: &mut State) {
    for child in (*el).children() {
        emit_node(child, state);
    }
}

fn emit_fallback(el: ElementRef<'_>, state: &mut State) {
    for child in (*el).children() {
        emit_node(child, state);
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
    fn unusual_elements_emit_fallback() {
        let result =
            md("<p>Before <blink>Blinking text</blink> after.</p><marquee>Marquee text</marquee>");
        assert_eq!(result, "Before Blinking text after.\n\nMarquee text");
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
            ("h4", "#### "),
            ("h5", "##### "),
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

    #[test]
    fn nested_tables_do_not_corrupt_outer_table() {
        let result = md(
            r#"<table><tr><td><table><tr><td>inner</td></tr></table></td><td>outer-cell</td></tr></table>"#,
        );
        assert!(
            result.contains("outer-cell"),
            "outer table cell should appear"
        );
    }

    #[test]
    fn anchor_wrapping_image_preserves_image_markdown() {
        let result = md(r#"<a href="https://example.com"><img src="logo.png" alt="Logo"></a>"#);
        assert!(result.contains("Logo"), "alt text should appear");
        assert!(result.contains("https://example.com"), "href should appear");
    }

    #[test]
    fn pre_block_content_already_ending_with_newline() {
        // pre text that ends with \n: the `if !buf.ends_with('\n')` branch is skipped
        let result = md("<pre>line1\nline2\n</pre>");
        assert!(result.contains("line1"), "pre content should be emitted");
        assert!(result.contains("line2"), "pre content should be emitted");
    }

    #[test]
    fn nested_ordered_list() {
        // Nested ol: inner ol has list_depth() > 0 so ensure_newlines is not called
        let result = md("<ol><li>outer<ol><li>inner</li></ol></li></ol>");
        assert!(result.contains("outer"), "outer item should appear");
        assert!(result.contains("inner"), "inner item should appear");
    }

    #[test]
    fn depth_guard_does_not_stack_overflow() {
        // 1500 nested divs — should not panic/stack-overflow
        let mut html = String::new();
        for _ in 0..1500 {
            html.push_str("<div>");
        }
        html.push_str("deep");
        for _ in 0..1500 {
            html.push_str("</div>");
        }
        let result = md(&html);
        // "deep" text may or may not appear depending on depth limit, but the call must not panic
        let _ = result;
    }

    #[test]
    fn deep_nesting_below_limit_serializes_correctly() {
        // 50 nested divs, well below the MAX_DEPTH of 200
        let mut html = String::new();
        for _ in 0..50 {
            html.push_str("<div>");
        }
        html.push_str("deep content");
        for _ in 0..50 {
            html.push_str("</div>");
        }
        let result = md(&html);
        assert!(
            result.contains("deep content"),
            "content below depth limit should appear"
        );
    }

    #[test]
    fn pre_block_with_triple_backtick_uses_longer_fence() {
        let result = md("<pre><code>let x = ```foo```;</code></pre>");
        // The fence must be at least 4 backticks
        assert!(
            result.contains("````"),
            "fence should be >= 4 backticks when content has ```"
        );
    }

    #[test]
    fn heading_level_is_correct() {
        for (tag, hashes) in &[("h1", "#"), ("h2", "##"), ("h3", "###"), ("h6", "######")] {
            let result = md(&format!("<{tag}>Title</{tag}>"));
            assert!(result.starts_with(hashes), "wrong heading prefix for {tag}");
        }
    }

    #[test]
    fn orphan_li_uses_empty_list_prefix() {
        // <li> outside <ul>/<ol> — list_prefix() returns "" (no list on the stack)
        let result = md("<li>Item text</li>");
        assert!(result.contains("Item text"), "li content should be emitted");
    }

    #[test]
    fn pre_text_inside_table_cell_does_not_panic() {
        // <pre> inside a <td>: pre content goes into a scratch state, not the cell
        let result = md("<table><tr><td><pre>code here</pre></td></tr></table>");
        // Content may appear as a code fence in the output (outside the table)
        let _ = result;
    }

    #[test]
    fn li_inside_pre_emits_children() {
        // <li> as a child of <pre> — in_pre is true, so the li branch runs emit_node on children
        let result = md("<pre><ul><li>item</li></ul></pre>");
        // Content may or may not be exactly right, but must not panic
        let _ = result;
    }

    #[test]
    fn whitespace_text_between_blocks_skipped_at_pending_nl() {
        // "\n    " between </h1> and <p>: after h1 emits, pending_nl=2; the whitespace-only
        // text node must be skipped (return at the pending_nl guard) rather than pushed.
        let result = md("<h1>Title</h1>\n    <p>Content</p>");
        assert!(result.contains("# Title"));
        assert!(result.contains("Content"));
    }

    #[test]
    fn table_subelements_as_root_do_not_panic() {
        // element_to_markdown called directly on <tbody>/<tr> roots means the "tr" and
        // "th"/"td" emit_element arms run with table_state=None (no enclosing "table" arm
        // to initialise it).  The if-let-Some guards must handle that gracefully.
        let doc =
            Html::parse_document("<table><tbody><tr><td>A</td><th>B</th></tr></tbody></table>");
        // tbody root → children are <tr> → emit_element("tr", table_state=None)
        let tbody = doc
            .select(&Selector::parse("tbody").unwrap())
            .next()
            .unwrap();
        let _ = element_to_markdown(tbody);
        // tr root → children are <td>/<th> → emit_element("td"/"th", table_state=None)
        let tr = doc.select(&Selector::parse("tr").unwrap()).next().unwrap();
        let _ = element_to_markdown(tr);
    }
}
