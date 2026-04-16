use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::fmt::Write as _;

use crossterm::style::{Attribute, Color, Stylize};
use serde::{Deserialize, Serialize};

pub const DEFAULT_PROTOCOL_REPORT_WIDTH: usize = 100;
pub const DEFAULT_TOP_CALL_ISSUES: usize = 8;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProtocolAggregateReport {
    pub run_id: String,
    pub subject_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<String>,
    #[serde(default)]
    pub coverage: ProtocolAggregateCoverage,
    #[serde(default)]
    pub segments: Vec<ProtocolAggregateSegmentRow>,
    #[serde(default)]
    pub call_issues: Vec<ProtocolAggregateCallIssueRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProtocolAggregateCoverage {
    pub total_tool_calls: usize,
    pub reviewed_tool_calls: usize,
    pub total_segments: usize,
    pub usable_segment_reviews: usize,
    pub mismatched_segment_reviews: usize,
    #[serde(default)]
    pub missing_tool_call_indices: Vec<usize>,
    #[serde(default)]
    pub missing_segment_indices: Vec<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duplicate_artifacts: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProtocolAggregateSegmentRow {
    pub index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_span: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub call_refs: Vec<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProtocolAggregateCallIssueRow {
    pub index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overall: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
pub struct ProtocolReportRenderOptions {
    pub width: usize,
    pub use_color: bool,
    pub top_call_issues: usize,
    pub color_profile: ProtocolColorProfile,
}

impl Default for ProtocolReportRenderOptions {
    fn default() -> Self {
        Self {
            width: DEFAULT_PROTOCOL_REPORT_WIDTH,
            use_color: true,
            top_call_issues: DEFAULT_TOP_CALL_ISSUES,
            color_profile: ProtocolColorProfile::TokioNight,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolColorProfile {
    TokioNight,
    Gruvbox,
    MonoDark,
}

#[derive(Debug, Clone, Copy)]
struct ProtocolPalette {
    heading: Color,
    accent: Color,
    usable: Color,
    mismatched: Color,
    missing: Color,
    disagreement: Color,
}

pub fn render_protocol_aggregate_report_with_options(
    report: &ProtocolAggregateReport,
    options: ProtocolReportRenderOptions,
) -> String {
    let width = options.width.max(72).min(120);
    let use_color = options.use_color;
    let palette = palette_for(options.color_profile);
    let mut out = String::new();

    render_header(&mut out, report, width, use_color, palette);
    render_reliability_panel(&mut out, report, width, use_color, palette);
    render_issue_overview(&mut out, report, width, use_color, palette);
    render_segment_table(&mut out, report, width, use_color, palette);
    render_call_issue_table(
        &mut out,
        report,
        width,
        use_color,
        palette,
        options.top_call_issues,
    );

    out
}

fn render_header(
    out: &mut String,
    report: &ProtocolAggregateReport,
    width: usize,
    use_color: bool,
    palette: ProtocolPalette,
) {
    let title = report
        .title
        .as_deref()
        .unwrap_or("Protocol aggregate report");
    let title = paint(
        &pad_to_width(title, width.saturating_sub(4)),
        palette.heading,
        true,
        use_color,
    );
    let rule = "─".repeat(width.saturating_sub(2));
    let _ = writeln!(out, "┌{}┐", rule);
    let _ = writeln!(out, "│ {} │", title);
    let _ = writeln!(out, "├{}┤", rule);

    let mut lines = Vec::new();
    lines.push(format!("Run: {}", report.run_id));
    lines.push(format!("Subject: {}", report.subject_id));
    if let Some(scope) = report.scope.as_deref() {
        lines.push(format!("Scope: {}", scope));
    }
    if !report.provenance.is_empty() {
        for line in &report.provenance {
            lines.push(format!("Provenance: {}", line));
        }
    } else if let Some(generated_at) = report.generated_at.as_deref() {
        lines.push(format!("Generated: {}", generated_at));
    }
    for line in lines {
        let _ = writeln!(
            out,
            "│ {} │",
            pad_to_width(
                &truncate(&line, width.saturating_sub(4)),
                width.saturating_sub(4)
            )
        );
    }
    if !report.notes.is_empty() {
        let joined = report
            .notes
            .iter()
            .map(|note| format!("• {}", note))
            .collect::<Vec<_>>()
            .join("  ");
        let _ = writeln!(
            out,
            "│ {} │",
            pad_to_width(
                &truncate(&joined, width.saturating_sub(4)),
                width.saturating_sub(4)
            )
        );
    }
    let _ = writeln!(out, "└{}┘\n", rule);
}

fn render_reliability_panel(
    out: &mut String,
    report: &ProtocolAggregateReport,
    width: usize,
    use_color: bool,
    palette: ProtocolPalette,
) {
    let coverage = &report.coverage;
    let section = paint("Evidence reliability", palette.heading, true, use_color);
    let _ = writeln!(out, "{}\n", section);

    let call_ratio = ratio(coverage.reviewed_tool_calls, coverage.total_tool_calls);
    let usable_ratio = ratio(coverage.usable_segment_reviews, coverage.total_segments);
    let missing_segment_count = coverage.missing_segment_indices.len();
    let call_line = format!(
        "Call reviews        {:>4}/{:<4} {} {:>6}  missing: {}",
        coverage.reviewed_tool_calls,
        coverage.total_tool_calls,
        progress_bar(call_ratio, 12, Some(palette.accent), use_color),
        fmt_pct(call_ratio),
        render_index_list(&coverage.missing_tool_call_indices),
    );
    let usable_line = format!(
        "Usable seg reviews  {:>4}/{:<4} {} {:>6}",
        coverage.usable_segment_reviews,
        coverage.total_segments,
        progress_bar(usable_ratio, 12, Some(palette.usable), use_color),
        fmt_pct(usable_ratio),
    );
    let segment_line = format!(
        "Segment evidence    usable {:>3}  mismatch {:>3}  missing {:>3}",
        coverage.usable_segment_reviews, coverage.mismatched_segment_reviews, missing_segment_count,
    );
    let segment_bar = format!(
        "Evidence mix        {}",
        segmented_status_bar(
            coverage.usable_segment_reviews,
            coverage.mismatched_segment_reviews,
            missing_segment_count,
            28,
            palette,
            use_color,
        )
    );
    let artifact_line = format!(
        "Artifact notes      duplicates: {}  missing segment ids: {}",
        coverage
            .duplicate_artifacts
            .map(|n| n.to_string())
            .unwrap_or_else(|| "-".to_string())
            .as_str(),
        render_index_list(&coverage.missing_segment_indices),
    );

    let _ = writeln!(out, "{call_line}");
    let _ = writeln!(out, "{usable_line}");
    let _ = writeln!(out, "{segment_line}");
    let _ = writeln!(out, "{segment_bar}");
    let _ = writeln!(out, "{artifact_line}\n");
    let _ = writeln!(out, "{}\n", "─".repeat(width.saturating_sub(2)));
}

fn render_issue_overview(
    out: &mut String,
    report: &ProtocolAggregateReport,
    width: usize,
    use_color: bool,
    palette: ProtocolPalette,
) {
    let _ = writeln!(
        out,
        "{}",
        paint("Issue surface", palette.heading, true, use_color)
    );

    if report.call_issues.is_empty() {
        let _ = writeln!(out, "(no call issues recorded)\n");
        return;
    }

    let issue_counts = group_counts(
        report
            .call_issues
            .iter()
            .filter_map(|row| row.issue.as_deref())
            .map(str::to_string),
    );
    let tool_counts = group_counts(
        report
            .call_issues
            .iter()
            .filter_map(|row| row.tool_name.as_deref())
            .map(str::to_string),
    );

    if !issue_counts.is_empty() {
        let _ = writeln!(out, "Issue kinds");
        render_count_chart(out, &issue_counts, width, palette.disagreement, use_color);
        let _ = writeln!(out);
    }

    if !tool_counts.is_empty() {
        let _ = writeln!(out, "Issue tools");
        render_count_chart(out, &tool_counts, width, palette.accent, use_color);
        let _ = writeln!(out);
    }

    let _ = writeln!(out, "{}\n", "─".repeat(width.saturating_sub(2)));
}

fn render_segment_table(
    out: &mut String,
    report: &ProtocolAggregateReport,
    width: usize,
    use_color: bool,
    palette: ProtocolPalette,
) {
    let _ = writeln!(
        out,
        "{}",
        paint("Segment evidence", palette.heading, true, use_color)
    );
    let _ = writeln!(
        out,
        "{:<4} │ {:<11} │ {:<18} │ {:<14} │ {:<10} │ {:<8} │ {}",
        "Idx", "Span", "Intent", "Status", "Evidence", "Refs", "Note"
    );
    let _ = writeln!(out, "{}", "─".repeat(width.saturating_sub(2)));

    let mut rows = report.segments.clone();
    rows.sort_by_key(|row| row.index);
    if rows.is_empty() {
        let _ = writeln!(out, "(no segment rows selected)\n");
        return;
    }
    for row in rows {
        let span = row.call_span.as_deref().unwrap_or("-");
        let label = row.label.as_deref().unwrap_or("-");
        let confidence = row.confidence.map(fmt_pct);
        let refs = if row.call_refs.is_empty() {
            "-".to_string()
        } else {
            join_indices(&row.call_refs)
        };
        let note = row
            .note
            .as_deref()
            .map(|status| truncate(status, width.saturating_sub(84)))
            .unwrap_or_else(|| confidence.unwrap_or_else(|| "-".to_string()));
        let evidence = row.evidence.as_deref().unwrap_or("-");
        let _ = writeln!(
            out,
            "{:<4} │ {:<11} │ {:<18} │ {:<14} │ {:<10} │ {:<8} │ {}",
            row.index,
            truncate(span, 11),
            truncate(label, 18),
            truncate(row.status.as_deref().unwrap_or("-"), 14),
            truncate(evidence, 10),
            truncate(&refs, 8),
            truncate(&note, width.saturating_sub(84)),
        );
    }
    let _ = writeln!(out);
}

fn render_call_issue_table(
    out: &mut String,
    report: &ProtocolAggregateReport,
    width: usize,
    use_color: bool,
    palette: ProtocolPalette,
    top_limit: usize,
) {
    let _ = writeln!(
        out,
        "{}",
        paint("Top call issues", palette.heading, true, use_color)
    );
    let _ = writeln!(
        out,
        "{:<4} │ {:<6} │ {:<16} │ {:<18} │ {:<10} │ {:<8} │ {}",
        "Idx", "Turn", "Tool", "Issue", "Severity", "Segment", "Detail"
    );
    let _ = writeln!(out, "{}", "─".repeat(width.saturating_sub(2)));

    let mut issues = report.call_issues.clone();
    issues.sort_by_key(|row| (Reverse(severity_bucket(row.severity)), Reverse(row.index)));
    issues.truncate(top_limit);

    if issues.is_empty() {
        let _ = writeln!(out, "(no call issues recorded)\n");
        return;
    }

    for row in issues {
        let turn = row
            .turn
            .map(|turn| turn.to_string())
            .unwrap_or_else(|| "-".to_string());
        let tool = row.tool_name.as_deref().unwrap_or("-");
        let issue = row.issue.as_deref().unwrap_or("-");
        let detail = row.detail.as_deref().unwrap_or("-");
        let seg = row
            .segment_index
            .map(|idx| idx.to_string())
            .unwrap_or_else(|| "-".to_string());
        let sev = row
            .severity
            .map(|sev| {
                let glyph = if sev >= 0.85 {
                    "████"
                } else if sev >= 0.65 {
                    "███░"
                } else if sev >= 0.35 {
                    "██░░"
                } else if sev > 0.0 {
                    "█░░░"
                } else {
                    "░░░░"
                };
                format!("{glyph} {:>3.0}%", sev * 100.0)
            })
            .unwrap_or_else(|| "-".to_string());
        let _ = writeln!(
            out,
            "{:<4} │ {:<6} │ {:<16} │ {:<18} │ {:<10} │ {:<8} │ {}",
            row.index,
            truncate(&turn, 6),
            truncate(tool, 16),
            truncate(issue, 22),
            truncate(&sev, 10),
            truncate(&seg, 8),
            truncate(detail, width.saturating_sub(84)),
        );
    }
    let _ = writeln!(out);
}

fn ratio(numerator: usize, denominator: usize) -> f32 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f32 / denominator as f32
    }
}

fn fmt_pct(value: f32) -> String {
    format!("{:>5.1}%", (value.clamp(0.0, 1.0) * 100.0))
}

fn progress_bar(value: f32, width: usize, color: Option<Color>, use_color: bool) -> String {
    let width = width.max(3);
    let filled = ((value.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
    let mut out = String::with_capacity(width + 2);
    out.push('[');
    let filled_text = "█".repeat(filled);
    if let Some(color) = color {
        out.push_str(&paint(&filled_text, color, false, use_color));
    } else {
        out.push_str(&filled_text);
    }
    out.push_str(&"░".repeat(width - filled));
    out.push(']');
    out
}

fn severity_bucket(severity: Option<f32>) -> usize {
    let sev = severity.unwrap_or(0.0).clamp(0.0, 1.0);
    if sev >= 0.85 {
        4
    } else if sev >= 0.65 {
        3
    } else if sev >= 0.35 {
        2
    } else if sev > 0.0 {
        1
    } else {
        0
    }
}

fn segmented_status_bar(
    usable: usize,
    mismatched: usize,
    missing: usize,
    width: usize,
    palette: ProtocolPalette,
    use_color: bool,
) -> String {
    let total = usable + mismatched + missing;
    if total == 0 {
        return "[no segment evidence]".to_string();
    }

    let width = width.max(6);
    let usable_width = ((usable as f32 / total as f32) * width as f32).round() as usize;
    let mismatch_width = ((mismatched as f32 / total as f32) * width as f32).round() as usize;
    let missing_width = width.saturating_sub(usable_width + mismatch_width);

    let mut out = String::with_capacity(width + 32);
    out.push('[');
    out.push_str(&paint(
        &"█".repeat(usable_width),
        palette.usable,
        false,
        use_color,
    ));
    out.push_str(&paint(
        &"▓".repeat(mismatch_width),
        palette.mismatched,
        false,
        use_color,
    ));
    out.push_str(&paint(
        &"░".repeat(missing_width),
        palette.missing,
        false,
        use_color,
    ));
    out.push(']');
    let _ = write!(
        out,
        "  usable={} mismatch={} missing={}",
        usable, mismatched, missing
    );
    out
}

fn group_counts(values: impl IntoIterator<Item = String>) -> Vec<(String, usize)> {
    let mut counts = BTreeMap::<String, usize>::new();
    for value in values {
        *counts.entry(value).or_default() += 1;
    }
    let mut rows = counts.into_iter().collect::<Vec<_>>();
    rows.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    rows
}

fn render_count_chart(
    out: &mut String,
    rows: &[(String, usize)],
    width: usize,
    color: Color,
    use_color: bool,
) {
    let chart_width = 26usize;
    let label_width = 20usize.min(width.saturating_sub(chart_width + 10));
    let max = rows
        .iter()
        .map(|(_, count)| *count)
        .max()
        .unwrap_or(1)
        .max(1);
    for (label, count) in rows.iter().take(5) {
        let ratio = *count as f32 / max as f32;
        let bar_len = (ratio * chart_width as f32).round() as usize;
        let (bar, padding) = if bar_len == 0 {
            ("░".to_string(), " ".repeat(chart_width.saturating_sub(1)))
        } else {
            (
                paint(&"█".repeat(bar_len), color, false, use_color),
                " ".repeat(chart_width.saturating_sub(bar_len)),
            )
        };
        let _ = writeln!(
            out,
            "  {:<label_width$} {}{} {}",
            truncate(label, label_width),
            bar,
            padding,
            count,
            label_width = label_width,
        );
    }
}

fn render_index_list(indices: &[usize]) -> String {
    if indices.is_empty() {
        "none".to_string()
    } else {
        join_indices(indices)
    }
}

fn join_indices(indices: &[usize]) -> String {
    indices
        .iter()
        .map(|idx| idx.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn truncate(text: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }
    let trimmed = text.trim();
    let char_count = trimmed.chars().count();
    if char_count <= max_len {
        return trimmed.to_string();
    }
    if max_len <= 3 {
        return ".".repeat(max_len);
    }
    let keep = max_len - 3;
    let head = keep / 2;
    let tail = keep - head;
    let prefix: String = trimmed.chars().take(head).collect();
    let suffix: String = trimmed
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{prefix}...{suffix}")
}

fn pad_to_width(text: &str, width: usize) -> String {
    let visible = text.chars().count();
    if visible >= width {
        return text.to_string();
    }
    let mut out = String::with_capacity(width);
    out.push_str(text);
    out.push_str(&" ".repeat(width - visible));
    out
}

fn paint(text: &str, color: Color, bold: bool, use_color: bool) -> String {
    if !use_color {
        return text.to_string();
    }
    let mut styled = text.with(color);
    if bold {
        styled = styled.attribute(Attribute::Bold);
    }
    styled.to_string()
}

fn palette_for(profile: ProtocolColorProfile) -> ProtocolPalette {
    match profile {
        ProtocolColorProfile::TokioNight => ProtocolPalette {
            heading: Color::Rgb {
                r: 122,
                g: 162,
                b: 247,
            },
            accent: Color::Rgb {
                r: 125,
                g: 207,
                b: 255,
            },
            usable: Color::Rgb {
                r: 115,
                g: 218,
                b: 202,
            },
            mismatched: Color::Rgb {
                r: 224,
                g: 175,
                b: 104,
            },
            missing: Color::Rgb {
                r: 247,
                g: 118,
                b: 142,
            },
            disagreement: Color::Rgb {
                r: 187,
                g: 154,
                b: 247,
            },
        },
        ProtocolColorProfile::Gruvbox => ProtocolPalette {
            heading: Color::Rgb {
                r: 131,
                g: 165,
                b: 152,
            },
            accent: Color::Rgb {
                r: 235,
                g: 219,
                b: 178,
            },
            usable: Color::Rgb {
                r: 142,
                g: 192,
                b: 124,
            },
            mismatched: Color::Rgb {
                r: 250,
                g: 189,
                b: 47,
            },
            missing: Color::Rgb {
                r: 251,
                g: 73,
                b: 52,
            },
            disagreement: Color::Rgb {
                r: 211,
                g: 134,
                b: 155,
            },
        },
        ProtocolColorProfile::MonoDark => ProtocolPalette {
            heading: Color::Rgb {
                r: 156,
                g: 163,
                b: 175,
            },
            accent: Color::Rgb {
                r: 96,
                g: 165,
                b: 250,
            },
            usable: Color::Rgb {
                r: 132,
                g: 204,
                b: 22,
            },
            mismatched: Color::Rgb {
                r: 245,
                g: 158,
                b: 11,
            },
            missing: Color::Rgb {
                r: 239,
                g: 68,
                b: 68,
            },
            disagreement: Color::Rgb {
                r: 167,
                g: 139,
                b: 250,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_renders_sections() {
        let report = ProtocolAggregateReport {
            run_id: "tokio-rs__tokio-0000".to_string(),
            subject_id: "tokio-rs__tokio-0000".to_string(),
            title: Some("Protocol aggregate report".to_string()),
            generated_at: Some("2026-04-15T12:34:56Z".to_string()),
            scope: Some("single run".to_string()),
            provenance: vec!["anchor=1776271451854".to_string()],
            coverage: ProtocolAggregateCoverage {
                total_tool_calls: 4,
                reviewed_tool_calls: 3,
                total_segments: 2,
                usable_segment_reviews: 1,
                mismatched_segment_reviews: 1,
                missing_tool_call_indices: vec![3],
                missing_segment_indices: vec![1],
                duplicate_artifacts: Some(0),
            },
            segments: vec![ProtocolAggregateSegmentRow {
                index: 0,
                label: Some("segment-a".to_string()),
                call_span: Some("0..2".to_string()),
                status: Some("focused".to_string()),
                evidence: Some("usable".to_string()),
                confidence: Some(0.7),
                note: Some("good".to_string()),
                call_refs: vec![0, 1],
            }],
            call_issues: vec![ProtocolAggregateCallIssueRow {
                index: 0,
                turn: Some(1),
                segment_index: Some(0),
                tool_name: Some("search_code".to_string()),
                issue: Some("retry loop".to_string()),
                overall: Some("mixed".to_string()),
                detail: Some("needs follow-up".to_string()),
                severity: Some(0.9),
                confidence: Some(0.8),
            }],
            notes: vec!["compact single-run surface".to_string()],
        };

        let rendered = render_protocol_aggregate_report_with_options(
            &report,
            ProtocolReportRenderOptions {
                width: 100,
                use_color: false,
                top_call_issues: 8,
            },
        );

        assert!(rendered.contains("Evidence reliability"));
        assert!(rendered.contains("Segment evidence"));
        assert!(rendered.contains("Top call issues"));
    }
}
