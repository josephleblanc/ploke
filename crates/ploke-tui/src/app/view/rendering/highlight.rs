//! Markdown rendering and syntax highlighting for chat content.
//!
//! Table handling summary:
//! - Only inline content inside cells is supported (text, inline code, emphasis).
//! - Block elements inside cells (fenced code blocks, block quotes, nested tables)
//!   are not supported by CommonMark tables and will end the table parse.
//! - Inline pipes must be escaped to stay inside a cell; raw pipes split cells.
//! - HTML line breaks like `<br>` are normalized to spaces inside cells.
//! - We render a single header separator line using computed column widths,
//!   with a small minimum dash width for readability.
use once_cell::sync::Lazy;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::highlighting::{FontStyle, Style as SyntectStyle, Theme};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Debug, PartialEq)]
pub struct StyledSpan {
    pub content: String,
    pub style: Style,
}

pub type StyledLine = Vec<StyledSpan>;

static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);
static THEME: Lazy<Theme> = Lazy::new(|| {
    let mut themes = ThemeSet::load_defaults();
    themes
        .themes
        .remove("base16-ocean.dark")
        .or_else(|| themes.themes.into_values().next())
        .unwrap_or_default()
});

const DIFF_ADDITION: Color = Color::LightGreen;
const DIFF_REMOVAL: Color = Color::LightRed;
const DIFF_INFO: Color = Color::Yellow;
const DIFF_META: Color = Color::Cyan;

pub fn highlight_message_lines(content: &str, base_style: Style, width: u16) -> Vec<StyledLine> {
    let width = width.max(1) as usize;
    let diff_enabled = detect_diff_markers(content);

    let mut lines: Vec<StyledLine> = Vec::new();
    let mut current_line: StyledLine = Vec::new();

    let mut italic_depth = 0usize;
    let mut bold_depth = 0usize;

    enum Mode {
        Normal,
        CodeBlock {
            lang: Option<String>,
            buffer: String,
        },
    }
    let mut mode = Mode::Normal;
    // Table rendering notes:
    // - We only support inline content inside cells (text, inline code, emphasis).
    // - Block elements inside cells (fenced code blocks, block quotes, nested tables)
    //   are not supported by CommonMark tables and will end the table parse.
    // - Inline pipes must be escaped to stay inside a cell; raw pipes split cells.
    // - HTML line breaks like <br> are normalized to spaces inside cells.
    // - We render a single header separator line using computed column widths,
    //   with a small minimum dash width for readability.
    struct TableRow {
        cells: Vec<StyledLine>,
        is_header: bool,
    }
    struct TableState {
        rows: Vec<TableRow>,
        current_row: Option<TableRow>,
        current_cell: Option<StyledLine>,
        in_head: bool,
    }
    impl TableState {
        fn new() -> Self {
            Self {
                rows: Vec::new(),
                current_row: None,
                current_cell: None,
                in_head: false,
            }
        }

        fn start_row(&mut self) {
            self.end_row();
            self.current_row = Some(TableRow {
                cells: Vec::new(),
                is_header: self.in_head,
            });
        }

        fn end_row(&mut self) {
            self.end_cell();
            if let Some(row) = self.current_row.take() {
                self.rows.push(row);
            }
        }

        fn start_cell(&mut self) {
            self.end_cell();
            self.current_cell = Some(Vec::new());
        }

        fn end_cell(&mut self) {
            if let Some(cell) = self.current_cell.take() {
                if let Some(row) = self.current_row.as_mut() {
                    row.cells.push(cell);
                }
            }
        }
    }
    let mut table_state: Option<TableState> = None;
    #[derive(Clone, Copy)]
    struct ListState {
        ordered: Option<u64>,
        next_index: u64,
    }
    let mut list_stack: Vec<ListState> = Vec::new();

    let push_newline =
        |current_line: &mut StyledLine, lines: &mut Vec<StyledLine>, diff_enabled: bool| {
            if current_line.is_empty() {
                return;
            }
            let mut line = std::mem::take(current_line);
            if diff_enabled {
                line = apply_diff_to_line(line, diff_enabled);
            }
            lines.push(line);
        };

    let push_text = |text: &str,
                     style: Style,
                     current_line: &mut StyledLine,
                     lines: &mut Vec<StyledLine>,
                     diff_enabled: bool| {
        let mut last = 0usize;
        for (idx, ch) in text.char_indices() {
            if ch == '\n' {
                if idx > last {
                    current_line.push(StyledSpan {
                        content: text[last..idx].to_string(),
                        style,
                    });
                }
                push_newline(current_line, lines, diff_enabled);
                last = idx + ch.len_utf8();
            }
        }
        if last < text.len() {
            current_line.push(StyledSpan {
                content: text[last..].to_string(),
                style,
            });
        }
    };
    let push_table_text = |table: &mut TableState, text: &str, style: Style| {
        if table.current_row.is_none() {
            table.start_row();
        }
        if table.current_cell.is_none() {
            table.start_cell();
        }
        let cell = table
            .current_cell
            .as_mut()
            .expect("cell ensured before text");
        let mut last = 0usize;
        for (idx, ch) in text.char_indices() {
            if ch == '\n' {
                if idx > last {
                    cell.push(StyledSpan {
                        content: text[last..idx].to_string(),
                        style,
                    });
                }
                cell.push(StyledSpan {
                    content: " ".to_string(),
                    style,
                });
                last = idx + ch.len_utf8();
            }
        }
        if last < text.len() {
            cell.push(StyledSpan {
                content: text[last..].to_string(),
                style,
            });
        }
    };
    let render_table = |rows: &[TableRow], base_style: Style| -> Vec<StyledLine> {
        const TABLE_GAP: usize = 2;
        const MIN_DASH_WIDTH: usize = 3;
        let mut col_count = 0usize;
        for row in rows {
            col_count = col_count.max(row.cells.len());
        }
        if col_count == 0 {
            return Vec::new();
        }
        let mut widths = vec![0usize; col_count];
        for row in rows {
            for (idx, cell) in row.cells.iter().enumerate() {
                let width = styled_line_width(cell);
                if width > widths[idx] {
                    widths[idx] = width;
                }
            }
        }
        for width in &mut widths {
            *width = (*width).max(MIN_DASH_WIDTH);
        }
        let mut rendered = Vec::new();
        for row in rows {
            let mut line = StyledLine::new();
            for col_idx in 0..col_count {
                if col_idx > 0 {
                    line.push(StyledSpan {
                        content: " ".repeat(TABLE_GAP),
                        style: base_style,
                    });
                }
                let cell = row.cells.get(col_idx);
                if let Some(cell) = cell {
                    line.extend(cell.iter().cloned());
                    if col_idx + 1 < col_count {
                        let padding = widths[col_idx].saturating_sub(styled_line_width(cell));
                        if padding > 0 {
                            line.push(StyledSpan {
                                content: " ".repeat(padding),
                                style: base_style,
                            });
                        }
                    }
                } else if col_idx + 1 < col_count && widths[col_idx] > 0 {
                    line.push(StyledSpan {
                        content: " ".repeat(widths[col_idx]),
                        style: base_style,
                    });
                }
            }
            rendered.push(line);
            if row.is_header {
                let mut separator = StyledLine::new();
                for (idx, width) in widths.iter().enumerate() {
                    if idx > 0 {
                        separator.push(StyledSpan {
                            content: " ".repeat(TABLE_GAP),
                            style: base_style,
                        });
                    }
                    separator.push(StyledSpan {
                        content: "-".repeat(*width),
                        style: base_style,
                    });
                }
                rendered.push(separator);
            }
        }
        rendered
    };

    let parser = Parser::new_ext(content, Options::all());
    for event in parser {
        match (&mut mode, &event) {
            (_, Event::Start(Tag::Table(_))) => {
                if !current_line.is_empty() {
                    push_newline(&mut current_line, &mut lines, diff_enabled);
                }
                table_state = Some(TableState::new());
            }
            (_, Event::End(TagEnd::Table)) => {
                if let Some(mut table) = table_state.take() {
                    table.end_row();
                    lines.extend(render_table(&table.rows, base_style));
                }
            }
            (_, Event::Start(Tag::TableHead)) => {
                if let Some(table) = table_state.as_mut() {
                    table.in_head = true;
                }
            }
            (_, Event::End(TagEnd::TableHead)) => {
                if let Some(table) = table_state.as_mut() {
                    table.end_row();
                    table.in_head = false;
                }
            }
            (_, Event::Start(Tag::TableRow)) => {
                if let Some(table) = table_state.as_mut() {
                    table.start_row();
                }
            }
            (_, Event::End(TagEnd::TableRow)) => {
                if let Some(table) = table_state.as_mut() {
                    table.end_row();
                }
            }
            (_, Event::Start(Tag::TableCell)) => {
                if let Some(table) = table_state.as_mut() {
                    table.start_cell();
                }
            }
            (_, Event::End(TagEnd::TableCell)) => {
                if let Some(table) = table_state.as_mut() {
                    table.end_cell();
                }
            }
            (_, Event::Start(Tag::CodeBlock(kind))) => {
                if !current_line.is_empty() {
                    push_newline(&mut current_line, &mut lines, diff_enabled);
                }
                mode = Mode::CodeBlock {
                    lang: codeblock_lang_hint(&kind),
                    buffer: String::new(),
                };
            }
            (Mode::CodeBlock { .. }, Event::End(TagEnd::CodeBlock)) => {
                if let Mode::CodeBlock { lang, buffer } = std::mem::replace(&mut mode, Mode::Normal)
                {
                    let mut code_lines: Vec<String> =
                        buffer.split('\n').map(|s| s.to_string()).collect();
                    let highlighted = if lang
                        .as_deref()
                        .map(|l| matches!(l, "diff" | "patch"))
                        .unwrap_or(false)
                    {
                        code_lines.iter().map(|l| diff_line(l)).collect::<Vec<_>>()
                    } else {
                        highlight_code_block(&mut code_lines, lang.as_deref())
                    };
                    lines.extend(highlighted);
                }
            }
            (Mode::CodeBlock { buffer, .. }, Event::Text(t)) => {
                buffer.push_str(&t);
            }
            (Mode::CodeBlock { buffer, .. }, Event::SoftBreak | Event::HardBreak) => {
                buffer.push('\n');
            }
            (Mode::CodeBlock { buffer, .. }, Event::Code(t)) => {
                buffer.push_str(&t);
            }
            (_, Event::Start(Tag::Paragraph))
            | (_, Event::Start(Tag::Heading { .. }))
            | (_, Event::Start(Tag::BlockQuote(_)))
            | (_, Event::Start(Tag::FootnoteDefinition(_))) => {
                if !current_line.is_empty() {
                    push_newline(&mut current_line, &mut lines, diff_enabled);
                }
            }
            (_, Event::Start(Tag::List(start))) => {
                if !current_line.is_empty() {
                    push_newline(&mut current_line, &mut lines, diff_enabled);
                }
                let next_index = start.unwrap_or(1);
                list_stack.push(ListState {
                    ordered: *start,
                    next_index,
                });
            }
            (_, Event::End(TagEnd::Paragraph))
            | (_, Event::End(TagEnd::Heading(..)))
            | (_, Event::End(TagEnd::BlockQuote))
            | (_, Event::End(TagEnd::FootnoteDefinition)) => {
                push_newline(&mut current_line, &mut lines, diff_enabled);
            }
            (_, Event::End(TagEnd::List(_))) => {
                let _ = list_stack.pop();
                push_newline(&mut current_line, &mut lines, diff_enabled);
            }
            (_, Event::Text(t)) => {
                let style = inline_style(base_style, italic_depth, bold_depth);
                if let Some(table) = table_state.as_mut() {
                    push_table_text(table, &t, style);
                } else {
                    push_text(&t, style, &mut current_line, &mut lines, diff_enabled);
                }
            }
            (_, Event::Code(t)) => {
                let style = inline_code_style(base_style);
                if let Some(table) = table_state.as_mut() {
                    push_table_text(table, &t, style);
                } else {
                    push_text(&t, style, &mut current_line, &mut lines, diff_enabled);
                }
            }
            (_, Event::SoftBreak | Event::HardBreak) => {
                if let Some(table) = table_state.as_mut() {
                    let style = inline_style(base_style, italic_depth, bold_depth);
                    push_table_text(table, " ", style);
                } else {
                    push_newline(&mut current_line, &mut lines, diff_enabled);
                }
            }
            (_, Event::Start(Tag::Emphasis)) => {
                italic_depth = italic_depth.saturating_add(1);
            }
            (_, Event::End(TagEnd::Emphasis)) => {
                italic_depth = italic_depth.saturating_sub(1);
            }
            (_, Event::Start(Tag::Strong)) => {
                bold_depth = bold_depth.saturating_add(1);
            }
            (_, Event::End(TagEnd::Strong)) => {
                bold_depth = bold_depth.saturating_sub(1);
            }
            (_, Event::Html(t)) | (_, Event::InlineHtml(t)) => {
                let style = inline_style(base_style, italic_depth, bold_depth);
                if let Some(table) = table_state.as_mut() {
                    if html_is_line_break(t) {
                        push_table_text(table, " ", style);
                    } else {
                        push_table_text(table, t, style);
                    }
                } else {
                    push_text(t, style, &mut current_line, &mut lines, diff_enabled);
                }
            }
            (_, Event::FootnoteReference(t)) => {
                let style = inline_style(base_style, italic_depth, bold_depth);
                if let Some(table) = table_state.as_mut() {
                    push_table_text(table, t, style);
                } else {
                    push_text(t, style, &mut current_line, &mut lines, diff_enabled);
                }
            }
            (_, Event::TaskListMarker(done)) => {
                let marker = if *done { "[x] " } else { "[ ] " };
                let style = inline_style(base_style, italic_depth, bold_depth);
                if let Some(table) = table_state.as_mut() {
                    push_table_text(table, marker, style);
                } else {
                    push_text(marker, style, &mut current_line, &mut lines, diff_enabled);
                }
            }
            (_, Event::Rule) => {
                let style = inline_style(base_style, italic_depth, bold_depth);
                push_text("—", style, &mut current_line, &mut lines, diff_enabled);
                push_newline(&mut current_line, &mut lines, diff_enabled);
            }
            (_, Event::InlineMath(t)) | (_, Event::DisplayMath(t)) => {
                let style = inline_style(base_style, italic_depth, bold_depth);
                if let Some(table) = table_state.as_mut() {
                    push_table_text(table, t, style);
                } else {
                    push_text(t, style, &mut current_line, &mut lines, diff_enabled);
                }
            }
            (_, Event::Start(Tag::Item)) => {
                if !current_line.is_empty() {
                    push_newline(&mut current_line, &mut lines, diff_enabled);
                }
                let prefix = if let Some(state) = list_stack.last() {
                    if state.ordered.is_some() {
                        format!("{}. ", state.next_index)
                    } else {
                        "- ".to_string()
                    }
                } else {
                    "- ".to_string()
                };
                let style = inline_style(base_style, italic_depth, bold_depth);
                push_text(&prefix, style, &mut current_line, &mut lines, diff_enabled);
            }
            (_, Event::End(TagEnd::Item)) => {
                if let Some(state) = list_stack.last_mut()
                    && state.ordered.is_some()
                {
                    state.next_index = state.next_index.saturating_add(1);
                }
                push_newline(&mut current_line, &mut lines, diff_enabled);
            }
            (Mode::CodeBlock { buffer, .. }, _) => {
                // Other events inside code blocks become text.
                if let Some(text) = event_to_string(&event) {
                    buffer.push_str(&text);
                }
            }
            (_, _) => {}
        }
    }

    // Flush trailing content
    match mode {
        Mode::Normal => {
            if !current_line.is_empty() {
                push_newline(&mut current_line, &mut lines, diff_enabled);
            }
        }
        Mode::CodeBlock { lang, buffer } => {
            let mut code_lines: Vec<String> = buffer.split('\n').map(|s| s.to_string()).collect();
            let highlighted = if lang
                .as_deref()
                .map(|l| matches!(l, "diff" | "patch"))
                .unwrap_or(false)
            {
                code_lines.iter().map(|l| diff_line(l)).collect::<Vec<_>>()
            } else {
                highlight_code_block(&mut code_lines, lang.as_deref())
            };
            lines.extend(highlighted);
        }
    }

    wrap_lines(lines, width)
}

pub fn highlight_diff_text(content: &str, width: u16) -> Vec<StyledLine> {
    let width = width.max(1) as usize;
    let unwrapped = content.split('\n').map(diff_line).collect::<Vec<_>>();
    wrap_lines(unwrapped, width)
}

pub fn styled_to_ratatui_lines(lines: Vec<StyledLine>) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .map(|styled_line| {
            let spans: Vec<Span<'static>> = styled_line
                .into_iter()
                .map(|span| Span::styled(span.content, span.style))
                .collect();
            Line::from(spans)
        })
        .collect()
}

fn plain_text_line(line: &str, base_style: Style, diff_enabled: bool) -> StyledLine {
    if diff_enabled && let Some(style) = diff_style(line) {
        return vec![StyledSpan {
            content: line.to_string(),
            style,
        }];
    }
    vec![StyledSpan {
        content: line.to_string(),
        style: base_style,
    }]
}

fn plain_text_line_unguarded(line: &str, base_style: Style) -> StyledLine {
    if let Some(style) = diff_style(line) {
        vec![StyledSpan {
            content: line.to_string(),
            style,
        }]
    } else {
        vec![StyledSpan {
            content: line.to_string(),
            style: base_style,
        }]
    }
}

fn diff_line(line: &str) -> StyledLine {
    let style = diff_style(line).unwrap_or_else(|| Style::new().fg(Color::White));
    vec![StyledSpan {
        content: line.to_string(),
        style,
    }]
}

fn diff_style(line: &str) -> Option<Style> {
    if line.starts_with("+") && !line.starts_with("+++") {
        Some(Style::new().fg(DIFF_ADDITION))
    } else if line.starts_with("-") && !line.starts_with("---") {
        Some(Style::new().fg(DIFF_REMOVAL))
    } else if line.starts_with("@@") {
        Some(Style::new().fg(DIFF_INFO).add_modifier(Modifier::BOLD))
    } else if line.starts_with("diff --") || line.starts_with("index ") {
        Some(Style::new().fg(DIFF_META))
    } else if line.starts_with("---") || line.starts_with("+++") {
        Some(Style::new().fg(Color::LightBlue))
    } else {
        None
    }
}

fn highlight_code_block(lines: &mut Vec<String>, lang: Option<&str>) -> Vec<StyledLine> {
    let syntax = lang
        .and_then(find_syntax)
        .or_else(|| SYNTAX_SET.find_syntax_by_extension("txt"))
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());
    let mut highlighter = HighlightLines::new(syntax, &THEME);
    let mut highlighted = Vec::new();
    for line in lines.drain(..) {
        let spans = highlighter
            .highlight_line(&line, &SYNTAX_SET)
            .map(|ranges| {
                ranges
                    .into_iter()
                    .filter(|(_, text)| !text.is_empty())
                    .map(|(style, text)| StyledSpan {
                        content: text.to_string(),
                        style: syntect_style_to_ratatui(style),
                    })
                    .collect::<StyledLine>()
            })
            .unwrap_or_else(|_| plain_text_line_unguarded(&line, Style::default()));
        if spans.is_empty() {
            highlighted.push(vec![StyledSpan {
                content: String::new(),
                style: Style::default(),
            }]);
        } else {
            highlighted.push(spans);
        }
    }
    if highlighted.is_empty() {
        highlighted.push(vec![StyledSpan {
            content: String::new(),
            style: Style::default(),
        }]);
    }
    highlighted
}

fn detect_diff_markers(content: &str) -> bool {
    let mut plain = String::new();
    let mut in_code = false;
    for event in Parser::new_ext(content, Options::all()) {
        match event {
            Event::Start(Tag::CodeBlock(_)) => in_code = true,
            Event::End(TagEnd::CodeBlock) => in_code = false,
            Event::Text(t) | Event::Code(t) if !in_code => plain.push_str(&t),
            Event::Html(t) | Event::InlineHtml(t) if !in_code => plain.push_str(&t),
            Event::SoftBreak | Event::HardBreak if !in_code => plain.push('\n'),
            _ => {}
        }
    }
    for line in plain.lines() {
        if line.starts_with("diff --")
            || line.starts_with("index ")
            || line.starts_with("---")
            || line.starts_with("+++")
            || line.starts_with("@@")
        {
            return true;
        }
    }
    plain.lines().any(|l| l.starts_with("@@"))
}

fn find_syntax(lang: &str) -> Option<&'static SyntaxReference> {
    let normalized = lang.trim();
    if normalized.is_empty() {
        return None;
    }
    let primary = normalized
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .find(|s| !s.is_empty())
        .unwrap_or(normalized);
    if let Some(syntax) = SYNTAX_SET.find_syntax_by_token(primary) {
        return Some(syntax);
    }
    if let Some(syntax) = SYNTAX_SET.find_syntax_by_name(primary) {
        return Some(syntax);
    }
    if let Some(syntax) = SYNTAX_SET.find_syntax_by_extension(primary) {
        return Some(syntax);
    }
    let lower = primary.to_lowercase();
    if lower != primary {
        if let Some(syntax) = SYNTAX_SET.find_syntax_by_token(&lower) {
            return Some(syntax);
        }
        if let Some(syntax) = SYNTAX_SET.find_syntax_by_name(&lower) {
            return Some(syntax);
        }
        if let Some(syntax) = SYNTAX_SET.find_syntax_by_extension(&lower) {
            return Some(syntax);
        }
    }
    None
}

fn inline_style(base: Style, italic_depth: usize, bold_depth: usize) -> Style {
    let mut style = base;
    if italic_depth > 0 {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if bold_depth > 0 {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
}

fn inline_code_style(base: Style) -> Style {
    base.add_modifier(Modifier::REVERSED)
}

fn apply_diff_to_line(line: StyledLine, diff_enabled: bool) -> StyledLine {
    if !diff_enabled {
        return line;
    }
    let text: String = line.iter().map(|s| s.content.as_str()).collect();
    if let Some(style) = diff_style(&text) {
        return vec![StyledSpan {
            content: text,
            style,
        }];
    }
    line
}

fn codeblock_lang_hint(kind: &CodeBlockKind) -> Option<String> {
    match kind {
        CodeBlockKind::Fenced(info) => {
            let trimmed = info.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        CodeBlockKind::Indented => None,
    }
}

fn event_to_string(event: &Event<'_>) -> Option<String> {
    match event {
        Event::Text(t) | Event::Code(t) | Event::Html(t) | Event::InlineHtml(t) => {
            Some(t.to_string())
        }
        Event::SoftBreak | Event::HardBreak => Some("\n".to_string()),
        _ => None,
    }
}

fn syntect_style_to_ratatui(style: SyntectStyle) -> Style {
    let mut tui_style = Style::new().fg(Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    ));
    if style.font_style.contains(FontStyle::BOLD) {
        tui_style = tui_style.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        tui_style = tui_style.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        tui_style = tui_style.add_modifier(Modifier::UNDERLINED);
    }
    tui_style
}

fn wrap_lines(lines: Vec<StyledLine>, width: usize) -> Vec<StyledLine> {
    let width = width.max(1);
    let mut wrapped = Vec::new();
    for line in lines {
        wrapped.extend(wrap_single_line(&line, width));
    }
    if wrapped.is_empty() {
        wrapped.push(vec![StyledSpan {
            content: String::new(),
            style: Style::default(),
        }]);
    }
    wrapped
}

fn styled_line_width(line: &StyledLine) -> usize {
    line.iter()
        .map(|span| {
            span.content
                .chars()
                .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
                .sum::<usize>()
        })
        .sum()
}

fn html_is_line_break(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.len() < 4 {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    matches!(lower.as_str(), "<br>" | "<br/>" | "<br />")
}

fn wrap_single_line(line: &StyledLine, width: usize) -> Vec<StyledLine> {
    let mut lines = Vec::new();
    let mut current: StyledLine = Vec::new();
    let mut current_width = 0usize;
    let mut had_content = false;

    for span in line {
        if span.content.is_empty() {
            continue;
        }
        had_content = true;
        let mut remaining = span.content.as_str();
        while !remaining.is_empty() {
            if current_width >= width {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
            let available = width.saturating_sub(current_width);
            if available == 0 {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
                continue;
            }
            let (take_bytes, take_width) = take_prefix_by_width(remaining, available);
            if take_bytes == 0 {
                break;
            }
            let chunk = &remaining[..take_bytes];
            current.push(StyledSpan {
                content: chunk.to_string(),
                style: span.style,
            });
            current_width += take_width;
            remaining = &remaining[take_bytes..];
            if current_width >= width {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
        }
    }

    if !current.is_empty() {
        lines.push(current);
    } else if !had_content {
        let style = line
            .get(0)
            .map(|span| span.style)
            .unwrap_or_else(Style::default);
        lines.push(vec![StyledSpan {
            content: String::new(),
            style,
        }]);
    }

    if lines.is_empty() {
        lines.push(vec![StyledSpan {
            content: String::new(),
            style: Style::default(),
        }]);
    }

    lines
}

fn take_prefix_by_width(s: &str, max_width: usize) -> (usize, usize) {
    if max_width == 0 {
        return (0, 0);
    }
    let mut accum_width = 0usize;
    let mut byte_idx = 0usize;
    for (idx, ch) in s.char_indices() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if ch_width == 0 && accum_width == 0 {
            let len = ch.len_utf8();
            return (len, 0);
        }
        if accum_width + ch_width > max_width {
            if accum_width == 0 {
                let len = ch.len_utf8();
                return (len, ch_width);
            }
            break;
        }
        accum_width += ch_width;
        byte_idx = idx + ch.len_utf8();
    }

    if byte_idx == 0 {
        byte_idx = s.len();
    }

    (byte_idx, accum_width)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_text(line: &StyledLine) -> String {
        line.iter().map(|s| s.content.as_str()).collect()
    }

    #[test]
    fn wraps_plain_text_by_width() {
        let base = Style::new().fg(Color::White);
        let lines = highlight_message_lines("abcdefghij", base, 3);
        assert_eq!(lines.len(), 4);
        assert!(lines.iter().all(|line| !line.is_empty()));
    }

    #[test]
    fn code_block_is_highlighted() {
        let base = Style::new().fg(Color::White);
        let content = "plain\n```rust\nlet x = 1;\n```\nplain again";
        let lines = highlight_message_lines(content, base, 200);
        let code_line = lines
            .iter()
            .find(|l| line_text(l).contains("let x = 1;"))
            .expect("code line present");
        assert!(
            code_line.iter().any(|s| s.style != base),
            "code line should be syntax highlighted"
        );
        let tail = lines
            .iter()
            .find(|l| line_text(l).contains("plain again"))
            .expect("tail line present");
        assert!(
            tail.iter().all(|s| s.style == base),
            "plain text after code should remain plain"
        );
    }

    #[test]
    fn inline_markdown_strips_markers() {
        let base = Style::new().fg(Color::White);
        let content = "**bold** and `code` text";
        let lines = highlight_message_lines(content, base, 200);
        let joined: String = lines
            .iter()
            .flat_map(|l| l.iter().map(|s| s.content.as_str()))
            .collect();
        assert!(
            !joined.contains('*') && !joined.contains('`'),
            "markdown markers should be stripped"
        );
        let has_bold = lines
            .iter()
            .flat_map(|l| l.iter())
            .any(|s| s.content.contains("bold") && s.style != base);
        let has_code = lines
            .iter()
            .flat_map(|l| l.iter())
            .any(|s| s.content.contains("code") && s.style != base);
        assert!(has_bold && has_code);
    }

    #[test]
    fn inline_triple_backticks_remain_inline_code() {
        let base = Style::new().fg(Color::Indexed(1));
        let content = "Here ```not a block``` inline.";
        let lines = highlight_message_lines(content, base, 200);
        let inline_span = lines
            .iter()
            .flat_map(|l| l.iter())
            .find(|s| s.content.contains("not a block"))
            .expect("inline code span present");
        assert!(
            inline_span.style != base,
            "inline code should be styled differently from base"
        );
        let joined: String = lines
            .iter()
            .flat_map(|l| l.iter().map(|s| s.content.as_str()))
            .collect();
        assert!(
            !joined.contains("```"),
            "inline triple backticks should not render as markers"
        );
    }

    #[test]
    fn diff_coloring_applies_outside_code_only() {
        let base = Style::new().fg(Color::Indexed(2));
        let content = "```rust\n+inside\n```\n@@\n-outside";
        let lines = highlight_message_lines(content, base, 200);
        let inside = lines
            .iter()
            .find(|l| line_text(l).contains("+inside"))
            .expect("inside line present");
        assert!(
            inside
                .iter()
                .all(|s| s.style != Style::new().fg(DIFF_ADDITION)),
            "inside code block should not get diff +/- styling"
        );
        let outside = lines
            .iter()
            .find(|l| line_text(l).contains("-outside"))
            .expect("outside line present");
        assert_eq!(
            outside[0].style.fg,
            Some(DIFF_REMOVAL),
            "outside diff marker should be colored"
        );
    }

    #[test]
    fn unclosed_code_block_highlights_until_end() {
        let base = Style::new().fg(Color::Indexed(3));
        let content = "```rust\nlet z = 9;";
        let lines = highlight_message_lines(content, base, 200);
        assert!(
            lines.iter().any(|l| l.iter().any(|s| s.style != base)),
            "unterminated block should keep highlighting"
        );
    }

    #[test]
    fn bullet_lists_are_not_colored_like_diffs() {
        let base = Style::new().fg(Color::White);
        let content = "- item one\n- item two";
        let lines = highlight_message_lines(content, base, 40);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0][0].style, base);
        assert_eq!(lines[1][0].style, base);
    }

    #[test]
    fn headings_and_text_stay_plain_between_blocks() {
        let base = Style::new().fg(Color::Indexed(4));
        let content = "\
**1.**\n```\nblock one\n```\nplain between\n```rust\nlet x = 1;\n```\nplain tail";
        let lines = highlight_message_lines(content, base, 200);
        let between = lines
            .iter()
            .find(|l| line_text(l).contains("plain between"))
            .expect("between line present");
        assert!(
            between.iter().all(|s| s.style == base),
            "plain text between code blocks should stay plain"
        );
        let tail = lines
            .iter()
            .find(|l| line_text(l).contains("plain tail"))
            .expect("tail line present");
        assert!(
            tail.iter().all(|s| s.style == base),
            "plain text after code blocks should stay plain"
        );
    }

    #[test]
    fn llm_table_rendering_separates_cells() {
        let content = r#"---------- Reasoning ----------
 
                    We need to answer user. They ask: "Hello, can you see any code snippets? We are testing the chat interface I'm using to chat with you. Do you see any tool calls available?" We should respond acknowledging that there are many tool definitions available, list them perhaps. Provide that we can call them. Also mention code snippets? Probably say yes, there are many functions defined.

                                        
                    Yes – the repository contains a number of **tool definitions** that you (or I) can invoke from the chat.  Here’s a quick rundown of the available tool calls:

| Tool | Purpose | Key Parameters |
|------|---------|-----------------|
| `request_code_context` | Ask the repository for additional code snippets up to a token budget. | `search_term` (optional string), `token_budget` (optional integer) |
| `apply_code_edit` | Apply canonical edits to one or more Rust nodes identified by their canonical path. | `edits` array – each with `canon`, `code`, `file`, `node_type`; optional `confidence` |
| `create_file` | Atomically create a new Rust source file (staged for approval). | `file_path`, `content`; optional `create_parents`, `on_exists` |
| `non_semantic_patch` | Apply raw unified‑diff patches (useful for non‑Rust files or when the semantic parser fails). | `patches` array – each with `diff`, `file`, `reasoning`; optional `confidence` |
| `read_file` | Read a file (with optional line‑range and byte limits). | `file`, optional `start_line`, `end_line`, `max_bytes` |
| `code_item_lookup` | Look up the definition of a known code item (better than grep). | `file_path`, `item_name`, `module_path`, `node_kind` |
| `code_item_edges` | Show all graph edges for a target item (discover nearby code items). | Same args as `code_item_lookup` |
| `cargo` | Run `cargo check` or `cargo test` with JSON diagnostics. | `command` (`"check"` or `"test"`), many optional flags (`all_features`, `bins`, `features`, `release`, etc.) |

All of these tools are defined in the `functions` namespace and can be called directly from the chat by providing a JSON payload matching the parameter schema shown above. If you’d like me to invoke any of them (e.g., read a file, run a cargo command, or request code context), just let me know what you need!"#;
        let base = Style::new().fg(Color::Indexed(5));
        let lines = highlight_message_lines(content, base, 200);
        let rendered: Vec<String> = lines.iter().map(line_text).collect();

        fn has_spaced_columns(line: &str, cols: [&str; 3]) -> bool {
            let Some(first) = line.find(cols[0]) else {
                return false;
            };
            let rest = &line[first + cols[0].len()..];
            let Some(second_rel) = rest.find(cols[1]) else {
                return false;
            };
            let second = first + cols[0].len() + second_rel;
            let rest = &line[second + cols[1].len()..];
            let Some(third_rel) = rest.find(cols[2]) else {
                return false;
            };
            let third = second + cols[1].len() + third_rel;
            let between_1 = &line[first + cols[0].len()..second];
            let between_2 = &line[second + cols[1].len()..third];
            between_1.chars().all(|c| c == ' ')
                && between_1.len() >= 2
                && between_2.chars().all(|c| c == ' ')
                && between_2.len() >= 2
        }

        let header_ok = rendered
            .iter()
            .any(|line| has_spaced_columns(line, ["Tool", "Purpose", "Key Parameters"]));
        if !header_ok {
            for line in rendered.iter().filter(|line| line.contains("Tool")) {
                eprintln!("header line: {line}");
            }
        }
        assert!(header_ok, "table header should separate cells with spacing");

        let row_ok = rendered.iter().any(|line| {
            has_spaced_columns(
                line,
                [
                    "request_code_context",
                    "Ask the repository for additional code snippets up to a token budget.",
                    "search_term",
                ],
            )
        });
        assert!(row_ok, "table rows should separate cells with spacing");
    }

    #[test]
    fn table_multiline_cell_breaks_to_space() {
        let content = "\
| Col1 | Notes |
| ---- | ----- |
| A | line1<br>line2 |";
        let base = Style::new().fg(Color::Indexed(6));
        let lines = highlight_message_lines(content, base, 200);
        let rendered: Vec<String> = lines.iter().map(line_text).collect();
        let row_ok = rendered.iter().any(|line| line.contains("line1 line2"));
        assert!(row_ok, "line breaks in table cells should render as spaces");
    }

    #[test]
    fn table_with_many_columns_keeps_spacing() {
        let content = "\
| C1 | C2 | C3 | C4 | C5 |
| -- | -- | -- | -- | -- |
| a | b | c | d | e |";
        let base = Style::new().fg(Color::Indexed(7));
        let lines = highlight_message_lines(content, base, 200);
        let rendered: Vec<String> = lines.iter().map(line_text).collect();
        let wide_ok = rendered.iter().any(|line| {
            line.contains("C1") && line.contains("C2") && line.contains("C3") && line.contains("C4")
        });
        assert!(wide_ok, "wide tables should render a single aligned row");
    }

    #[test]
    fn table_with_empty_headers_and_body_renders() {
        let content = "\
| | | |
|---|---|---|
| a | b | c |";
        let base = Style::new().fg(Color::Indexed(8));
        let lines = highlight_message_lines(content, base, 200);
        let rendered: Vec<String> = lines.iter().map(line_text).collect();
        let row_ok = rendered
            .iter()
            .any(|line| line.contains("a") && line.contains("b"));
        assert!(
            row_ok,
            "tables with empty headers should still render body rows"
        );
    }

    #[test]
    fn table_header_separator_has_min_dash_width() {
        let content = "\
| A | B |
| - | - |
| x | y |";
        let base = Style::new().fg(Color::Indexed(9));
        let lines = highlight_message_lines(content, base, 200);
        let rendered: Vec<String> = lines.iter().map(line_text).collect();
        let separator = rendered
            .iter()
            .find(|line| line.chars().all(|c| c == '-' || c == ' '))
            .expect("separator line");
        for segment in separator.split("  ") {
            if segment.trim().is_empty() {
                continue;
            }
            assert!(
                segment.chars().all(|c| c == '-') && segment.len() >= 3,
                "separator segments should be at least 3 dashes"
            );
        }
    }

    #[test]
    fn table_cell_with_raw_pipe_stays_in_cell() {
        let content = "\
| Key | Value |
| --- | ----- |
| pipe | a\\|b |";
        let base = Style::new().fg(Color::Indexed(10));
        let lines = highlight_message_lines(content, base, 200);
        let rendered: Vec<String> = lines.iter().map(line_text).collect();
        let pipe_ok = rendered.iter().any(|line| line.contains("a|b"));
        assert!(pipe_ok, "escaped pipes should render in the cell content");
    }

    #[test]
    fn table_with_missing_cells_does_not_break() {
        let content = "\
| C1 | C2 | C3 |
| -- | -- | -- |
| only1 | only2 |";
        let base = Style::new().fg(Color::Indexed(11));
        let lines = highlight_message_lines(content, base, 200);
        let rendered: Vec<String> = lines.iter().map(line_text).collect();
        let row_ok = rendered
            .iter()
            .any(|line| line.contains("only1") && line.contains("only2"));
        assert!(row_ok, "rows with fewer cells should still render");
    }

    #[test]
    fn table_second_header_like_row_does_not_add_separator() {
        let content = "\
| H1 | H2 |
| --- | --- |
| **H1** | **H2** |
| a | b |";
        let base = Style::new().fg(Color::Indexed(12));
        let lines = highlight_message_lines(content, base, 200);
        let rendered: Vec<String> = lines.iter().map(line_text).collect();
        let separator_count = rendered
            .iter()
            .filter(|line| line.chars().all(|c| c == '-' || c == ' '))
            .count();
        assert_eq!(
            separator_count, 1,
            "only one header separator should render"
        );
    }

    #[test]
    fn table_text_after_table_is_separate_line() {
        let content = "\
| A | B |
| --- | --- |
| x | y |
Trailing text.";
        let base = Style::new().fg(Color::Indexed(13));
        let lines = highlight_message_lines(content, base, 200);
        let rendered: Vec<String> = lines.iter().map(line_text).collect();
        let tail_ok = rendered.iter().any(|line| line.contains("Trailing text."));
        assert!(tail_ok, "text following a table should render separately");
    }

    #[test]
    fn table_with_extra_cells_ignores_overflow() {
        let content = "\
| C1 | C2 |
| --- | --- |
| a | b | c | d |";
        let base = Style::new().fg(Color::Indexed(14));
        let lines = highlight_message_lines(content, base, 200);
        let rendered: Vec<String> = lines.iter().map(line_text).collect();
        let row_ok = rendered
            .iter()
            .any(|line| line.contains("a") && line.contains("b"));
        assert!(row_ok, "rows with extra cells should still render");
    }

    #[test]
    fn table_cells_keep_inline_styles() {
        let content = "\
| Name | Detail |
| ---- | ------ |
| **Bold** | `code` |";
        let base = Style::new().fg(Color::Indexed(15));
        let lines = highlight_message_lines(content, base, 200);
        let mut bold_ok = false;
        let mut code_ok = false;
        for line in &lines {
            for span in line {
                if span.content.contains("Bold") && span.style != base {
                    bold_ok = true;
                }
                if span.content.contains("code") && span.style != base {
                    code_ok = true;
                }
            }
        }
        assert!(
            bold_ok && code_ok,
            "inline styles should apply inside table cells"
        );
    }
}
