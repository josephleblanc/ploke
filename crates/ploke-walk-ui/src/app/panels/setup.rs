use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, ComboBox, DragValue, Grid, RichText, ScrollArea, TextEdit, Ui};
use ploke_eval::{
    setup_client::{
        ArchiveScope, CampaignId, ChildScheduleModeRecord, EmbeddingRoute, EvalStorageBackend,
        ExecutionStopAfter, GenerationSource, ModelDefaults, ModelRouteSource, OracleGate,
        OracleMode, PatchGate, ProfilePreview, ProfileReceipt, ProfileRouteSource,
        ProtocolReasoningEffort, ProtocolReasoningMode, RunMode, RunProfileRecord, RunSetupBatch,
        RunSetupEmbedding, RunSetupModel, RunSetupPreview, RunSetupProfile, RunSetupProfilePreview,
        RunSetupProtocol, RunSetupReceipt, RunSetupRequest, ScoreProfile, SelectionEvidence,
        SelectionStrategy, TraceJsonl, default_run_profile, load_run_profile, preview_run_profile,
        save_run_profile,
    },
    walk_client::ValueSource,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchKind {
    Id,
    Manifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BatchDraft {
    kind: BatchKind,
    value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProfileKind {
    Name,
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProfileDraft {
    kind: ProfileKind,
    value: String,
    record: Option<RunProfileRecord>,
    reviewed: Option<ProfileBinding>,
    usable: Option<ProfileBinding>,
    saved: Option<ProfileReceipt>,
    notice: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProfileBinding {
    target: RunSetupProfile,
    preview: ProfilePreview,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ModelDraft {
    id: String,
    provider: String,
    route: Option<ModelRouteSource>,
    tokens: Option<u32>,
    use_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ProtocolDraft {
    id: String,
    provider: String,
    route: Option<ModelRouteSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct EmbeddingDraft {
    id: String,
    provider: String,
    route: Option<EmbeddingRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) struct SetupDraft {
    root: String,
    batch: BatchDraft,
    campaign: String,
    profile: ProfileDraft,
    primary: String,
    advanced: bool,
    model: ModelDraft,
    protocol: ProtocolDraft,
    embedding: EmbeddingDraft,
}

impl SetupDraft {
    pub(in crate::app) fn for_root(root: &Path) -> Self {
        let name = "new-run";
        let (record, notice) = match default_run_profile(name) {
            Ok(record) => (
                Some(record),
                Some("Loaded current canonical run-profile defaults".to_string()),
            ),
            Err(error) => (
                None,
                Some(format!(
                    "Could not load canonical run-profile defaults: {error}"
                )),
            ),
        };
        Self {
            root: root.display().to_string(),
            batch: BatchDraft {
                kind: BatchKind::Id,
                value: String::new(),
            },
            campaign: String::new(),
            profile: ProfileDraft {
                kind: ProfileKind::Name,
                value: name.to_string(),
                record,
                reviewed: None,
                usable: None,
                saved: None,
                notice,
            },
            primary: String::new(),
            advanced: false,
            model: ModelDraft::default(),
            protocol: ProtocolDraft::default(),
            embedding: EmbeddingDraft::default(),
        }
    }

    pub(in crate::app) fn request(&self) -> Result<RunSetupRequest, String> {
        let repo_root = required_path(&self.root, "repository root")?;
        let batch = match self.batch.kind {
            BatchKind::Id => RunSetupBatch::Id(required(&self.batch.value, "batch id")?),
            BatchKind::Manifest => {
                RunSetupBatch::Manifest(required_path(&self.batch.value, "batch manifest")?)
            }
        };
        let profile = match self.profile.kind {
            ProfileKind::Name => {
                RunSetupProfile::Name(required(&self.profile.value, "profile name")?)
            }
            ProfileKind::Path => {
                RunSetupProfile::Path(required_path(&self.profile.value, "profile path")?)
            }
        };
        Ok(RunSetupRequest {
            repo_root,
            batch,
            campaign: CampaignId::from(required(&self.campaign, "campaign id")?),
            profile,
            primary_instance: optional(&self.primary),
            model: RunSetupModel {
                id: optional(&self.model.id),
                provider: optional(&self.model.provider),
                route: self.model.route,
                max_tokens: self.model.tokens,
                use_default: self.model.use_default,
            },
            protocol: RunSetupProtocol {
                id: optional(&self.protocol.id),
                provider: optional(&self.protocol.provider),
                route: self.protocol.route,
            },
            embedding: RunSetupEmbedding {
                id: optional(&self.embedding.id),
                provider: optional(&self.embedding.provider),
                route: self.embedding.route,
            },
        })
    }

    pub(in crate::app) fn restore_profile(&mut self) {
        let name = match self.profile_name() {
            Ok(name) => name,
            Err(error) => {
                self.profile.notice = Some(error);
                return;
            }
        };
        match default_run_profile(&name) {
            Ok(record) => {
                self.profile.record = Some(record);
                self.profile.reviewed = None;
                self.profile.usable = None;
                self.profile.saved = None;
                self.profile.notice = Some(format!(
                    "Restored current canonical defaults for profile '{name}'; validate/review before saving"
                ));
            }
            Err(error) => {
                self.profile.notice = Some(format!(
                    "Could not restore canonical run-profile defaults: {error}"
                ));
            }
        }
    }

    pub(in crate::app) fn load_profile(&mut self) {
        self.profile.reviewed = None;
        self.profile.usable = None;
        self.profile.saved = None;
        let target = match self.profile_target() {
            Ok(target) => target,
            Err(error) => {
                self.profile.notice = Some(error);
                return;
            }
        };
        match load_run_profile(&target) {
            Ok(source) => {
                let preview = match preview_run_profile(&target, &source.record) {
                    Ok(preview) if preview.path == source.path => preview,
                    Ok(preview) => {
                        self.profile.notice = Some(format!(
                            "Loaded profile path '{}' did not match its reviewed path '{}'",
                            source.path.display(),
                            preview.path.display()
                        ));
                        return;
                    }
                    Err(error) => {
                        self.profile.notice =
                            Some(format!("Loaded profile failed production review: {error}"));
                        return;
                    }
                };
                self.profile.record = Some(source.record.clone());
                self.profile.notice = Some(format!(
                    "Loaded {} through the production run-profile parser; validate/review before saving",
                    source.path.display()
                ));
                self.profile.usable = Some(ProfileBinding { target, preview });
            }
            Err(error) => {
                self.profile.notice = Some(format!("Could not load selected profile: {error}"));
            }
        }
    }

    pub(in crate::app) fn review_profile(&mut self) {
        let target = match self.profile_target() {
            Ok(target) => target,
            Err(error) => {
                self.profile.notice = Some(error);
                return;
            }
        };
        let Some(record) = self.profile.record.clone() else {
            self.profile.notice = Some(
                "No run-profile draft is available; restore defaults or load a selected profile"
                    .to_string(),
            );
            return;
        };
        match preview_run_profile(&target, &record) {
            Ok(preview) => {
                self.profile.notice = Some(format!(
                    "Validated {} as {}",
                    preview.path.display(),
                    preview.content_hash
                ));
                self.profile.reviewed = Some(ProfileBinding { target, preview });
            }
            Err(error) => {
                self.profile.reviewed = None;
                self.profile.notice = Some(format!("Run-profile validation failed: {error}"));
            }
        }
    }

    pub(in crate::app) fn save_profile(&mut self) -> bool {
        let Some(reviewed) = self.profile.reviewed.clone() else {
            self.profile.notice =
                Some("Validate/review the exact profile before saving".to_string());
            return false;
        };
        if !self.profile_unchanged() {
            self.profile.notice = Some(
                "Profile target or draft changed after review; revalidate before saving"
                    .to_string(),
            );
            return false;
        }
        match save_run_profile(
            &reviewed.target,
            &reviewed.preview.record,
            &reviewed.preview.content_hash,
        ) {
            Ok(receipt) => {
                self.select_profile(&reviewed.target);
                self.profile.record = Some(receipt.saved.record.clone());
                self.profile.notice = Some(format!(
                    "Saved create-only profile {} as {} and selected it for setup",
                    receipt.saved.path.display(),
                    receipt.saved.content_hash
                ));
                self.profile.usable = Some(ProfileBinding {
                    target: reviewed.target,
                    preview: receipt.saved.clone(),
                });
                self.profile.saved = Some(receipt);
                true
            }
            Err(error) => {
                self.profile.notice = Some(format!("Could not save reviewed profile: {error}"));
                false
            }
        }
    }

    fn profile_target(&self) -> Result<RunSetupProfile, String> {
        match self.profile.kind {
            ProfileKind::Name => {
                required(&self.profile.value, "profile name").map(RunSetupProfile::Name)
            }
            ProfileKind::Path => {
                required_path(&self.profile.value, "profile path").map(RunSetupProfile::Path)
            }
        }
    }

    fn profile_name(&self) -> Result<String, String> {
        match self.profile_target()? {
            RunSetupProfile::Name(name) => Ok(name),
            RunSetupProfile::Path(path) => path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .filter(|stem| !stem.trim().is_empty())
                .map(str::to_string)
                .ok_or_else(|| {
                    format!(
                        "profile path '{}' has no UTF-8 file stem for the canonical profile name",
                        path.display()
                    )
                }),
        }
    }

    fn profile_unchanged(&self) -> bool {
        let Some(reviewed) = self.profile.reviewed.as_ref() else {
            return false;
        };
        self.profile.record.as_ref() == Some(&reviewed.preview.record)
            && self.profile_target().as_ref() == Ok(&reviewed.target)
    }

    pub(in crate::app) fn setup_profile_usable(&self) -> Result<(), String> {
        self.usable_profile().map(|_| ())
    }

    pub(in crate::app) fn matches_setup_profile(
        &self,
        preview: &RunSetupProfilePreview,
    ) -> Result<(), String> {
        let usable = self.usable_profile()?;
        if preview.source_path != usable.preview.path {
            return Err(format!(
                "setup preview resolved profile source '{}' but the editor loaded or saved '{}'",
                preview.source_path.display(),
                usable.preview.path.display()
            ));
        }
        if preview.profile_hash != usable.preview.content_hash {
            return Err(format!(
                "setup preview resolved profile hash '{}' but the editor loaded or saved '{}'",
                preview.profile_hash, usable.preview.content_hash
            ));
        }
        if preview.record != usable.preview.record {
            return Err(
                "setup preview resolved profile policy that differs from the displayed loaded or saved profile"
                    .to_string(),
            );
        }
        Ok(())
    }

    fn usable_profile(&self) -> Result<&ProfileBinding, String> {
        let usable = self.profile.usable.as_ref().ok_or_else(|| {
            "Load an existing profile or save a reviewed new profile before setup preview"
                .to_string()
        })?;
        let target = self.profile_target()?;
        if target != usable.target {
            return Err(
                "The selected profile target differs from the loaded or saved profile; load or save this target before setup preview"
                    .to_string(),
            );
        }
        if self.profile.record.as_ref() != Some(&usable.preview.record) {
            return Err(
                "The displayed profile has unsaved edits; save a reviewed new target or restore the loaded policy before setup preview"
                    .to_string(),
            );
        }
        Ok(usable)
    }

    fn profile_identity(&self) -> (ProfileKind, String, Option<RunProfileRecord>) {
        (
            self.profile.kind,
            self.profile.value.clone(),
            self.profile.record.clone(),
        )
    }

    fn select_profile(&mut self, target: &RunSetupProfile) {
        match target {
            RunSetupProfile::Name(name) => {
                self.profile.kind = ProfileKind::Name;
                self.profile.value.clone_from(name);
            }
            RunSetupProfile::Path(path) => {
                self.profile.kind = ProfileKind::Path;
                self.profile.value = path.display().to_string();
            }
        }
    }
}

pub(in crate::app) struct ReviewedSetup {
    pub(in crate::app) request: RunSetupRequest,
    pub(in crate::app) preview: RunSetupPreview,
}

pub(in crate::app) struct SetupPanel<'a> {
    pub(in crate::app) draft: &'a mut SetupDraft,
    pub(in crate::app) reviewed: Option<&'a ReviewedSetup>,
    pub(in crate::app) receipt: Option<&'a RunSetupReceipt>,
    pub(in crate::app) pending: bool,
    pub(in crate::app) operation_pending: bool,
}

#[derive(Debug, Default)]
pub(in crate::app) struct SetupAction {
    pub(in crate::app) profile_defaults: bool,
    pub(in crate::app) profile_load: bool,
    pub(in crate::app) profile_review: bool,
    pub(in crate::app) profile_save: bool,
    pub(in crate::app) profile_changed: bool,
    pub(in crate::app) preview: bool,
    pub(in crate::app) admit: bool,
}

impl SetupPanel<'_> {
    pub(in crate::app) fn show(self, ui: &mut Ui) -> SetupAction {
        let mut action = SetupAction::default();
        let profile_before = self.draft.profile_identity();
        ScrollArea::vertical()
            .id_salt("fresh_run_setup")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.heading("Fresh run setup");
                ui.label(
                    "Preview and admission use the same canonical setup service as the CLI. Preview is read-only; admission is bound to the reviewed plan hash.",
                );
                ui.add_space(8.0);

                Grid::new("fresh_setup_inputs")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        text_row(ui, "repository root", &mut self.draft.root, 520.0);
                        batch_row(ui, &mut self.draft.batch);
                        text_row(ui, "campaign", &mut self.draft.campaign, 300.0);
                        profile_row(ui, &mut self.draft.profile);
                        text_row(ui, "primary instance (optional)", &mut self.draft.primary, 360.0);
                    });

                ui.add_space(10.0);
                ui.separator();
                ui.heading("Run profile configuration");
                ui.label(
                    "The draft below is the canonical RunProfileRecord used by ploke-eval. Validation and create-only save run through the production profile service.",
                );
                let controls = profile_buttons(
                    ui,
                    self.pending,
                    self.draft.profile.record.is_some(),
                    self.draft.profile_unchanged(),
                );
                action.profile_defaults |= controls.profile_defaults;
                action.profile_load |= controls.profile_load;
                action.profile_review |= controls.profile_review;
                action.profile_save |= controls.profile_save;
                if let Some(notice) = self.draft.profile.notice.as_deref() {
                    ui.label(notice);
                }
                if self.draft.profile.reviewed.is_some() && !self.draft.profile_unchanged() {
                    ui.label(
                        RichText::new(
                            "Profile target or draft changed after review; revalidate before saving.",
                        )
                        .color(Color32::YELLOW),
                    );
                }
                match self.draft.profile.record.as_mut() {
                    Some(record) => show_profile_editor(ui, record),
                    None => {
                        ui.label(
                            RichText::new(
                                "No run-profile draft is available. Restore canonical defaults or load a selected profile.",
                            )
                            .color(Color32::YELLOW),
                        );
                    }
                }
                ui.add_space(8.0);
                ui.heading("Reviewed profile");
                match self.draft.profile.reviewed.as_ref() {
                    Some(reviewed) => render_profile(ui, &reviewed.preview),
                    None => {
                        ui.label(RichText::new("No validated profile review").color(Color32::GRAY));
                    }
                }
                ui.heading("Saved profile");
                match self.draft.profile.saved.as_ref() {
                    Some(receipt) => render_profile(ui, &receipt.saved),
                    None => {
                        ui.label(RichText::new("No profile saved in this session").color(Color32::GRAY));
                    }
                }
                ui.heading("Setup-usable profile");
                match self.draft.profile.usable.as_ref() {
                    Some(usable) if self.draft.setup_profile_usable().is_ok() => {
                        render_profile(ui, &usable.preview)
                    }
                    Some(_) => {
                        ui.label(
                            RichText::new(
                                "The displayed target or policy differs from the last loaded or saved profile.",
                            )
                            .color(Color32::YELLOW),
                        );
                    }
                    None => {
                        ui.label(
                            RichText::new(
                                "Load an existing profile or save a reviewed new profile before setup preview.",
                            )
                            .color(Color32::GRAY),
                        );
                    }
                }

                ui.add_space(8.0);
                egui::CollapsingHeader::new("Advanced typed overrides")
                    .id_salt("fresh_setup_advanced")
                    .default_open(self.draft.advanced)
                    .show(ui, |ui| {
                        self.draft.advanced = true;
                        override_grid(ui, self.draft);
                    });

                let current = self.draft.request();
                let usable = self.draft.setup_profile_usable();
                let unchanged = current.as_ref().ok().is_some_and(|request| {
                    self.reviewed
                        .is_some_and(|reviewed| reviewed.request == *request)
                });
                ui.add_space(10.0);
                let controls = setup_buttons(
                    ui,
                    self.pending,
                    self.operation_pending,
                    current.is_ok() && usable.is_ok(),
                    unchanged && usable.is_ok(),
                );
                action.preview |= controls.preview;
                action.admit |= controls.admit;
                match (current, usable) {
                    (Err(error), _) => {
                        ui.label(RichText::new(error).color(Color32::GRAY));
                    }
                    (Ok(_), Err(error)) => {
                        ui.label(RichText::new(error).color(Color32::GRAY));
                    }
                    (Ok(_), Ok(())) if self.reviewed.is_some() && !unchanged => {
                        ui.label(
                            RichText::new("Inputs changed after preview; review a new plan before admission.")
                                .color(Color32::YELLOW),
                        );
                    }
                    _ => {}
                }

                ui.separator();
                ui.heading("Reviewed plan");
                match self.reviewed {
                    Some(reviewed) => render_preview(ui, &reviewed.preview),
                    None => {
                        ui.label(RichText::new("No setup preview").color(Color32::GRAY));
                    }
                }

                ui.separator();
                ui.heading("Admission receipt");
                match self.receipt {
                    Some(receipt) => render_receipt(ui, receipt),
                    None => {
                        ui.label(RichText::new("No admitted run").color(Color32::GRAY));
                    }
                }
            });
        action.profile_changed = profile_before != self.draft.profile_identity();
        action
    }
}

fn profile_buttons(ui: &mut Ui, pending: bool, has_record: bool, unchanged: bool) -> SetupAction {
    let mut action = SetupAction::default();
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(!pending, egui::Button::new("Restore canonical defaults"))
            .clicked()
        {
            action.profile_defaults = true;
        }
        if ui
            .add_enabled(!pending, egui::Button::new("Load selected profile"))
            .clicked()
        {
            action.profile_load = true;
        }
        if ui
            .add_enabled(
                !pending && has_record,
                egui::Button::new("Validate / review profile"),
            )
            .clicked()
        {
            action.profile_review = true;
        }
        if ui
            .add_enabled(
                !pending && unchanged,
                egui::Button::new("Save reviewed profile"),
            )
            .on_hover_text(
                "Create-only save is enabled only while the target and typed draft exactly match the validated review",
            )
            .clicked()
        {
            action.profile_save = true;
        }
    });
    action
}

fn show_profile_editor(ui: &mut Ui, record: &mut RunProfileRecord) {
    profile_section(ui, "Profile identity", "profile_identity", true, |ui| {
        text_row(ui, "profile name", &mut record.name, 300.0);
        read_only_row(ui, "schema version", &record.schema_version);
    });

    profile_section(ui, "Storage / evaluation", "profile_storage", true, |ui| {
        path_row(
            ui,
            "worktree root",
            &mut record.storage.worktree_root,
            520.0,
        );
        choice_row(
            ui,
            "evaluation storage backend",
            &mut record.storage.eval.backend,
            &[
                (EvalStorageBackend::Fs, "filesystem"),
                (EvalStorageBackend::DbMirror, "database mirror"),
                (
                    EvalStorageBackend::Database,
                    "database (compatibility spelling)",
                ),
                (EvalStorageBackend::DualStrict, "dual strict"),
            ],
        );
    });

    profile_section(ui, "Target", "profile_target", true, |ui| {
        optional_text_row(ui, "dataset key", &mut record.target.dataset_key, 320.0);
        optional_text_row(ui, "primary instance", &mut record.target.instance, 420.0);
        instances_row(ui, &mut record.target.instances);
    });

    profile_section(ui, "Model", "profile_model", false, |ui| {
        model_rows(ui, &mut record.model, "evaluation");
    });

    profile_section(ui, "Search", "profile_search", true, |ui| {
        u32_row(ui, "max generations", &mut record.search.max_generations);
        u32_row(ui, "max total nodes", &mut record.search.max_total_nodes);
        u32_row(ui, "children minimum", &mut record.search.children.min);
        u32_row(ui, "children maximum", &mut record.search.children.max);
        optional_u32_row(
            ui,
            "parallel targets",
            &mut record.search.children.parallel_targets,
        );
        choice_row(
            ui,
            "child schedule",
            &mut record.search.schedule,
            &[
                (ChildScheduleModeRecord::FullBatch, "full batch"),
                (ChildScheduleModeRecord::AdaptiveBatch, "adaptive batch"),
            ],
        );
        bool_row(
            ui,
            "stop on first keep",
            &mut record.search.stop_on_first_keep,
        );
        bool_row(
            ui,
            "require keep for continuation",
            &mut record.search.require_keep_for_continuation,
        );
        bool_row(
            ui,
            "explore from rejected",
            &mut record.search.explore_from_rejected,
        );
    });

    profile_section(ui, "Generation", "profile_generation", false, |ui| {
        choice_row(
            ui,
            "generation source",
            &mut record.generation.source,
            &[
                (GenerationSource::Legacy, "legacy single-target"),
                (
                    GenerationSource::BroadHarnessRequest,
                    "broad harness request",
                ),
                (
                    GenerationSource::DeterministicTuiTools,
                    "deterministic TUI tools",
                ),
            ],
        );
    });

    profile_section(ui, "Selection", "profile_selection", false, |ui| {
        choice_row(
            ui,
            "selection strategy",
            &mut record.selection.strategy,
            &[
                (SelectionStrategy::GenerationLocal, "generation local"),
                (
                    SelectionStrategy::HistoryFrontierMax,
                    "history frontier maximum",
                ),
                (
                    SelectionStrategy::HistoryScoreChildProp,
                    "history score child proportion",
                ),
            ],
        );
        choice_row(
            ui,
            "selection evidence",
            &mut record.selection.evidence,
            &[
                (SelectionEvidence::Operational, "operational"),
                (
                    SelectionEvidence::OperationalAndProtocol,
                    "operational and protocol",
                ),
            ],
        );
        u64_row(ui, "selection seed", &mut record.selection.seed);
        bool_row(ui, "persist metrics", &mut record.selection.metrics.persist);
        read_only_row(
            ui,
            "score profile",
            match record.selection.metrics.score_profile {
                ScoreProfile::OperationalQualityV1 => "operational quality v1",
            },
        );
        bool_row(
            ui,
            "ImpAtK enabled",
            &mut record.selection.metrics.imp_at_k.enabled,
        );
        usize_row(
            ui,
            "ImpAtK budget",
            &mut record.selection.metrics.imp_at_k.budget_k,
        );
        read_only_row(
            ui,
            "ImpAtK archive scope",
            match record.selection.metrics.imp_at_k.archive_scope {
                ArchiveScope::SelectionScope => "selection scope",
            },
        );
        i64_row(
            ui,
            "ImpAtK score points",
            &mut record.selection.metrics.imp_at_k.score_points_per_imp_point,
        );
        bool_row(
            ui,
            "ImpAtK required for score",
            &mut record.selection.metrics.imp_at_k.require_for_score,
        );
        choice_row(
            ui,
            "oracle mode",
            &mut record.selection.oracle.mode,
            &[
                (OracleMode::RecordOnly, "record only"),
                (OracleMode::RelativeScore, "relative score"),
            ],
        );
        bool_row(
            ui,
            "oracle requires evidence",
            &mut record.selection.oracle.require_evidence,
        );
        choice_row(
            ui,
            "oracle gate",
            &mut record.selection.oracle.gate,
            &[
                (OracleGate::Disabled, "disabled"),
                (OracleGate::AllResolved, "all resolved"),
            ],
        );
        choice_row(
            ui,
            "patch gate",
            &mut record.selection.patch.gate,
            &[
                (PatchGate::Disabled, "disabled"),
                (PatchGate::ReviewedAdmissible, "reviewed admissible"),
            ],
        );
    });

    profile_section(ui, "Protocol", "profile_protocol", false, |ui| {
        model_rows(ui, &mut record.protocol.model, "protocol");
        u32_row(ui, "protocol max tokens", &mut record.protocol.max_tokens);
        usize_row(
            ui,
            "tool review parallelism",
            &mut record.protocol.tool_review_parallelism,
        );
        choice_row(
            ui,
            "reasoning mode",
            &mut record.protocol.reasoning.mode,
            &[
                (ProtocolReasoningMode::Auto, "auto"),
                (ProtocolReasoningMode::Omit, "omit"),
                (ProtocolReasoningMode::Effort, "effort"),
                (ProtocolReasoningMode::Disabled, "disabled"),
            ],
        );
        optional_choice_row(
            ui,
            "reasoning effort",
            &mut record.protocol.reasoning.effort,
            "not specified",
            &[
                (ProtocolReasoningEffort::Xhigh, "xhigh"),
                (ProtocolReasoningEffort::High, "high"),
                (ProtocolReasoningEffort::Medium, "medium"),
                (ProtocolReasoningEffort::Low, "low"),
                (ProtocolReasoningEffort::Minimal, "minimal"),
                (ProtocolReasoningEffort::None, "none"),
            ],
        );
    });

    profile_section(ui, "Execution", "profile_execution", false, |ui| {
        choice_row(
            ui,
            "execution stop after",
            &mut record.execution.stop_after,
            &[
                (ExecutionStopAfter::Materialize, "materialize"),
                (ExecutionStopAfter::Build, "build"),
                (ExecutionStopAfter::Spawn, "spawn"),
                (ExecutionStopAfter::Complete, "complete"),
            ],
        );
        u64_row(
            ui,
            "child stale seconds",
            &mut record.execution.child_stale_secs,
        );
        optional_u32_row(
            ui,
            "broad TUI max attempts",
            &mut record.execution.broad_tui.max_attempts,
        );
        optional_usize_row(
            ui,
            "broad TUI fresh slots per child",
            &mut record.execution.broad_tui.fresh_slots_per_child,
        );
        optional_usize_row(
            ui,
            "broad TUI graph nearest",
            &mut record.execution.broad_tui.graph_nearest,
        );
        optional_u64_row(
            ui,
            "broad TUI timeout seconds",
            &mut record.execution.broad_tui.timeout_secs,
        );
        choice_row(
            ui,
            "trace JSONL",
            &mut record.execution.trace_jsonl,
            &[
                (TraceJsonl::Inherit, "inherit"),
                (TraceJsonl::Auto, "auto"),
                (TraceJsonl::Off, "off"),
            ],
        );
        bool_row(ui, "debug tools", &mut record.execution.debug_tools);
        bool_row(ui, "MBE enabled", &mut record.execution.mbe.enabled);
        text_row(ui, "MBE Python", &mut record.execution.mbe.python, 300.0);
        u32_row(ui, "MBE workers", &mut record.execution.mbe.workers);
    });

    profile_section(ui, "Control", "profile_control", true, |ui| {
        choice_row(
            ui,
            "control mode",
            &mut record.control.mode,
            &[(RunMode::Continuous, "continuous"), (RunMode::Step, "step")],
        );
        optional_u32_row(ui, "control parallel cap", &mut record.control.parallel_cap);
    });
}

fn profile_section(
    ui: &mut Ui,
    title: &str,
    id: &'static str,
    open: bool,
    rows: impl FnOnce(&mut Ui),
) {
    egui::CollapsingHeader::new(title)
        .id_salt(id)
        .default_open(open)
        .show(ui, |ui| {
            Grid::new(id)
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, rows);
        });
}

fn model_rows(ui: &mut Ui, model: &mut ModelDefaults, role: &str) {
    optional_text_row(ui, &format!("{role} model id"), &mut model.id, 380.0);
    optional_choice_row(
        ui,
        &format!("{role} model route"),
        &mut model.route_source,
        "not specified",
        &[
            (ProfileRouteSource::OpenRouter, "openrouter"),
            (ProfileRouteSource::DirectGoogle, "direct Google"),
        ],
    );
    optional_text_row(
        ui,
        &format!("{role} model provider"),
        &mut model.provider,
        260.0,
    );
}

fn choice_row<T: Copy + PartialEq>(ui: &mut Ui, label: &str, value: &mut T, choices: &[(T, &str)]) {
    ui.label(label);
    let selected = choices
        .iter()
        .find(|(choice, _)| choice == value)
        .map(|(_, name)| *name)
        .unwrap_or("unsupported persisted value");
    ComboBox::from_id_salt(("profile_choice", label))
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for (choice, name) in choices {
                ui.selectable_value(value, *choice, *name);
            }
        });
    ui.end_row();
}

fn optional_choice_row<T: Copy + PartialEq>(
    ui: &mut Ui,
    label: &str,
    value: &mut Option<T>,
    empty: &str,
    choices: &[(T, &str)],
) {
    ui.label(label);
    let selected = match value {
        Some(value) => choices
            .iter()
            .find(|(choice, _)| choice == value)
            .map(|(_, name)| *name)
            .unwrap_or("unsupported persisted value"),
        None => empty,
    };
    ComboBox::from_id_salt(("profile_optional_choice", label))
        .selected_text(selected)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, None, empty);
            for (choice, name) in choices {
                ui.selectable_value(value, Some(*choice), *name);
            }
        });
    ui.end_row();
}

fn optional_text_row(ui: &mut Ui, label: &str, value: &mut Option<String>, width: f32) {
    ui.label(label);
    ui.horizontal(|ui| {
        let mut enabled = value.is_some();
        if ui.checkbox(&mut enabled, "set").changed() {
            *value = enabled.then(String::new);
        }
        if let Some(value) = value {
            ui.add_sized([width, 22.0], TextEdit::singleline(value));
        }
    });
    ui.end_row();
}

fn instances_row(ui: &mut Ui, values: &mut Vec<String>) {
    ui.label("target instances");
    ui.vertical(|ui| {
        let mut remove = None;
        for (index, value) in values.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add_sized([420.0, 22.0], TextEdit::singleline(value));
                if ui
                    .button(format!("Remove instance {}", index + 1))
                    .clicked()
                {
                    remove = Some(index);
                }
            });
        }
        if let Some(index) = remove {
            values.remove(index);
        }
        if ui.button("Add target instance").clicked() {
            values.push(String::new());
        }
    });
    ui.end_row();
}

fn path_row(ui: &mut Ui, label: &str, value: &mut PathBuf, width: f32) {
    ui.label(label);
    let mut text = value.display().to_string();
    if ui
        .add_sized([width, 22.0], TextEdit::singleline(&mut text))
        .changed()
    {
        *value = PathBuf::from(text);
    }
    ui.end_row();
}

fn read_only_row(ui: &mut Ui, label: &str, value: &str) {
    ui.label(label);
    ui.label(value);
    ui.end_row();
}

fn bool_row(ui: &mut Ui, label: &str, value: &mut bool) {
    ui.label(label);
    ui.checkbox(value, "");
    ui.end_row();
}

fn u32_row(ui: &mut Ui, label: &str, value: &mut u32) {
    ui.label(label);
    ui.add(DragValue::new(value).range(0..=u32::MAX));
    ui.end_row();
}

fn u64_row(ui: &mut Ui, label: &str, value: &mut u64) {
    ui.label(label);
    ui.add(DragValue::new(value).range(0..=u64::MAX));
    ui.end_row();
}

fn usize_row(ui: &mut Ui, label: &str, value: &mut usize) {
    ui.label(label);
    ui.add(DragValue::new(value).range(0..=usize::MAX));
    ui.end_row();
}

fn i64_row(ui: &mut Ui, label: &str, value: &mut i64) {
    ui.label(label);
    ui.add(DragValue::new(value));
    ui.end_row();
}

fn optional_u32_row(ui: &mut Ui, label: &str, value: &mut Option<u32>) {
    ui.label(label);
    ui.horizontal(|ui| {
        let mut enabled = value.is_some();
        if ui.checkbox(&mut enabled, "set").changed() {
            *value = enabled.then_some(0);
        }
        if let Some(value) = value {
            ui.add(DragValue::new(value).range(0..=u32::MAX));
        }
    });
    ui.end_row();
}

fn optional_u64_row(ui: &mut Ui, label: &str, value: &mut Option<u64>) {
    ui.label(label);
    ui.horizontal(|ui| {
        let mut enabled = value.is_some();
        if ui.checkbox(&mut enabled, "set").changed() {
            *value = enabled.then_some(0);
        }
        if let Some(value) = value {
            ui.add(DragValue::new(value).range(0..=u64::MAX));
        }
    });
    ui.end_row();
}

fn optional_usize_row(ui: &mut Ui, label: &str, value: &mut Option<usize>) {
    ui.label(label);
    ui.horizontal(|ui| {
        let mut enabled = value.is_some();
        if ui.checkbox(&mut enabled, "set").changed() {
            *value = enabled.then_some(0);
        }
        if let Some(value) = value {
            ui.add(DragValue::new(value).range(0..=usize::MAX));
        }
    });
    ui.end_row();
}

fn render_profile(ui: &mut Ui, preview: &ProfilePreview) {
    ui.monospace(format!(
        "target: {}\nsha256: {}\ncontrol mode: {:?}\nparallel cap: {} ({})\npatch cap: {} ({})",
        preview.path.display(),
        preview.content_hash,
        preview.control.mode,
        preview.control.parallel_cap.value,
        source_label(preview.control.parallel_cap.source),
        preview.control.patch_cap.value,
        source_label(preview.control.patch_cap.source),
    ));
    render_value(
        ui,
        "Normalized typed profile",
        serde_json::to_value(&preview.record),
    );
}

fn setup_buttons(
    ui: &mut Ui,
    pending: bool,
    operation_pending: bool,
    can_preview: bool,
    unchanged: bool,
) -> SetupAction {
    let mut action = SetupAction::default();
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(!pending && can_preview, egui::Button::new("Preview setup"))
            .clicked()
        {
            action.preview = true;
        }
        if ui
            .add_enabled(
                !pending && !operation_pending && unchanged,
                egui::Button::new("Admit reviewed setup"),
            )
            .on_hover_text(
                "Enabled only while every current input exactly matches the reviewed request",
            )
            .clicked()
        {
            action.admit = true;
        }
        if pending {
            ui.label(RichText::new("setup request…").color(Color32::YELLOW));
        } else if operation_pending {
            ui.label(
                RichText::new(
                    "stop the live walk authority before admission; preview remains available",
                )
                .color(Color32::YELLOW),
            );
        }
    });
    action
}

fn batch_row(ui: &mut Ui, draft: &mut BatchDraft) {
    ui.label("prepared batch");
    ui.horizontal(|ui| {
        ComboBox::from_id_salt("setup_batch_kind")
            .selected_text(match draft.kind {
                BatchKind::Id => "batch id",
                BatchKind::Manifest => "manifest path",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut draft.kind, BatchKind::Id, "batch id");
                ui.selectable_value(&mut draft.kind, BatchKind::Manifest, "manifest path");
            });
        ui.add_sized([390.0, 22.0], TextEdit::singleline(&mut draft.value));
    });
    ui.end_row();
}

fn profile_row(ui: &mut Ui, draft: &mut ProfileDraft) {
    ui.label("run profile");
    ui.horizontal(|ui| {
        ComboBox::from_id_salt("setup_profile_kind")
            .selected_text(match draft.kind {
                ProfileKind::Name => "registered name",
                ProfileKind::Path => "TOML path",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut draft.kind, ProfileKind::Name, "registered name");
                ui.selectable_value(&mut draft.kind, ProfileKind::Path, "TOML path");
            });
        ui.add_sized([390.0, 22.0], TextEdit::singleline(&mut draft.value));
    });
    ui.end_row();
}

fn override_grid(ui: &mut Ui, draft: &mut SetupDraft) {
    ui.label("Blank values inherit the selected profile or canonical setup defaults.");
    Grid::new("fresh_setup_overrides")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            text_row(ui, "eval model", &mut draft.model.id, 360.0);
            model_route_row(ui, "eval route", &mut draft.model.route);
            text_row(ui, "eval provider", &mut draft.model.provider, 240.0);
            optional_tokens_row(ui, &mut draft.model.tokens);
            ui.label("canonical default model");
            ui.checkbox(&mut draft.model.use_default, "request canonical default");
            ui.end_row();

            text_row(ui, "protocol model", &mut draft.protocol.id, 360.0);
            model_route_row(ui, "protocol route", &mut draft.protocol.route);
            text_row(ui, "protocol provider", &mut draft.protocol.provider, 240.0);

            text_row(ui, "embedding model", &mut draft.embedding.id, 360.0);
            embedding_route_row(ui, &mut draft.embedding.route);
            text_row(
                ui,
                "embedding provider",
                &mut draft.embedding.provider,
                240.0,
            );
        });
}

fn model_route_row(ui: &mut Ui, label: &str, route: &mut Option<ModelRouteSource>) {
    ui.label(label);
    ComboBox::from_id_salt(label)
        .selected_text(match route {
            None => "inherit",
            Some(ModelRouteSource::OpenRouter) => "openrouter",
            Some(ModelRouteSource::DirectGoogle) => "direct google",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(route, None, "inherit");
            ui.selectable_value(route, Some(ModelRouteSource::OpenRouter), "openrouter");
            ui.selectable_value(route, Some(ModelRouteSource::DirectGoogle), "direct google");
        });
    ui.end_row();
}

fn embedding_route_row(ui: &mut Ui, route: &mut Option<EmbeddingRoute>) {
    ui.label("embedding route");
    ComboBox::from_id_salt("setup_embedding_route")
        .selected_text(match route {
            None => "inherit",
            Some(EmbeddingRoute::OpenRouter) => "openrouter",
            Some(EmbeddingRoute::DirectOpenAi) => "direct OpenAI",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(route, None, "inherit");
            ui.selectable_value(route, Some(EmbeddingRoute::OpenRouter), "openrouter");
            ui.selectable_value(route, Some(EmbeddingRoute::DirectOpenAi), "direct OpenAI");
        });
    ui.end_row();
}

fn optional_tokens_row(ui: &mut Ui, tokens: &mut Option<u32>) {
    ui.label("eval max tokens");
    ui.horizontal(|ui| {
        let mut enabled = tokens.is_some();
        if ui.checkbox(&mut enabled, "override").changed() {
            *tokens = enabled.then_some(1);
        }
        if let Some(tokens) = tokens {
            ui.add(DragValue::new(tokens).range(1..=u32::MAX));
        }
    });
    ui.end_row();
}

fn text_row(ui: &mut Ui, label: &str, value: &mut String, width: f32) {
    ui.label(label);
    ui.add_sized([width, 22.0], TextEdit::singleline(value));
    ui.end_row();
}

fn render_preview(ui: &mut Ui, preview: &RunSetupPreview) {
    ui.monospace(format!(
        "plan sha256: {}\ncheckout: {} @ {}\nartifact branch: {}\nbatch manifest: {}\nprimary instance: {}\ncampaign manifest: {}\ncampaign sha256: {}\nslice: {}\nslice sha256: {}\nprofile: {}\nprofile sha256: {}\ncontrol mode: {:?}\nparallel cap: {} ({})\npatch cap: {} ({})",
        preview.plan_hash,
        preview.checkout.branch,
        preview.checkout.head,
        preview.artifact_branch,
        preview.batch.manifest_path.display(),
        preview.batch.primary_instance,
        preview.campaign.manifest_path.display(),
        preview.campaign.manifest_hash,
        preview.campaign.slice_path.display(),
        preview.campaign.slice_hash,
        preview.profile.profile_path.display(),
        preview.profile.profile_hash,
        preview.control.mode,
        preview.control.parallel_cap.value,
        source_label(preview.control.parallel_cap.source),
        preview.control.patch_cap.value,
        source_label(preview.control.patch_cap.source),
    ));
    render_value(
        ui,
        "Prepared cohort",
        serde_json::to_value(&preview.batch.batch),
    );
    render_value(
        ui,
        "Campaign manifest",
        serde_json::to_value(&preview.campaign.manifest),
    );
    render_value(
        ui,
        "Resolved campaign",
        serde_json::to_value(&preview.campaign.resolved),
    );
    render_value(
        ui,
        "Validated profile",
        serde_json::to_value(&preview.profile.record),
    );
}

fn render_receipt(ui: &mut Ui, receipt: &RunSetupReceipt) {
    ui.monospace(format!(
        "plan sha256: {}\ncheckout: {}\ncampaign: {}\nparent: {}\nnode: {}\nsetup receipt: {}\nprofile: {}\ncontrol mode: {:?}",
        receipt.plan_hash,
        receipt.repo_root.display(),
        receipt.config.identity.record.campaign_id,
        receipt.config.identity.record.parent_id,
        receipt.config.identity.record.node_id,
        receipt.config.campaign.admission.path.display(),
        receipt.config.profile.record.name,
        receipt.config.control.mode,
    ));
}

fn render_value(ui: &mut Ui, label: &str, value: Result<serde_json::Value, serde_json::Error>) {
    egui::CollapsingHeader::new(label)
        .id_salt(label)
        .show(ui, |ui| {
            let rendered = value
                .and_then(|value| serde_json::to_string_pretty(&value))
                .unwrap_or_else(|error| format!("rendering failed: {error}"));
            ui.monospace(rendered);
        });
}

fn source_label(source: ValueSource) -> String {
    match source {
        ValueSource::Explicit(field) => format!("explicit {field:?}"),
        ValueSource::Derived(rule) => format!("derived {rule:?}"),
    }
}

fn required(value: &str, label: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        Err(format!("{label} is required"))
    } else {
        Ok(value.to_string())
    }
}

fn required_path(value: &str, label: &str) -> Result<PathBuf, String> {
    required(value, label).map(PathBuf::from)
}

fn optional(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn canonical_defaults_initialize_the_typed_draft() {
        let draft = SetupDraft::for_root(Path::new("/tmp/worktree"));
        let expected = default_run_profile("new-run").expect("canonical defaults");

        assert_eq!(draft.profile.record.as_ref(), Some(&expected));
        assert_eq!(draft.profile.value, "new-run");
        assert!(
            draft
                .profile
                .notice
                .as_deref()
                .is_some_and(|notice| notice.contains("canonical run-profile defaults"))
        );
    }

    #[test]
    fn restoring_defaults_uses_the_selected_target_name() {
        let mut draft = SetupDraft::for_root(Path::new("/tmp/worktree"));
        draft.profile.value = "v37".to_string();
        draft
            .profile
            .record
            .as_mut()
            .expect("initial typed draft")
            .name = "stale-draft-name".to_string();

        draft.restore_profile();

        assert_eq!(
            draft
                .profile
                .record
                .as_ref()
                .expect("restored typed draft")
                .name,
            "v37"
        );
    }

    #[test]
    fn typed_edits_reach_the_production_profile_review() {
        let path = temp_profile("exact-edit");
        let mut draft = SetupDraft::for_root(Path::new("/tmp/worktree"));
        draft.profile.kind = ProfileKind::Path;
        draft.profile.value = path.display().to_string();
        let record = draft.profile.record.as_mut().expect("initial typed draft");
        record.name = "exact-edit".to_string();
        record.target.instances = vec!["instance-a".to_string(), "instance-b".to_string()];
        record.search.max_generations = 7;
        record.selection.seed = 41;
        record.protocol.tool_review_parallelism = 5;
        let expected = record.clone();

        draft.review_profile();

        let reviewed = draft.profile.reviewed.as_ref().expect("profile review");
        assert_eq!(reviewed.preview.record, expected);
        assert!(!path.exists(), "review must not write the target");
    }

    #[test]
    fn invalid_numeric_draft_fails_production_validation() {
        let mut draft = SetupDraft::for_root(Path::new("/tmp/worktree"));
        draft
            .profile
            .record
            .as_mut()
            .expect("initial typed draft")
            .search
            .children
            .min = 0;

        draft.review_profile();

        assert!(draft.profile.reviewed.is_none());
        assert!(
            draft
                .profile
                .notice
                .as_deref()
                .is_some_and(|notice| notice.contains("must be nonzero")),
            "production validation should remain visible: {:?}",
            draft.profile.notice
        );
    }

    #[test]
    fn profile_drift_requires_a_new_review() {
        let path = temp_profile("review-drift");
        let mut draft = SetupDraft::for_root(Path::new("/tmp/worktree"));
        draft.profile.kind = ProfileKind::Path;
        draft.profile.value = path.display().to_string();
        draft.review_profile();
        assert!(draft.profile_unchanged());

        draft
            .profile
            .record
            .as_mut()
            .expect("reviewed typed draft")
            .selection
            .seed += 1;

        assert!(!draft.profile_unchanged());
        assert!(!draft.save_profile());
        assert!(
            draft
                .profile
                .notice
                .as_deref()
                .is_some_and(|notice| notice.contains("revalidate"))
        );
        assert!(!path.exists(), "drifted review must not write the target");
    }

    #[test]
    fn create_only_save_selects_a_setup_usable_profile() {
        let path = temp_profile("create-only");
        let mut draft = SetupDraft::for_root(Path::new("/tmp/worktree"));
        draft.batch.value = "batch-a".to_string();
        draft.campaign = "campaign-a".to_string();
        draft.profile.kind = ProfileKind::Path;
        draft.profile.value = path.display().to_string();
        draft
            .profile
            .record
            .as_mut()
            .expect("initial typed draft")
            .name = "create-only".to_string();

        draft.review_profile();
        assert!(draft.save_profile());

        assert_eq!(
            draft.request().expect("setup request").profile,
            RunSetupProfile::Path(path.clone())
        );
        let saved = draft.profile.saved.as_ref().expect("profile receipt");
        assert_eq!(saved.saved.path, path);
        assert!(saved.saved.path.exists());
        std::fs::remove_file(&saved.saved.path).expect("remove created profile");
    }

    #[test]
    fn unsaved_edits_block_setup_after_loading() {
        let path = temp_profile("loaded-authority");
        let mut creator = SetupDraft::for_root(Path::new("/tmp/worktree"));
        creator.profile.kind = ProfileKind::Path;
        creator.profile.value = path.display().to_string();
        creator
            .profile
            .record
            .as_mut()
            .expect("initial typed draft")
            .name = "loaded-authority".to_string();
        creator.review_profile();
        assert!(creator.save_profile());

        let mut loaded = SetupDraft::for_root(Path::new("/tmp/worktree"));
        loaded.profile.kind = ProfileKind::Path;
        loaded.profile.value = path.display().to_string();
        loaded.load_profile();
        assert!(loaded.setup_profile_usable().is_ok());

        loaded
            .profile
            .record
            .as_mut()
            .expect("loaded typed draft")
            .search
            .max_generations += 1;

        let error = loaded
            .setup_profile_usable()
            .expect_err("unsaved edit must block setup preview");
        assert!(error.contains("unsaved edits"), "unexpected error: {error}");
        std::fs::remove_file(path).expect("remove created profile");
    }

    #[test]
    fn failed_reload_clears_the_setup_binding() {
        let path = temp_profile("failed-reload");
        let mut draft = SetupDraft::for_root(Path::new("/tmp/worktree"));
        draft.profile.kind = ProfileKind::Path;
        draft.profile.value = path.display().to_string();
        draft.review_profile();
        assert!(draft.save_profile());
        assert!(draft.setup_profile_usable().is_ok());
        std::fs::remove_file(&path).expect("remove created profile");

        draft.load_profile();

        assert!(draft.setup_profile_usable().is_err());
        assert!(
            draft
                .profile
                .notice
                .as_deref()
                .is_some_and(|notice| notice.contains("Could not load selected profile"))
        );
    }

    #[test]
    fn returned_setup_profile_must_match_the_editor_binding() {
        let path = temp_profile("returned-binding");
        let mut draft = SetupDraft::for_root(Path::new("/tmp/worktree"));
        draft.profile.kind = ProfileKind::Path;
        draft.profile.value = path.display().to_string();
        draft.review_profile();
        assert!(draft.save_profile());
        let usable = draft.profile.usable.as_ref().expect("setup-usable binding");
        let mut returned = RunSetupProfilePreview {
            source_path: usable.preview.path.clone(),
            profile_path: PathBuf::from("/tmp/worktree/prototype1/run-profile.toml"),
            profile_hash: usable.preview.content_hash.clone(),
            record: usable.preview.record.clone(),
        };

        draft
            .matches_setup_profile(&returned)
            .expect("exact returned setup profile");
        returned.record.selection.seed += 1;
        let error = draft
            .matches_setup_profile(&returned)
            .expect_err("external profile drift must reject setup preview");
        assert!(error.contains("differs from the displayed"));
        std::fs::remove_file(path).expect("remove created profile");
    }

    #[test]
    fn kittest_exposes_profile_sections_and_actions() {
        use egui_kittest::{Harness, kittest::Queryable};

        let mut record = default_run_profile("ui-sections").expect("canonical defaults");
        let harness = Harness::builder()
            .with_size(egui::Vec2::new(1280.0, 4000.0))
            .build_ui(|ui| {
                profile_buttons(ui, false, true, false);
                show_profile_editor(ui, &mut record);
            });

        for label in [
            "Restore canonical defaults",
            "Load selected profile",
            "Validate / review profile",
            "Save reviewed profile",
            "Profile identity",
            "Storage / evaluation",
            "Target",
            "Model",
            "Search",
            "Generation",
            "Selection",
            "Protocol",
            "Execution",
            "Control",
        ] {
            harness.get_by_label(label);
        }
    }

    #[test]
    fn admission_requires_an_unchanged_reviewed_request() {
        let mut draft = SetupDraft::for_root(Path::new("/tmp/worktree"));
        draft.batch.value = "batch-a".to_string();
        draft.campaign = "campaign-a".to_string();
        draft.profile.value = "profile-a".to_string();
        let request = draft.request().expect("complete typed request");

        assert_eq!(draft.request().expect("same request"), request);
        draft.primary = "instance-b".to_string();
        assert_ne!(draft.request().expect("changed request"), request);
    }

    #[test]
    fn advanced_routes_remain_typed() {
        let mut draft = SetupDraft::for_root(Path::new("/tmp/worktree"));
        draft.batch.value = "batch-a".to_string();
        draft.campaign = "campaign-a".to_string();
        draft.profile.value = "profile-a".to_string();
        draft.model.route = Some(ModelRouteSource::DirectGoogle);
        draft.embedding.route = Some(EmbeddingRoute::DirectOpenAi);

        let request = draft.request().expect("typed request");
        assert_eq!(request.model.route, Some(ModelRouteSource::DirectGoogle));
        assert_eq!(request.embedding.route, Some(EmbeddingRoute::DirectOpenAi));
    }

    #[test]
    fn kittest_admit_requires_matching_preview() {
        use egui_kittest::{
            Harness,
            kittest::{NodeT, Queryable},
        };

        let harness = Harness::builder().build_ui(|ui| {
            setup_buttons(ui, false, false, true, false);
        });
        assert!(
            !harness
                .get_by_label("Preview setup")
                .accesskit_node()
                .is_disabled()
        );
        assert!(
            harness
                .get_by_label("Admit reviewed setup")
                .accesskit_node()
                .is_disabled()
        );

        let harness = Harness::builder().build_ui(|ui| {
            setup_buttons(ui, false, false, true, true);
        });
        assert!(
            !harness
                .get_by_label("Admit reviewed setup")
                .accesskit_node()
                .is_disabled()
        );
    }

    #[test]
    fn kittest_live_operation_allows_preview_but_disables_admission() {
        use egui_kittest::{
            Harness,
            kittest::{NodeT, Queryable},
        };

        let harness = Harness::builder().build_ui(|ui| {
            setup_buttons(ui, false, true, true, true);
        });

        assert!(
            !harness
                .get_by_label("Preview setup")
                .accesskit_node()
                .is_disabled(),
            "read-only preview remains available while live work owns the checkout"
        );
        assert!(
            harness
                .get_by_label("Admit reviewed setup")
                .accesskit_node()
                .is_disabled(),
            "admission must be disabled while live work owns the checkout"
        );
        harness.get_by_label(
            "stop the live walk authority before admission; preview remains available",
        );
    }

    fn temp_profile(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "ploke-walk-ui-{label}-{}-{nonce}.toml",
            std::process::id()
        ))
    }
}
