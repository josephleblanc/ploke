//! Render-boundary diff formatting and highlighting for patch inspection.

use eframe::egui;
use egui::text::{LayoutJob, TextFormat};
use egui_extras::syntax_highlighting::{self, CodeTheme};
use similar::TextDiff;

pub(crate) fn unified_rust_diff(path: &str, before: &str, after: &str) -> String {
    let header_a = format!("a/{path}");
    let header_b = format!("b/{path}");
    let body = TextDiff::from_lines(before, after)
        .unified_diff()
        .header(&header_a, &header_b)
        .to_string();
    let mut diff = format!("diff --git {header_a} {header_b}\n{body}");
    if !diff.ends_with('\n') {
        diff.push('\n');
    }
    diff
}

pub(crate) fn highlighted_diff_job(ui: &egui::Ui, diff: &str, wrap_width: f32) -> LayoutJob {
    let theme = if ui.visuals().dark_mode {
        CodeTheme::dark(12.0)
    } else {
        CodeTheme::light(12.0)
    };
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    job.break_on_newline = true;

    for line in diff.split_inclusive('\n') {
        append_diff_line(ui, &theme, &mut job, line);
    }

    if job.text.is_empty() {
        append_plain(
            &mut job,
            "",
            egui::Color32::TRANSPARENT,
            ui.visuals().text_color(),
        );
    }
    job
}

fn append_diff_line(ui: &egui::Ui, theme: &CodeTheme, job: &mut LayoutJob, line: &str) {
    let line = line.strip_suffix('\n').unwrap_or(line);
    if let Some((marker, payload, background)) = rust_payload(line) {
        append_plain(job, marker, background, marker_color(marker, ui));
        append_rust_payload(ui, theme, job, payload, background);
        append_plain(
            job,
            "\n",
            egui::Color32::TRANSPARENT,
            ui.visuals().text_color(),
        );
        return;
    }

    let (foreground, background) = diff_line_style(line, ui);
    append_plain(job, line, background, foreground);
    append_plain(
        job,
        "\n",
        egui::Color32::TRANSPARENT,
        ui.visuals().text_color(),
    );
}

fn rust_payload(line: &str) -> Option<(&'static str, &str, egui::Color32)> {
    if line.starts_with("+++") || line.starts_with("---") {
        return None;
    }
    if let Some(payload) = line.strip_prefix('+') {
        Some((
            "+",
            payload,
            egui::Color32::from_rgba_unmultiplied(32, 120, 64, 38),
        ))
    } else if let Some(payload) = line.strip_prefix('-') {
        Some((
            "-",
            payload,
            egui::Color32::from_rgba_unmultiplied(150, 48, 48, 42),
        ))
    } else {
        line.strip_prefix(' ').map(|payload| {
            (
                " ",
                payload,
                egui::Color32::from_rgba_unmultiplied(128, 128, 128, 10),
            )
        })
    }
}

fn append_rust_payload(
    ui: &egui::Ui,
    theme: &CodeTheme,
    job: &mut LayoutJob,
    payload: &str,
    background: egui::Color32,
) {
    let highlighted = syntax_highlighting::highlight(ui.ctx(), ui.style(), theme, payload, "rs");
    for section in highlighted.sections {
        if section.byte_range.is_empty() {
            continue;
        }
        let text = &highlighted.text[section.byte_range];
        let mut format = section.format;
        format.background = background;
        job.append(text, section.leading_space, format);
    }
}

fn append_plain(job: &mut LayoutJob, text: &str, background: egui::Color32, color: egui::Color32) {
    let mut format = TextFormat::default();
    format.font_id = egui::FontId::monospace(12.0);
    format.color = color;
    format.background = background;
    job.append(text, 0.0, format);
}

fn marker_color(marker: &str, ui: &egui::Ui) -> egui::Color32 {
    match marker {
        "+" => egui::Color32::from_rgb(64, 190, 110),
        "-" => egui::Color32::from_rgb(230, 92, 92),
        _ => ui.visuals().weak_text_color(),
    }
}

fn diff_line_style(line: &str, ui: &egui::Ui) -> (egui::Color32, egui::Color32) {
    if line.starts_with("@@") {
        (
            egui::Color32::from_rgb(108, 151, 255),
            egui::Color32::from_rgba_unmultiplied(72, 96, 180, 38),
        )
    } else if line.starts_with("diff --git") || line.starts_with("index ") {
        (
            ui.visuals().weak_text_color(),
            egui::Color32::from_rgba_unmultiplied(128, 128, 128, 18),
        )
    } else if line.starts_with("+++") || line.starts_with("---") {
        (
            egui::Color32::from_rgb(120, 170, 220),
            egui::Color32::from_rgba_unmultiplied(60, 100, 140, 24),
        )
    } else {
        (ui.visuals().text_color(), egui::Color32::TRANSPARENT)
    }
}

#[cfg(test)]
mod tests {
    use super::unified_rust_diff;

    #[test]
    fn unified_rust_diff_uses_git_style_file_headers() {
        let diff = unified_rust_diff("src/lib.rs", "fn a() {}\n", "fn b() {}\n");

        assert!(diff.contains("diff --git a/src/lib.rs b/src/lib.rs"));
        assert!(diff.contains("--- a/src/lib.rs"));
        assert!(diff.contains("+++ b/src/lib.rs"));
        assert!(diff.contains("-fn a() {}"));
        assert!(diff.contains("+fn b() {}"));
    }
}
