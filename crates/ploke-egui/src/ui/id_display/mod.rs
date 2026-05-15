//! UI-only display policy for long passive identifiers.

use std::fmt;
use std::hash::Hash;

use eframe::egui;

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

    response.context_menu(|ui| {
        if ui.button("Copy full id").clicked() {
            ui.ctx().copy_text(full.to_owned());
            ui.close();
        }
    });

    response
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

#[cfg(test)]
mod tests {
    use super::ShortId;

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
}
