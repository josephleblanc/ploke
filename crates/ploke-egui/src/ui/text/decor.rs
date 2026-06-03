use egui::RichText;

use crate::ui::theme::PaletteTokens;
use ploke_records::ids::ArtifactId;
use ploke_records::invocation::InvocationRecord;
use ploke_records::invocation::Role;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Serialize)]
pub enum Badge<'a> {
    Parent(&'a ArtifactId),
    Child(&'a ArtifactId),
}

pub(crate) struct BadgeText<'a> {
    pub(crate) rich_text: RichText,
    artifact_id: &'a ArtifactId,
}

impl<'a> AsRef<RichText> for BadgeText<'a> {
    fn as_ref(&self) -> &RichText {
        &self.rich_text
    }
}

impl<'a> BadgeText<'a> {
    pub(crate) fn artifact_id(&self) -> &'a ArtifactId {
        self.artifact_id
    }

    pub(crate) fn show(self, ui: &mut egui::Ui) -> egui::Response {
        ui.label(self.rich_text)
    }
}

impl<'a> From<&'a Badge<'a>> for &'static str {
    fn from(value: &'a Badge) -> Self {
        match value {
            Badge::Parent(_) => "Parent",
            Badge::Child(_) => "Child",
        }
    }
}

impl<'a> Badge<'a> {
    pub(crate) fn bg_color(&self, tokens: PaletteTokens) -> egui::Color32 {
        match self {
            Self::Parent(_) => tokens.badge_parent,
            Self::Child(_) => tokens.badge_child,
        }
    }

    pub(crate) fn text_color(&self, tokens: PaletteTokens) -> egui::Color32 {
        tokens.badge_text
    }

    pub(crate) fn from_invocation(inv: &'a InvocationRecord) -> Option<Self> {
        let artifact_id = inv
            .node
            .as_ref()
            .and_then(|node| node.derived_artifact_id.as_ref())
            .or_else(|| {
                inv.request
                    .as_ref()
                    .and_then(|request| request.derived_artifact_id.as_ref())
            })?;

        match inv.role {
            Role::Child => Some(Self::Child(artifact_id)),
            Role::Successor => Some(Self::Parent(artifact_id)),
        }
    }

    pub(crate) fn artifact_id(&self) -> &'a ArtifactId {
        self.inner()
    }

    fn inner(&self) -> &'a ArtifactId {
        match self {
            Badge::Parent(art) => art,
            Badge::Child(art) => art,
        }
    }

    pub(crate) fn to_badge_text(&'a self, tokens: PaletteTokens) -> BadgeText<'a> {
        let text: &'static str = self.into();
        let fill = self.bg_color(tokens);
        let text_color = self.text_color(tokens);
        let rich_text = RichText::new(text)
            .background_color(fill)
            .color(text_color)
            .monospace();
        BadgeText {
            rich_text,
            artifact_id: self.inner(),
        }
    }
}
