use std::cmp::Reverse;
use std::fmt::Write as _;

use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProtocolCampaignTriageReport {
    pub campaign_id: String,
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue_filter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_filter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_filter: Option<String>,
    pub selected_runs: usize,
    pub campaign_runs: usize,
    pub summary: ProtocolCampaignSummary,
    pub evidence: ProtocolCampaignEvidence,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issue_kinds: Vec<ProtocolCampaignCountRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issue_tools: Vec<ProtocolCampaignCountRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nearby_segment_labels: Vec<ProtocolCampaignCountRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nearby_segment_statuses: Vec<ProtocolCampaignCountRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problem_families: Vec<ProtocolCampaignFamilyRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exemplars: Vec<ProtocolCampaignExemplarRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub next_steps: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProtocolCampaignSummary {
    pub eligible_runs: usize,
    pub full_runs: usize,
    pub partial_runs: usize,
    pub error_runs: usize,
    pub missing_runs: usize,
    pub ineligible_runs: usize,
    pub runs_with_issue_calls: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProtocolCampaignEvidence {
    pub total_tool_calls: usize,
    pub reviewed_tool_calls: usize,
    pub missing_tool_call_reviews: usize,
    pub known_segments: usize,
    pub usable_segment_reviews: usize,
    pub mismatched_segment_reviews: usize,
    pub missing_segment_reviews: usize,
    pub artifact_failure_runs: usize,
    pub duplicate_artifacts: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProtocolCampaignCountRow {
    pub label: String,
    pub count: usize,
    pub affected_runs: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProtocolCampaignFamilyRow {
    pub label: String,
    pub family_kind: String,
    pub affected_runs: usize,
    pub affected_calls: usize,
    pub likely_owner: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exemplar_run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success_metric: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProtocolCampaignExemplarRow {
    pub run_id: String,
    pub protocol_status: String,
    pub matching_calls: usize,
    pub total_issues: usize,
    pub tool_calls_total: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

pub fn render_protocol_campaign_triage_report(
    report: &ProtocolCampaignTriageReport,
    width: usize,
) -> String {
    let width = width.max(88).min(132);
    let mut out = String::new();

    render_header(&mut out, report, width);
    render_evidence_panel(&mut out, report, width);
    render_issue_surface(&mut out, report, width);
    render_family_context(&mut out, report, width);
    render_problem_families(&mut out, report, width);
    render_exemplars(&mut out, report, width);
    render_next_steps(&mut out, report, width);

    out
}

fn render_header(out: &mut String, report: &ProtocolCampaignTriageReport, width: usize) {
    let rule = "─".repeat(width.saturating_sub(2));
    let _ = writeln!(out, "┌{}┐", rule);
    let _ = writeln!(
        out,
        "│ {} │",
        pad_to_width("Campaign protocol triage", width.saturating_sub(4))
    );
    let _ = writeln!(out, "├{}┤", rule);
    let lines = [
        format!("Campaign: {}", report.campaign_id),
        format!("Scope: {}", report.scope),
        format!(
            "Selection: {} / {} protocol-tracked runs",
            report.selected_runs, report.campaign_runs
        ),
        format!(
            "Filters: issue={}  tool={}  status={}",
            report.issue_filter.as_deref().unwrap_or("none"),
            report.tool_filter.as_deref().unwrap_or("none"),
            report.status_filter.as_deref().unwrap_or("none")
        ),
    ];
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
    let _ = writeln!(out, "└{}┘\n", rule);
}

fn render_evidence_panel(out: &mut String, report: &ProtocolCampaignTriageReport, width: usize) {
    let summary = &report.summary;
    let evidence = &report.evidence;
    let _ = writeln!(out, "Campaign evidence reliability\n");
    let _ = writeln!(
        out,
        "Run status          eligible {:>3}  full {:>3}  partial {:>3}  error {:>3}  missing {:>3}  ineligible {:>3}",
        summary.eligible_runs,
        summary.full_runs,
        summary.partial_runs,
        summary.error_runs,
        summary.missing_runs,
        summary.ineligible_runs
    );
    let reviewed_ratio = ratio(evidence.reviewed_tool_calls, evidence.total_tool_calls);
    let usable_ratio = ratio(evidence.usable_segment_reviews, evidence.known_segments);
    let _ = writeln!(
        out,
        "Call reviews        {:>4}/{:<4} {} {:>6}  missing: {}",
        evidence.reviewed_tool_calls,
        evidence.total_tool_calls,
        progress_bar(reviewed_ratio, 14),
        fmt_pct(reviewed_ratio),
        evidence.missing_tool_call_reviews,
    );
    let _ = writeln!(
        out,
        "Usable seg reviews  {:>4}/{:<4} {} {:>6}",
        evidence.usable_segment_reviews,
        evidence.known_segments,
        progress_bar(usable_ratio, 14),
        fmt_pct(usable_ratio),
    );
    let _ = writeln!(
        out,
        "Segment evidence    usable {:>3}  mismatch {:>3}  missing {:>3}",
        evidence.usable_segment_reviews,
        evidence.mismatched_segment_reviews,
        evidence.missing_segment_reviews,
    );
    let _ = writeln!(
        out,
        "Artifact notes      schema/error runs {:>3}  duplicate artifacts {:>3}  runs with issue calls {:>3}\n",
        evidence.artifact_failure_runs, evidence.duplicate_artifacts, summary.runs_with_issue_calls,
    );
    let _ = writeln!(out, "{}\n", "─".repeat(width.saturating_sub(2)));
}

fn render_issue_surface(out: &mut String, report: &ProtocolCampaignTriageReport, width: usize) {
    let _ = writeln!(out, "Issue surface");
    if report.issue_kinds.is_empty() && report.issue_tools.is_empty() {
        let _ = writeln!(out, "(no matching issue evidence)\n");
        return;
    }
    if !report.issue_kinds.is_empty() {
        let _ = writeln!(out, "Issue kinds");
        render_count_chart(out, &report.issue_kinds, width);
        let _ = writeln!(out);
    }
    if !report.issue_tools.is_empty() {
        let _ = writeln!(out, "Issue tools");
        render_count_chart(out, &report.issue_tools, width);
        let _ = writeln!(out);
    }
    let _ = writeln!(out, "{}\n", "─".repeat(width.saturating_sub(2)));
}

fn render_family_context(out: &mut String, report: &ProtocolCampaignTriageReport, width: usize) {
    if report.nearby_segment_labels.is_empty() && report.nearby_segment_statuses.is_empty() {
        return;
    }
    let _ = writeln!(out, "Family context");
    if !report.nearby_segment_labels.is_empty() {
        let _ = writeln!(out, "Nearby segment labels");
        render_count_chart(out, &report.nearby_segment_labels, width);
        let _ = writeln!(out);
    }
    if !report.nearby_segment_statuses.is_empty() {
        let _ = writeln!(out, "Nearby segment statuses");
        render_count_chart(out, &report.nearby_segment_statuses, width);
        let _ = writeln!(out);
    }
    let _ = writeln!(out, "{}\n", "─".repeat(width.saturating_sub(2)));
}

fn render_problem_families(out: &mut String, report: &ProtocolCampaignTriageReport, width: usize) {
    let _ = writeln!(out, "Top problem families");
    if report.problem_families.is_empty() {
        let _ = writeln!(out, "(no grouped families for the current selection)\n");
        return;
    }
    let _ = writeln!(
        out,
        "{:<26} {:<10} {:<8} {:<18} {:<18} {}",
        "Family", "Runs", "Calls", "Owner", "Exemplar", "Why it matters"
    );
    let _ = writeln!(out, "{}", "─".repeat(width.saturating_sub(2)));
    for family in &report.problem_families {
        let exemplar = family.exemplar_run.as_deref().unwrap_or("-");
        let why = family
            .note
            .as_deref()
            .or(family.success_metric.as_deref())
            .unwrap_or("-");
        let _ = writeln!(
            out,
            "{:<26} {:<10} {:<8} {:<18} {:<18} {}",
            truncate(&family.label, 26),
            format!("{}/{}", family.affected_runs, report.selected_runs.max(1)),
            family.affected_calls,
            truncate(&family.likely_owner, 18),
            truncate(exemplar, 18),
            truncate(why, width.saturating_sub(86)),
        );
    }
    let _ = writeln!(out, "\n{}\n", "─".repeat(width.saturating_sub(2)));
}

fn render_exemplars(out: &mut String, report: &ProtocolCampaignTriageReport, width: usize) {
    let _ = writeln!(out, "Representative exemplars");
    if report.exemplars.is_empty() {
        let _ = writeln!(out, "(no exemplar runs for the current selection)\n");
        return;
    }
    let _ = writeln!(
        out,
        "{:<28} {:<10} {:<8} {:<8} {:<10} {}",
        "Run", "Status", "Match", "Issues", "Calls", "Focus"
    );
    let _ = writeln!(out, "{}", "─".repeat(width.saturating_sub(2)));
    for exemplar in &report.exemplars {
        let focus = exemplar
            .focus
            .as_deref()
            .or(exemplar.note.as_deref())
            .unwrap_or("-");
        let _ = writeln!(
            out,
            "{:<28} {:<10} {:<8} {:<8} {:<10} {}",
            truncate(&exemplar.run_id, 28),
            truncate(&exemplar.protocol_status, 10),
            exemplar.matching_calls,
            exemplar.total_issues,
            exemplar.tool_calls_total,
            truncate(focus, width.saturating_sub(72)),
        );
    }
    let _ = writeln!(out, "\n{}\n", "─".repeat(width.saturating_sub(2)));
}

fn render_next_steps(out: &mut String, report: &ProtocolCampaignTriageReport, width: usize) {
    let _ = writeln!(out, "Next moves");
    if report.next_steps.is_empty() {
        let _ = writeln!(out, "(no follow-up commands available)\n");
        return;
    }
    for step in &report.next_steps {
        let _ = writeln!(out, "  - {}", truncate(step, width.saturating_sub(4)));
    }
    let _ = writeln!(out);
}

fn render_count_chart(out: &mut String, rows: &[ProtocolCampaignCountRow], width: usize) {
    let top = rows.iter().map(|row| row.count).max().unwrap_or(1);
    let bar_width = width.saturating_sub(42).clamp(12, 28);
    for row in rows {
        let filled = if top == 0 {
            0
        } else {
            ((row.count as f32 / top as f32) * bar_width as f32).round() as usize
        }
        .min(bar_width);
        let _ = writeln!(
            out,
            "  {:<22} {} {:>4} calls  {:>3} runs",
            truncate(&row.label, 22),
            bar_from_width(filled, bar_width),
            row.count,
            row.affected_runs,
        );
    }
}

fn truncate(text: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_len {
        return text.to_string();
    }
    if max_len <= 3 {
        return ".".repeat(max_len);
    }
    chars[..max_len - 3].iter().collect::<String>() + "..."
}

fn pad_to_width(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if len >= width {
        truncate(text, width)
    } else {
        format!("{text}{}", " ".repeat(width - len))
    }
}

fn progress_bar(ratio: f32, width: usize) -> String {
    let width = width.max(4);
    let filled = ((ratio.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
    bar_from_width(filled, width)
}

fn bar_from_width(filled: usize, width: usize) -> String {
    format!(
        "[{}{}]",
        "█".repeat(filled),
        "░".repeat(width.saturating_sub(filled))
    )
}

fn fmt_pct(value: f32) -> String {
    format!("{:>5.1}%", value * 100.0)
}

fn ratio(numerator: usize, denominator: usize) -> f32 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f32 / denominator as f32
    }
}

pub fn sort_count_rows(rows: &mut [ProtocolCampaignCountRow]) {
    rows.sort_by_key(|row| {
        (
            Reverse(row.count),
            Reverse(row.affected_runs),
            row.label.clone(),
        )
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triage_report_renders_core_sections() {
        let report = ProtocolCampaignTriageReport {
            campaign_id: "rust-baseline-grok4-xai".to_string(),
            scope: "campaign-wide protocol triage".to_string(),
            selected_runs: 12,
            campaign_runs: 12,
            summary: ProtocolCampaignSummary {
                eligible_runs: 10,
                full_runs: 8,
                partial_runs: 1,
                error_runs: 1,
                missing_runs: 0,
                ineligible_runs: 2,
                runs_with_issue_calls: 6,
            },
            evidence: ProtocolCampaignEvidence {
                total_tool_calls: 120,
                reviewed_tool_calls: 110,
                missing_tool_call_reviews: 10,
                known_segments: 32,
                usable_segment_reviews: 29,
                mismatched_segment_reviews: 1,
                missing_segment_reviews: 2,
                artifact_failure_runs: 1,
                duplicate_artifacts: 4,
            },
            issue_kinds: vec![ProtocolCampaignCountRow {
                label: "search_thrash".to_string(),
                count: 18,
                affected_runs: 6,
            }],
            issue_tools: vec![ProtocolCampaignCountRow {
                label: "read_file".to_string(),
                count: 12,
                affected_runs: 5,
            }],
            nearby_segment_labels: vec![ProtocolCampaignCountRow {
                label: "refine_search".to_string(),
                count: 9,
                affected_runs: 4,
            }],
            nearby_segment_statuses: vec![ProtocolCampaignCountRow {
                label: "mixed".to_string(),
                count: 9,
                affected_runs: 4,
            }],
            problem_families: vec![ProtocolCampaignFamilyRow {
                label: "issue: search_thrash".to_string(),
                family_kind: "issue_family".to_string(),
                affected_runs: 6,
                affected_calls: 18,
                likely_owner: "ploke-tui tool harness".to_string(),
                exemplar_run: Some("tokio-rs__tokio-0001".to_string()),
                note: Some("completed protocol data still shows friction".to_string()),
                success_metric: Some("lower search_thrash calls".to_string()),
            }],
            exemplars: vec![ProtocolCampaignExemplarRow {
                run_id: "tokio-rs__tokio-0001".to_string(),
                protocol_status: "full".to_string(),
                matching_calls: 6,
                total_issues: 9,
                tool_calls_total: 14,
                focus: Some("issue=search_thrash".to_string()),
                note: None,
            }],
            next_steps: vec![
                "if you want exemplar runs for the top issue family, try `ploke-eval inspect protocol-overview --campaign rust-baseline-grok4-xai --issue search_thrash`".to_string(),
            ],
            ..ProtocolCampaignTriageReport::default()
        };

        let rendered = render_protocol_campaign_triage_report(&report, 100);

        assert!(rendered.contains("Campaign protocol triage"));
        assert!(rendered.contains("Campaign evidence reliability"));
        assert!(rendered.contains("Issue surface"));
        assert!(rendered.contains("Top problem families"));
        assert!(rendered.contains("Representative exemplars"));
        assert!(rendered.contains("Next moves"));
    }
}
