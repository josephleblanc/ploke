use std::fmt::Write as _;
use std::path::PathBuf;

use crate::ui::app::layout;

use super::{RUN_NAME_MAX_CHARS, RunPicker};

const WIDEST_MENU_LABEL_LIMIT: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerDiagnostics {
    pub root: PathBuf,
    pub selected_index: Option<usize>,
    pub closed_width_logical_px: u32,
    pub run_name_max_chars: usize,
    pub entries: Vec<EntryDiagnostics>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryDiagnostics {
    pub index: usize,
    pub selected: bool,
    pub name: String,
    pub selected_label: String,
    pub selected_label_chars: usize,
    pub menu_label: String,
    pub menu_label_chars: usize,
}

impl RunPicker {
    pub fn diagnostics(&self) -> PickerDiagnostics {
        PickerDiagnostics {
            root: self.root.clone(),
            selected_index: self.selected,
            closed_width_logical_px: logical_px(layout::LEFT_SIDEBAR_WIDTH),
            run_name_max_chars: RUN_NAME_MAX_CHARS,
            entries: self
                .entries
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    let selected_label = entry.selected_label();
                    let menu_label = entry.menu_label();
                    EntryDiagnostics {
                        index,
                        selected: self.selected == Some(index),
                        name: entry.name.clone(),
                        selected_label_chars: selected_label.chars().count(),
                        menu_label_chars: menu_label.chars().count(),
                        selected_label: selected_label.to_owned(),
                        menu_label: menu_label.to_owned(),
                    }
                })
                .collect(),
        }
    }
}

fn logical_px(value: f32) -> u32 {
    value.round().max(0.0) as u32
}

impl PickerDiagnostics {
    pub fn widest_menu_labels(&self, limit: usize) -> Vec<&EntryDiagnostics> {
        let mut entries = self.entries.iter().collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            right
                .menu_label_chars
                .cmp(&left.menu_label_chars)
                .then_with(|| left.index.cmp(&right.index))
        });
        entries.truncate(limit);
        entries
    }

    pub fn render_text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "run-picker diagnostics:");
        let _ = writeln!(out, "root: {}", self.root.display());
        let _ = writeln!(out, "selected_index: {:?}", self.selected_index);
        let _ = writeln!(
            out,
            "closed_width: {}px, selected_label_max_chars: {}",
            self.closed_width_logical_px, self.run_name_max_chars
        );
        let _ = writeln!(out, "widest menu labels:");
        for entry in self.widest_menu_labels(WIDEST_MENU_LABEL_LIMIT) {
            let marker = if entry.selected { "*" } else { "-" };
            let _ = writeln!(
                out,
                "{marker} [{}] menu_label({} chars): {}",
                entry.index, entry.menu_label_chars, entry.menu_label
            );
        }
        let _ = writeln!(out, "entries:");
        for entry in &self.entries {
            let marker = if entry.selected { "*" } else { "-" };
            let _ = writeln!(
                out,
                "{marker} [{}] selected_label({} chars): {}",
                entry.index, entry.selected_label_chars, entry.selected_label
            );
            let _ = writeln!(
                out,
                "      menu_label({} chars): {}",
                entry.menu_label_chars, entry.menu_label
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn widest_menu_labels_are_sorted_by_width() {
        let diagnostics = PickerDiagnostics {
            root: PathBuf::from("/tmp/runs"),
            selected_index: Some(1),
            closed_width_logical_px: 200,
            run_name_max_chars: 28,
            entries: vec![
                entry(0, false, "short"),
                entry(1, true, "the longest label here"),
                entry(2, false, "medium label"),
            ],
        };

        let labels = diagnostics.widest_menu_labels(2);

        assert_eq!(labels[0].index, 1);
        assert_eq!(labels[1].index, 2);
    }

    fn entry(index: usize, selected: bool, menu_label: &str) -> EntryDiagnostics {
        EntryDiagnostics {
            index,
            selected,
            name: format!("run-{index}"),
            selected_label: format!("run-{index}"),
            selected_label_chars: 5,
            menu_label: menu_label.to_owned(),
            menu_label_chars: menu_label.chars().count(),
        }
    }
}
