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

    let push_newline =
        |current_line: &mut StyledLine, lines: &mut Vec<StyledLine>, diff_enabled: bool| {
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

    let parser = Parser::new_ext(content, Options::all());
    for event in parser {
        match (&mut mode, &event) {
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
            | (_, Event::Start(Tag::List(_)))
            | (_, Event::Start(Tag::FootnoteDefinition(_)))
            | (_, Event::Start(Tag::Table(_))) => {
                if !current_line.is_empty() {
                    push_newline(&mut current_line, &mut lines, diff_enabled);
                }
            }
            (_, Event::End(TagEnd::Paragraph))
            | (_, Event::End(TagEnd::Heading(..)))
            | (_, Event::End(TagEnd::BlockQuote))
            | (_, Event::End(TagEnd::List(_)))
            | (_, Event::End(TagEnd::FootnoteDefinition))
            | (_, Event::End(TagEnd::Table))
            | (_, Event::End(TagEnd::TableHead))
            | (_, Event::End(TagEnd::TableRow)) => {
                push_newline(&mut current_line, &mut lines, diff_enabled);
            }
            (_, Event::Text(t)) => {
                let style = inline_style(base_style, italic_depth, bold_depth);
                push_text(&t, style, &mut current_line, &mut lines, diff_enabled);
            }
            (_, Event::Code(t)) => {
                let style = inline_code_style(base_style);
                push_text(&t, style, &mut current_line, &mut lines, diff_enabled);
            }
            (_, Event::SoftBreak | Event::HardBreak) => {
                push_newline(&mut current_line, &mut lines, diff_enabled);
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
                push_text(&t, style, &mut current_line, &mut lines, diff_enabled);
            }
            (_, Event::FootnoteReference(t)) => {
                let style = inline_style(base_style, italic_depth, bold_depth);
                push_text(&t, style, &mut current_line, &mut lines, diff_enabled);
            }
            (_, Event::TaskListMarker(done)) => {
                let marker = if *done { "[x] " } else { "[ ] " };
                let style = inline_style(base_style, italic_depth, bold_depth);
                push_text(marker, style, &mut current_line, &mut lines, diff_enabled);
            }
            (_, Event::Rule) => {
                let style = inline_style(base_style, italic_depth, bold_depth);
                push_text("—", style, &mut current_line, &mut lines, diff_enabled);
                push_newline(&mut current_line, &mut lines, diff_enabled);
            }
            (_, Event::InlineMath(t)) | (_, Event::DisplayMath(t)) => {
                let style = inline_style(base_style, italic_depth, bold_depth);
                push_text(&t, style, &mut current_line, &mut lines, diff_enabled);
            }
            (_, Event::End(TagEnd::Item)) => {
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
    if diff_enabled {
        if let Some(style) = diff_style(line) {
            return vec![StyledSpan {
                content: line.to_string(),
                style,
            }];
        }
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
}
