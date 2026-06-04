//! UI-only display policy for long passive identifiers.

use std::fmt;
use std::hash::Hash;

use eframe::egui;
use ploke_records::history::{ArtifactRefRecord, TreeKeyHashRecord};
use ploke_records::ids::ArtifactId;

const HASH_CHARS: usize = 8;
const MAX_PLAIN_CHARS: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShortId<'a> {
    full: &'a str,
}

impl<'a> ShortId<'a> {
    pub(crate) fn new(full: &'a str) -> Option<Self> {
        is_shortenable_id(full).then_some(Self { full })
    }

    fn parts(self) -> ShortIdParts<'a> {
        if let Some((prefix, hash)) = self.full.rsplit_once(':') {
            return ShortIdParts::Separated {
                prefix,
                separator: ":",
                short: short_prefix(hash, HASH_CHARS),
            };
        }

        if let Some((prefix, hash)) = self.full.rsplit_once('-') {
            return ShortIdParts::Separated {
                prefix,
                separator: "-",
                short: short_prefix(hash, HASH_CHARS),
            };
        }

        ShortIdParts::Plain {
            short: short_prefix(self.full, MAX_PLAIN_CHARS),
        }
    }
}

impl fmt::Display for ShortId<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.parts() {
            ShortIdParts::Separated {
                prefix,
                separator,
                short,
            } => write!(f, "{prefix}{separator}{short}..."),
            ShortIdParts::Plain { short } => write!(f, "{short}..."),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShortIdParts<'a> {
    Separated {
        prefix: &'a str,
        separator: &'static str,
        short: &'a str,
    },
    Plain {
        short: &'a str,
    },
}

pub(crate) fn expandable_id(ui: &mut egui::Ui, id_source: impl Hash, full: &str) -> egui::Response {
    let Some(short) = ShortId::new(full) else {
        return ui.monospace(full);
    };
    let value = CopyableId::new(full);

    let id = ui.make_persistent_id(("ploke-egui.short-id", id_source));
    let mut expanded = ui.data(|data| data.get_temp::<bool>(id).unwrap_or(false));
    let label = if expanded {
        full.to_owned()
    } else {
        short.to_string()
    };

    let response = ui
        .monospace(label)
        .on_hover_text("Click to expand. Right click to copy the full id.");

    if response.clicked() {
        expanded = !expanded;
        ui.data_mut(|data| data.insert_temp(id, expanded));
    }

    attach_copy_context_menu(&response, &value);
    copy_button(ui, &value);

    response
}

pub(crate) trait CopyableExpandable {
    fn full_text(&self) -> &str;
    fn copy_menu_label(&self) -> &'static str;
    fn copy_button_hover(&self) -> &'static str;
    fn hover_text(&self, expanded: bool, expandable: bool) -> &'static str;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CopyableId<'a> {
    full: &'a str,
}

impl<'a> CopyableId<'a> {
    pub(crate) fn new(full: &'a str) -> Self {
        Self { full }
    }
}

impl CopyableExpandable for CopyableId<'_> {
    fn full_text(&self) -> &str {
        self.full
    }

    fn copy_menu_label(&self) -> &'static str {
        "Copy full id"
    }

    fn copy_button_hover(&self) -> &'static str {
        "Copy full id"
    }

    fn hover_text(&self, expanded: bool, expandable: bool) -> &'static str {
        match (expanded, expandable) {
            (true, true) => "Click to collapse. Right click to copy the full id.",
            (false, true) => "Click to expand. Right click to copy the full id.",
            (_, false) => "Right click to copy the full id.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CopyablePath<'a> {
    full: &'a str,
}

impl<'a> CopyablePath<'a> {
    pub(crate) fn new(full: &'a str) -> Self {
        Self { full }
    }

    pub(crate) fn tail(self) -> &'a str {
        path_tail(self.full)
    }

    pub(crate) fn is_expandable(self) -> bool {
        self.tail() != self.full
    }
}

impl CopyableExpandable for CopyablePath<'_> {
    fn full_text(&self) -> &str {
        self.full
    }

    fn copy_menu_label(&self) -> &'static str {
        "Copy full path"
    }

    fn copy_button_hover(&self) -> &'static str {
        "Copy full path"
    }

    fn hover_text(&self, expanded: bool, expandable: bool) -> &'static str {
        match (expanded, expandable) {
            (true, true) => "Click to collapse. Right click to copy the full path.",
            (false, true) => "Click to expand. Right click to copy the full path.",
            (_, false) => "Right click to copy the full path.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CopyableArtifactFile<'a> {
    full: &'a str,
}

impl<'a> CopyableArtifactFile<'a> {
    pub(crate) fn new(full: &'a str) -> Self {
        Self { full }
    }

    pub(crate) fn display_tail(self) -> &'a str {
        artifact_file_tail(self.full)
    }

    pub(crate) fn is_expandable(self) -> bool {
        self.display_tail() != self.full
    }
}

impl CopyableExpandable for CopyableArtifactFile<'_> {
    fn full_text(&self) -> &str {
        self.full
    }

    fn copy_menu_label(&self) -> &'static str {
        "Copy artifact path"
    }

    fn copy_button_hover(&self) -> &'static str {
        "Copy artifact path"
    }

    fn hover_text(&self, expanded: bool, expandable: bool) -> &'static str {
        match (expanded, expandable) {
            (true, true) => "Click to collapse. Right click to copy the artifact path.",
            (false, true) => "Click to expand. Right click to copy the artifact path.",
            (_, false) => "Right click to copy the artifact path.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CopyableRunName<'a> {
    full: &'a str,
}

const RUN_NAME_COMPACT_MAX_BYTES: usize = 44;

impl<'a> CopyableRunName<'a> {
    pub(crate) fn new(full: &'a str) -> Self {
        Self { full }
    }

    pub(crate) fn is_compact(&self) -> bool {
        self.full.len() > RUN_NAME_COMPACT_MAX_BYTES
    }

    pub(crate) fn compact_label(&self) -> std::borrow::Cow<'_, str> {
        if !self.is_compact() {
            return std::borrow::Cow::Borrowed(self.full);
        }
        std::borrow::Cow::Owned(run_name_ellipsis_tail(
            self.full,
            RUN_NAME_COMPACT_MAX_BYTES,
        ))
    }
}

impl CopyableExpandable for CopyableRunName<'_> {
    fn full_text(&self) -> &str {
        self.full
    }

    fn copy_menu_label(&self) -> &'static str {
        "Copy run name"
    }

    fn copy_button_hover(&self) -> &'static str {
        "Copy run name"
    }

    fn hover_text(&self, expanded: bool, expandable: bool) -> &'static str {
        if expandable && !expanded {
            "Click to expand the full run id. Right click to copy."
        } else {
            "Right click to copy the run name."
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CopyableText<'a> {
    full: &'a str,
}

impl<'a> CopyableText<'a> {
    pub(crate) fn new(full: &'a str) -> Self {
        Self { full }
    }
}

impl CopyableExpandable for CopyableText<'_> {
    fn full_text(&self) -> &str {
        self.full
    }

    fn copy_menu_label(&self) -> &'static str {
        "Copy text"
    }

    fn copy_button_hover(&self) -> &'static str {
        "Copy text"
    }

    fn hover_text(&self, expanded: bool, expandable: bool) -> &'static str {
        match (expanded, expandable) {
            (true, true) => "Click to collapse. Right click to copy the full text.",
            (false, true) => "Click to expand. Right click to copy the full text.",
            (_, false) => "Right click to copy the full text.",
        }
    }
}

pub(crate) fn attach_copy_context_menu(response: &egui::Response, value: &impl CopyableExpandable) {
    response.context_menu(|ui| {
        if ui.button(value.copy_menu_label()).clicked() {
            ui.ctx().copy_text(value.full_text().to_owned());
            ui.close();
        }
    });
}

pub(crate) fn copy_button(ui: &mut egui::Ui, value: &impl CopyableExpandable) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
    let response = response.on_hover_text(value.copy_button_hover());
    if ui.is_rect_visible(rect) {
        paint_copy_icon_button(ui, rect, &response);
    }
    if response.clicked() {
        ui.ctx().copy_text(value.full_text().to_owned());
    }
    attach_copy_context_menu(&response, value);
    response
}

fn paint_copy_icon_button(ui: &egui::Ui, rect: egui::Rect, response: &egui::Response) {
    let visuals = ui.style().interact(response);
    ui.painter().rect_filled(rect, 2.0, visuals.weak_bg_fill);

    let icon_rect = rect.shrink(4.0);
    let offset = egui::vec2(3.0, -3.0);
    let back = icon_rect.translate(offset);
    let front = icon_rect.translate(-offset);
    paint_rect_outline(ui.painter(), back, visuals.fg_stroke);
    paint_rect_outline(ui.painter(), front, visuals.fg_stroke);
}

fn paint_rect_outline(painter: &egui::Painter, rect: egui::Rect, stroke: egui::Stroke) {
    painter.line_segment([rect.left_top(), rect.right_top()], stroke);
    painter.line_segment([rect.right_top(), rect.right_bottom()], stroke);
    painter.line_segment([rect.right_bottom(), rect.left_bottom()], stroke);
    painter.line_segment([rect.left_bottom(), rect.left_top()], stroke);
}

pub(crate) trait InteractiveId {
    fn full_id(&self) -> &str;

    fn compact_label(&self) -> (&str, bool) {
        compact_label(self.full_id())
    }

    fn id_prefix(&self) -> Option<&str> {
        id_prefix(self.full_id())
    }
}

pub(crate) trait TraceId: InteractiveId {
    fn trace_artifact_id_row<'a>(&'a self, slot: &str, label: &str) -> &'a Self {
        let full = self.full_id();
        let (compact, expandable) = self.compact_label();
        let _span = tracing::trace_span!(
            "ploke_egui.inspector.render_artifact_id_row",
            slot = slot,
            label = label,
            full = full,
            compact = compact,
            expandable = expandable
        )
        .entered();
        self
    }
}

impl<T> TraceId for T where T: InteractiveId + ?Sized {}

impl InteractiveId for ArtifactId {
    fn full_id(&self) -> &str {
        self.0.as_str()
    }
}

impl InteractiveId for ArtifactRefRecord {
    fn full_id(&self) -> &str {
        self.as_str()
    }
}

impl InteractiveId for TreeKeyHashRecord {
    fn full_id(&self) -> &str {
        self.hash.0.as_str()
    }
}

impl InteractiveId for str {
    fn full_id(&self) -> &str {
        self
    }
}

pub(crate) fn id_prefix(full: &str) -> Option<&str> {
    full.rsplit_once(':')
        .or_else(|| full.rsplit_once('-'))
        .map(|(prefix, _)| prefix)
}

fn is_shortenable_id(value: &str) -> bool {
    if value.contains('/') || value.chars().any(char::is_whitespace) {
        return false;
    }

    value
        .rsplit_once(':')
        .or_else(|| value.rsplit_once('-'))
        .map(|(_, suffix)| is_hexish(suffix) && suffix.len() > HASH_CHARS)
        .unwrap_or_else(|| value.len() > MAX_PLAIN_CHARS && is_hexish(value))
}

fn is_hexish(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit() || byte == b'_')
}

fn short_prefix(value: &str, max_chars: usize) -> &str {
    value
        .char_indices()
        .nth(max_chars)
        .map(|(index, _)| &value[..index])
        .unwrap_or(value)
}

fn run_name_ellipsis_tail(full: &str, max_bytes: usize) -> String {
    let tail = path_tail(full);
    if tail.len() <= max_bytes {
        return format!("…{tail}");
    }
    let mut start = tail.len().saturating_sub(max_bytes);
    while start < tail.len() && !tail.is_char_boundary(start) {
        start += 1;
    }
    format!("…{}", &tail[start..])
}

fn path_tail(value: &str) -> &str {
    let trimmed = value.trim_end_matches(|ch| ch == '/' || ch == '\\');
    if trimmed.is_empty() {
        return value;
    }

    trimmed
        .rfind(|ch| ch == '/' || ch == '\\')
        .map(|index| &trimmed[index + 1..])
        .unwrap_or(trimmed)
}

fn artifact_file_tail(value: &str) -> &str {
    let tail = path_tail(value);
    tail.rsplit_once("__")
        .map(|(_, suffix)| suffix)
        .unwrap_or(tail)
}

fn compact_label(full: &str) -> (&str, bool) {
    let suffix = full
        .rsplit_once(':')
        .or_else(|| full.rsplit_once('-'))
        .map(|(_, suffix)| suffix)
        .unwrap_or(full);

    if is_hexish(suffix) && suffix.len() > HASH_CHARS {
        return (short_prefix(suffix, HASH_CHARS), true);
    }

    if id_prefix(full).is_some() {
        return (suffix, false);
    }

    if full.len() > HASH_CHARS && is_hexish(full) {
        return (short_prefix(full, HASH_CHARS), true);
    }

    (full, false)
}

#[cfg(test)]
mod tests {
    use super::{
        CopyableArtifactFile, CopyablePath, CopyableRunName, ShortId, compact_label, id_prefix,
    };

    fn short(value: &str) -> Option<String> {
        ShortId::new(value).map(|id| id.to_string())
    }

    #[test]
    fn shortens_colon_hash_ids_without_losing_prefix() {
        assert_eq!(
            short("text-file-sha256:f6f73d0a2259c38d377144ed14f53be3"),
            Some("text-file-sha256:f6f73d0a...".to_owned())
        );
    }

    #[test]
    fn shortens_dash_scheduler_ids_without_losing_prefix() {
        assert_eq!(
            short("node-71c2d4aa6246bf58"),
            Some("node-71c2d4aa...".to_owned())
        );
    }

    #[test]
    fn leaves_paths_and_short_values_alone() {
        assert_eq!(short("crates/ploke-tui/src/tools/cargo.rs"), None);
        assert_eq!(short("not_available"), None);
    }

    #[test]
    fn copyable_path_defaults_to_tail_component() {
        let path = CopyablePath::new(
            "/home/brasides/.ploke-eval/instances/prototype1/campaign/runs/run-1/record.json.gz",
        );

        assert_eq!(path.tail(), "record.json.gz");
        assert!(path.is_expandable());
    }

    #[test]
    fn copyable_path_keeps_single_component_unexpanded() {
        let path = CopyablePath::new("record.json.gz");

        assert_eq!(path.tail(), "record.json.gz");
        assert!(!path.is_expandable());
    }

    #[test]
    fn copyable_path_ignores_trailing_separators_for_tail() {
        let path = CopyablePath::new("/tmp/worktree/");

        assert_eq!(path.tail(), "worktree");
        assert!(path.is_expandable());
    }

    #[test]
    fn copyable_run_name_compacts_long_ids_with_ellipsis_tail() {
        let long = "campaigns/foo/bar/run-with-a-very-long-instance-id-0123456789abcdef";
        let run = CopyableRunName::new(long);
        assert!(run.is_compact());
        let compact = run.compact_label();
        assert!(compact.starts_with('…'));
        assert!(compact.len() <= 48);
    }

    #[test]
    fn copyable_artifact_file_uses_subject_suffix_tail() {
        let artifact = CopyableArtifactFile::new(
            "/tmp/run/1779713106220_tool_call_review_BurntSushi__ripgrep-2209.json",
        );

        assert_eq!(artifact.display_tail(), "ripgrep-2209.json");
        assert!(artifact.is_expandable());
    }

    #[test]
    fn compact_label_omits_prefix_for_hash_ids() {
        assert_eq!(
            compact_label("text-file-sha256:f6f73d0a2259c38d377144ed14f53be3"),
            ("f6f73d0a", true)
        );
        assert_eq!(
            compact_label("artifact:git-commit:deadbeefcafebabe"),
            ("deadbeef", true)
        );
    }

    #[test]
    fn compact_label_keeps_non_hash_suffix_without_prefix() {
        assert_eq!(compact_label("artifact:after"), ("after", false));
    }

    #[test]
    fn id_prefix_uses_last_separator_boundary() {
        assert_eq!(
            id_prefix("artifact:git-commit:deadbeefcafebabe"),
            Some("artifact:git-commit")
        );
        assert_eq!(id_prefix("node-71c2d4aa6246bf58"), Some("node"));
    }
}
