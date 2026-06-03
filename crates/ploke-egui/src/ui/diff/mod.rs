//! Render-boundary diff formatting and highlighting for patch inspection.

use std::sync::Arc;

use eframe::egui;
use egui::text::{LayoutJob, TextFormat};
use egui_extras::syntax_highlighting::{self, CodeTheme};
use similar::TextDiff;

use crate::allocation::scope;
use crate::ui::inspector::PatchInspection;
use crate::ui::theme::{DiffLineColors, tokens_from_ui};

#[derive(Debug, Default)]
pub(crate) struct PatchDiffCache {
    entries: Vec<PatchDiffEntry>,
    rebuilds: usize,
}

impl PatchDiffCache {
    pub(crate) fn highlighted_patch_galley(
        &mut self,
        ui: &egui::Ui,
        patch: PatchInspection<'_>,
    ) -> Arc<egui::Galley> {
        self.highlighted_galley(
            ui,
            PatchDiffInput {
                patch_id: patch.patch_id(),
                target_relpath: patch.target_relpath(),
                source_hash: patch.source_content_hash(),
                proposed_hash: patch.proposed_content_hash(),
                source_content: patch.source_content(),
                proposed_content: patch.proposed_content(),
            },
        )
    }

    fn highlighted_galley(
        &mut self,
        ui: &egui::Ui,
        input: PatchDiffInput<'_>,
    ) -> Arc<egui::Galley> {
        let theme_key = tokens_from_ui(ui).cache_theme_key();
        if let Some(entry) = self
            .entries
            .iter()
            .find(|entry| entry.key.matches(input, theme_key))
        {
            let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DEBUG_DIFF_CACHE_HIT).entered();
            return entry.galley.clone();
        }

        let diff = {
            let _span =
                tracing::trace_span!(scope::INSPECTOR_PATCH_DEBUG_DIFF_BUILD_TEXT).entered();
            unified_rust_diff(
                input.target_relpath,
                input.source_content,
                input.proposed_content,
            )
        };
        let job = {
            let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DEBUG_DIFF_HIGHLIGHT).entered();
            highlighted_diff_job(ui, diff.as_str(), f32::INFINITY)
        };
        let galley = {
            let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DEBUG_DIFF_LAYOUT).entered();
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        };
        {
            let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DEBUG_DIFF_STORE).entered();
            self.entries.push(PatchDiffEntry {
                key: PatchDiffKey::from_input(input, theme_key),
                galley: galley.clone(),
            });
        }
        self.rebuilds += 1;
        galley
    }

    #[cfg(test)]
    pub(crate) fn rebuilds(&self) -> usize {
        self.rebuilds
    }
}

#[derive(Debug, Clone)]
struct PatchDiffEntry {
    key: PatchDiffKey,
    galley: Arc<egui::Galley>,
}

#[derive(Debug, Clone, Copy)]
struct PatchDiffInput<'a> {
    patch_id: &'a str,
    target_relpath: &'a str,
    source_hash: &'a str,
    proposed_hash: &'a str,
    source_content: &'a str,
    proposed_content: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PatchDiffKey {
    patch_id: String,
    target_relpath: String,
    source_hash: String,
    proposed_hash: String,
    theme_key: u8,
}

impl PatchDiffKey {
    fn from_input(input: PatchDiffInput<'_>, theme_key: u8) -> Self {
        Self {
            patch_id: input.patch_id.to_owned(),
            target_relpath: input.target_relpath.to_owned(),
            source_hash: input.source_hash.to_owned(),
            proposed_hash: input.proposed_hash.to_owned(),
            theme_key,
        }
    }

    fn matches(&self, input: PatchDiffInput<'_>, theme_key: u8) -> bool {
        self.theme_key == theme_key
            && self.patch_id == input.patch_id
            && self.target_relpath == input.target_relpath
            && self.source_hash == input.source_hash
            && self.proposed_hash == input.proposed_hash
    }
}

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
    let tokens = tokens_from_ui(ui);
    let theme = if tokens.is_dark {
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
    if let Some((marker, payload, background)) = rust_payload(line, ui) {
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

fn rust_payload<'a>(line: &'a str, ui: &egui::Ui) -> Option<(&'a str, &'a str, egui::Color32)> {
    let tokens = tokens_from_ui(ui);
    if line.starts_with("+++") || line.starts_with("---") {
        return None;
    }
    if let Some(payload) = line.strip_prefix('+') {
        Some(("+", payload, tokens.diff_add_bg()))
    } else if let Some(payload) = line.strip_prefix('-') {
        Some(("-", payload, tokens.diff_remove_bg()))
    } else {
        line.strip_prefix(' ')
            .map(|payload| (" ", payload, tokens.diff_context_bg()))
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
    let colors = DiffLineColors::from(tokens_from_ui(ui));
    match marker {
        "+" => colors.add,
        "-" => colors.remove,
        _ => ui.visuals().weak_text_color(),
    }
}

fn diff_line_style(line: &str, ui: &egui::Ui) -> (egui::Color32, egui::Color32) {
    let tokens = tokens_from_ui(ui);
    if line.starts_with("@@") {
        (tokens.diff_hunk, tokens.diff_hunk_bg())
    } else if line.starts_with("diff --git") || line.starts_with("index ") {
        (ui.visuals().weak_text_color(), tokens.diff_meta_bg())
    } else if line.starts_with("+++") || line.starts_with("---") {
        (tokens.diff_meta, tokens.diff_header_bg())
    } else {
        (ui.visuals().text_color(), egui::Color32::TRANSPARENT)
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use super::{PatchDiffCache, PatchDiffInput, unified_rust_diff};

    #[test]
    fn unified_rust_diff_uses_git_style_file_headers() {
        let diff = unified_rust_diff("src/lib.rs", "fn a() {}\n", "fn b() {}\n");

        assert!(diff.contains("diff --git a/src/lib.rs b/src/lib.rs"));
        assert!(diff.contains("--- a/src/lib.rs"));
        assert!(diff.contains("+++ b/src/lib.rs"));
        assert!(diff.contains("-fn a() {}"));
        assert!(diff.contains("+fn b() {}"));
    }

    #[test]
    fn patch_diff_cache_invalidates_on_hash_or_theme_change() {
        let mut cache = PatchDiffCache::default();
        let run_once = std::sync::Once::new();

        egui::__run_test_ui(|ui| {
            run_once.call_once(|| {
            let first = PatchDiffInput {
                patch_id: "patch:1",
                target_relpath: "src/lib.rs",
                source_hash: "sha256:source-a",
                proposed_hash: "sha256:proposed-a",
                source_content: "fn a() {}\n",
                proposed_content: "fn b() {}\n",
            };
            let mut rebuilds = cache.rebuilds();
            cache.highlighted_galley(ui, first);
            assert!(
                cache.rebuilds() > rebuilds,
                "expected cache miss on first highlighted galley"
            );
            rebuilds = cache.rebuilds();

            cache.highlighted_galley(ui, first);
            assert_eq!(
                cache.rebuilds(),
                rebuilds,
                "expected cache hit on repeated input"
            );

            cache.highlighted_galley(
                ui,
                PatchDiffInput {
                    source_hash: "sha256:source-b",
                    source_content: "fn c() {}\n",
                    ..first
                },
            );
            assert!(
                cache.rebuilds() > rebuilds,
                "expected cache miss when diff input hash changes"
            );
            rebuilds = cache.rebuilds();

            let other = crate::ui::theme::PaletteTokens::for_scheme(
                crate::ui::theme::NamedScheme::GruvboxLight,
            );
            other.install_on_context(ui.ctx());
            cache.highlighted_galley(ui, first);
            assert!(
                cache.rebuilds() > rebuilds,
                "expected cache miss when theme key changes"
            );
            });
        });
    }
}
