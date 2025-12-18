use once_cell::sync::Lazy;
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
    let mut blocks: Vec<StyledLine> = Vec::new();
    let mut pending_text: Vec<String> = Vec::new();
    let mut code_lines: Vec<String> = Vec::new();
    let mut in_code = false;
    let mut fence_len: usize = 0;
    let mut lang_hint: Option<String> = None;
    let diff_enabled = detect_diff_markers(content);

    let flush_text = |lines: &mut Vec<String>, out: &mut Vec<StyledLine>| {
        for line in lines.drain(..) {
            out.push(plain_text_line(&line, base_style, diff_enabled));
        }
    };

    let flush_code = |lang: &Option<String>, code: &mut Vec<String>, out: &mut Vec<StyledLine>| {
        if code.is_empty() {
            return;
        }
        let highlighted = if lang
            .as_deref()
            .map(|l| matches!(l, "diff" | "patch"))
            .unwrap_or(false)
        {
            code.iter()
                .map(|line| diff_line(line))
                .collect::<Vec<StyledLine>>()
        } else {
            highlight_code_block(code, lang.as_deref())
        };
        out.extend(highlighted);
        code.clear();
    };

    let lines: Vec<&str> = content.split('\n').collect();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let backticks = trimmed.chars().take_while(|c| *c == '`').count();
        let rest = trimmed.get(backticks..).unwrap_or_default().trim();

        if backticks >= 3 {
            if in_code {
                if rest.is_empty() && backticks >= fence_len {
                    let mut has_future_closer = false;
                    for future in lines.iter().skip(idx + 1) {
                        let ftrim = future.trim_start();
                        let fb = ftrim.chars().take_while(|c| *c == '`').count();
                        let frest = ftrim.get(fb..).unwrap_or_default().trim();
                        if fb >= 3 {
                            if frest.is_empty() && fb >= fence_len {
                                has_future_closer = true;
                            }
                        }
                    }
                    // Close only if this is the last closer.
                    if !has_future_closer {
                        flush_code(&lang_hint, &mut code_lines, &mut blocks);
                        in_code = false;
                        lang_hint = None;
                        fence_len = 0;
                        continue;
                    }
                }
            } else {
                flush_text(&mut pending_text, &mut blocks);
                in_code = true;
                fence_len = backticks;
                if !rest.is_empty() {
                    lang_hint = Some(rest.to_string());
                }
                continue;
            }
        }

        if in_code {
            code_lines.push(line.to_string());
        } else {
            pending_text.push(line.to_string());
        }
    }

    if in_code {
        flush_code(&lang_hint, &mut code_lines, &mut blocks);
    } else {
        flush_text(&mut pending_text, &mut blocks);
    }

    wrap_lines(blocks, width)
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
    let mut in_code = false;
    let mut fence_len: usize = 0;
    for line in content.lines() {
        let trimmed = line.trim_start();
        let backticks = trimmed.chars().take_while(|c| *c == '`').count();
        let rest = trimmed.get(backticks..).unwrap_or_default().trim();
        if backticks >= 3 {
            if in_code {
                if rest.is_empty() && backticks >= fence_len {
                    in_code = false;
                    fence_len = 0;
                }
            } else {
                in_code = true;
                fence_len = backticks;
            }
            continue;
        }
        if in_code {
            continue;
        }
        if line.starts_with("diff --")
            || line.starts_with("index ")
            || line.starts_with("---")
            || line.starts_with("+++")
            || line.starts_with("@@")
        {
            return true;
        }
    }
    // Allow +/- styling only if a hunk marker exists; bullets alone won't trigger.
    content.lines().any(|l| l.starts_with("@@"))
}

fn find_syntax(lang: &str) -> Option<&'static SyntaxReference> {
    let lang = lang.trim();
    if lang.is_empty() {
        return None;
    }
    if let Some(syntax) = SYNTAX_SET.find_syntax_by_token(lang) {
        return Some(syntax);
    }
    if let Some(syntax) = SYNTAX_SET.find_syntax_by_name(lang) {
        return Some(syntax);
    }
    SYNTAX_SET.find_syntax_by_extension(lang)
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

    #[test]
    fn wraps_plain_text_by_width() {
        let base = Style::new().fg(Color::White);
        let lines = highlight_message_lines("abcdefghij", base, 3);
        assert_eq!(lines.len(), 4);
        assert!(lines.iter().all(|line| !line.is_empty()));
    }

    #[test]
    fn detects_code_block_and_uses_syntect_styles() {
        let base = Style::new().fg(Color::White);
        let content = "plain\n```rust\nlet x = 1;\n```\n";
        let lines = highlight_message_lines(content, base, 40);
        assert!(
            lines
                .iter()
                .any(|line| { line.iter().any(|span| span.style != base) })
        );
    }

    #[test]
    fn diff_lines_get_custom_styles() {
        let base = Style::new().fg(Color::White);
        let content = "+added\n-context\n@@ hunk";
        let lines = highlight_message_lines(content, base, 40);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0][0].style.fg, Some(DIFF_ADDITION));
        assert_eq!(lines[1][0].style.fg, Some(DIFF_REMOVAL));
        assert_eq!(lines[2][0].style.fg, Some(DIFF_INFO));
    }

    #[test]
    fn plain_bullet_lists_are_not_colored_like_diffs() {
        let base = Style::new().fg(Color::White);
        let content = "- item one\n- item two";
        let lines = highlight_message_lines(content, base, 40);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0][0].style, base);
        assert_eq!(lines[1][0].style, base);
    }

    #[test]
    fn inner_fences_do_not_terminate_code_block() {
        let base = Style::new().fg(Color::Indexed(1));
        let content = "\
```rust
// newline immediately after opening fence
```rust
let a = 1;

// newline *before* the fence closer
let b = 2;
```

// fence opener at EOF with no terminating newline
```rust
let c = 3;```
// back-tick alone on a line inside a string literal
let s = \"first
`
third\";

// raw string that ends with a single back-tick on its own line
let raw = r#\"hello
`\"#;

// literal back-tick inside an attribute that itself contains
// what looks like a code fence
#[cfg_attr(docsrs, doc = \"```\")]
```";

        let lines = highlight_message_lines(content, base, 200);

        let line_texts: Vec<String> = lines
            .iter()
            .map(|l| l.iter().map(|s| s.content.as_str()).collect::<String>())
            .collect();

        for needle in [
            "// newline immediately after opening fence",
            "let a = 1;",
            "// newline *before* the fence closer",
            "let b = 2;",
            "// fence opener at EOF with no terminating newline",
            "let c = 3;```",
            "// back-tick alone on a line inside a string literal",
            "`",
            "third\";",
            "// raw string that ends with a single back-tick on its own line",
            "`\"#;",
            "// literal back-tick inside an attribute that itself contains",
            "#[cfg_attr(docsrs, doc = \"```\")]",
        ] {
            let (idx, line) = line_texts
                .iter()
                .enumerate()
                .find(|(_, l)| l.contains(needle))
                .unwrap_or_else(|| panic!("expected to find line containing {needle}"));
            assert!(
                lines[idx].iter().any(|s| s.style != base),
                "line '{needle}' should remain inside highlighted code block"
            );
        }
    }
}
