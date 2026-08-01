use eframe::egui::{Color32, RichText, ScrollArea, Ui};
use ploke_eval::walk_client::{ValueSource, WalkConfigSnapshot};

pub(in crate::app) struct ConfigPanel<'a> {
    pub(in crate::app) snapshot: Option<&'a WalkConfigSnapshot>,
    pub(in crate::app) pending: bool,
    pub(in crate::app) client_available: bool,
}

#[derive(Debug, Default)]
pub(in crate::app) struct ConfigAction {
    pub(in crate::app) refresh: bool,
}

impl ConfigPanel<'_> {
    pub(in crate::app) fn show(self, ui: &mut Ui) -> ConfigAction {
        let mut action = ConfigAction::default();
        ui.horizontal_wrapped(|ui| {
            ui.heading("Admitted run configuration");
            if ui
                .add_enabled(
                    self.client_available && !self.pending,
                    eframe::egui::Button::new("Refresh Config"),
                )
                .clicked()
            {
                action.refresh = true;
            }
            if self.pending {
                ui.label(RichText::new("loading...").color(Color32::YELLOW));
            }
        });
        ui.label(
            "Effective values are loaded through the canonical setup admission and runtime-profile validator.",
        );
        ui.separator();

        let Some(config) = self.snapshot else {
            ui.label("No admitted configuration loaded");
            return action;
        };

        ScrollArea::vertical()
            .id_salt("walk_config")
            .show(ui, |ui| {
                ui.heading("Identity");
                ui.monospace(format!(
                    "campaign: {}\nparent: {}\nnode: {}\ngeneration: {}\nbranch: {}\nidentity: {}",
                    config.identity.record.campaign_id,
                    config.identity.record.parent_id,
                    config.identity.record.node_id,
                    config.identity.record.generation,
                    config.identity.record.branch_id,
                    config.identity.path.display(),
                ));

                ui.add_space(10.0);
                ui.heading("Setup admission");
                ui.monospace(format!(
                    "plan sha256: {}\nreceipt: {}\nstarted: {}\ncompleted head: {}\nmanifest sha256: {}\nslice sha256: {}\nprofile sha256: {}",
                    config.campaign.admission.plan_hash,
                    config.campaign.admission.path.display(),
                    config.campaign.admission.started_at,
                    config.campaign.admission.completed_head,
                    config.campaign.admission.hashes.manifest,
                    config.campaign.admission.hashes.slice,
                    config.campaign.admission.hashes.profile,
                ));

                ui.add_space(10.0);
                ui.heading("Campaign");
                ui.monospace(format!(
                    "manifest: {}\nmodel: {}\nroute: {:?}\nprovider: {}\neval max tokens: {}",
                    config.campaign.path.display(),
                    option_label(config.campaign.manifest.model_id.as_deref()),
                    config.campaign.manifest.route_source,
                    config.campaign.provider,
                    config
                        .campaign
                        .manifest
                        .eval
                        .max_tokens
                        .map_or_else(|| "-".to_string(), |value| value.to_string()),
                ));
                render_json(
                    ui,
                    "Resolved campaign",
                    serde_json::to_value(&config.campaign.resolved),
                );

                ui.add_space(10.0);
                ui.heading("Run profile");
                ui.monospace(format!(
                    "name: {}\npath: {}\nsha256: {}\nreported source: {}",
                    config.profile.record.name,
                    config.profile.commitment.profile_path.display(),
                    config.profile.commitment.sha256,
                    config
                        .profile
                        .reported_source
                        .as_deref()
                        .map_or_else(|| "-".to_string(), |path| path.display().to_string()),
                ));
                render_json(
                    ui,
                    "Admitted profile",
                    serde_json::to_value(&config.profile.record),
                );

                ui.add_space(10.0);
                ui.heading("Effective controller");
                ui.monospace(format!(
                    "mode: {:?}\nparallel cap: {} ({})\npatch cap: {} ({})\nprofile: {}",
                    config.control.mode,
                    config.control.parallel_cap.value,
                    source_label(config.control.parallel_cap.source),
                    config.control.patch_cap.value,
                    source_label(config.control.patch_cap.source),
                    config.control.path.display(),
                ));
            });
        action
    }
}

fn render_json(ui: &mut Ui, label: &str, value: Result<serde_json::Value, serde_json::Error>) {
    eframe::egui::CollapsingHeader::new(label)
        .id_salt(label)
        .show(ui, |ui| {
            let rendered = value
                .and_then(|value| serde_json::to_string_pretty(&value))
                .unwrap_or_else(|error| format!("configuration rendering failed: {error}"));
            ui.monospace(rendered);
        });
}

fn option_label(value: Option<&str>) -> &str {
    value.unwrap_or("-")
}

fn source_label(source: ValueSource) -> String {
    match source {
        ValueSource::Explicit(field) => format!("explicit {field:?}"),
        ValueSource::Derived(rule) => format!("derived {rule:?}"),
    }
}
