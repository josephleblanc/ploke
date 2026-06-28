use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use cozo::DataValue;
use ploke_db::QueryResult;
use ploke_protocol::ProtocolReasoningPolicy;
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

use crate::{
    CampaignManifest, EvalCampaignPolicy, ProtocolCampaignPolicy,
    cli::prototype1_state::{
        identity::ParentIdentity,
        profile::{AdmittedRunProfile, EvalStorageBackend},
    },
    closure::ClosureState,
    intervention::CompleteBaseline,
    spec::{EvalBudget, FrameworkConfig, FrameworkToolConfig},
    target_registry::{BenchmarkFamily, RegistryDatasetSource},
};

use super::{
    cozo_store::EvalDb,
    error::EvalStoreError,
    schema::{EvalRelationSchema, define_eval_schema, put_eval_params},
};

define_eval_schema!(CampaignSchema {
    "eval_campaign",
    campaign_id: "String" =>
    schema_version: "String",
    manifest_ref: "String",
    prototype_root: "String",
    manifest_sha256: "String",
    profile_ref_id: "String?",
    storage_backend: "String?",
    benchmark_family: "String",
    dataset_sources: "[[String?;4]]",
    model_id: "String?",
    provider_slug: "String?",
    route_source: "String?",
    required_procedures: "[String]",
    instances_root: "String?",
    batches_root: "String?",
    framework_tools: "[[String?;2]]",
    ingested_at: "String",
});

// Campaign list fields are stored as Cozo lists on their owning rows. Keep the
// manifest path/hash on `eval_campaign` as provenance, but make campaign config
// queryable without reading the manifest file.
define_eval_schema!(CampaignEvalPolicySchema {
    "eval_campaign_eval_policy",
    campaign_id: "String" =>
    include_partial: "Bool",
    stop_on_error: "Bool",
    limit_count: "Int?",
    include_dataset_labels: "[String]",
    exclude_dataset_labels: "[String]",
    batch_prefix: "String?",
    embedding_model_id: "String?",
    embedding_provider_slug: "String?",
    ingested_at: "String",
});

define_eval_schema!(CampaignEvalBudgetSchema {
    "eval_campaign_eval_budget",
    campaign_id: "String" =>
    max_turns: "Int",
    max_tool_calls: "Int",
    wall_clock_secs: "Int",
    ingested_at: "String",
});

define_eval_schema!(CampaignProtocolPolicySchema {
    "eval_campaign_protocol_policy",
    campaign_id: "String" =>
    model_id: "String?",
    provider_slug: "String?",
    route_source: "String?",
    include_partial: "Bool",
    include_incompatible: "Bool",
    include_failed: "Bool",
    stop_on_error: "Bool",
    limit_count: "Int?",
    max_concurrency: "Int",
    tool_review_parallelism: "Int",
    max_tokens: "Int",
    reasoning_mode: "String",
    reasoning_effort: "String?",
    ingested_at: "String",
});

define_eval_schema!(ProfileCommitmentSchema {
    "eval_profile_commitment",
    profile_ref_id: "String" =>
    campaign_id: "String",
    schema_version: "String",
    profile_name: "String",
    source_ref: "String",
    profile_path: "String",
    content_sha256: "String",
    source_path: "String?",
    admitted_at: "String",
    storage_ref: "String",
    ingested_at: "String",
});

define_eval_schema!(RunProfilePolicySchema {
    "eval_run_profile_policy",
    campaign_id: "String" =>
    profile_ref_id: "String",
    schema_version: "String",
    max_generations: "Int",
    max_total_nodes: "Int",
    child_min: "Int",
    child_max: "Int",
    parallel_targets: "Int?",
    schedule_mode: "String",
    stop_first_keep: "Bool",
    require_keep: "Bool",
    explore_rejected: "Bool",
    generation_source: "String",
    selection_strategy: "String",
    selection_evidence: "String",
    selection_seed: "Int",
    metrics_persist: "Bool",
    score_profile: "String",
    imp_enabled: "Bool",
    imp_budget_k: "Int",
    imp_archive: "String",
    imp_score_points: "Int",
    imp_required: "Bool",
    oracle_mode: "String",
    oracle_required: "Bool",
    stop_after: "String",
    observe_stale_secs: "Int",
    trace_jsonl: "String",
    debug_tools: "Bool",
    broad_max_attempts: "Int?",
    fresh_slots: "Int?",
    graph_nearest: "Int?",
    timeout_secs: "Int?",
    control_mode: "String",
    parallel_cap: "Int?",
    ingested_at: "String",
});

define_eval_schema!(ClosureRefSchema {
    "eval_closure_ref",
    closure_ref_id: "String" =>
    campaign_id: "String",
    run_id: "String?",
    store_scope: "String",
    source_ref: "String",
    content_sha256: "String",
    schema_version: "String",
    recorded_at: "String",
    ingested_at: "String",
});

define_eval_schema!(ClosureInstanceSchema {
    "eval_closure_instance",
    closure_ref_id: "String",
    instance_id: "String" =>
    campaign_id: "String",
    dataset_label: "String",
    repo_family: "String",
    registry_status: "String",
    eval_status: "String",
    protocol_status: "String",
    eval_failure: "String?",
    protocol_failure: "String?",
    last_event_at: "String?",
    recorded_at: "String",
    ingested_at: "String",
});

define_eval_schema!(ClosureArtifactRefSchema {
    "eval_closure_artifact_ref",
    closure_ref_id: "String",
    instance_id: "String",
    artifact_kind: "String",
    artifact_index: "Int" =>
    campaign_id: "String",
    path: "String",
    recorded_at: "String",
    ingested_at: "String",
});

define_eval_schema!(ClosureProtocolProcedureSchema {
    "eval_closure_protocol_procedure",
    closure_ref_id: "String",
    instance_id: "String",
    procedure: "String" =>
    campaign_id: "String",
    status: "String",
    recorded_at: "String",
    ingested_at: "String",
});

define_eval_schema!(ClosureProtocolCountsSchema {
    "eval_closure_protocol_counts",
    closure_ref_id: "String",
    instance_id: "String" =>
    campaign_id: "String",
    total_calls: "Int",
    reviewed_calls: "Int",
    total_segments: "Int",
    usable_segments: "Int",
    mismatched_segments: "Int",
    missing_segments: "Int",
    recorded_at: "String",
    ingested_at: "String",
});

define_eval_schema!(BaselineSchema {
    "eval_baseline",
    baseline_id: "String" =>
    campaign_id: "String",
    parent_id: "String",
    parent_node_id: "String",
    parent_branch_id: "String",
    source_kind: "String",
    closure_ref_id: "String?",
    evaluation_id: "String?",
    record_ref: "String?",
    eval_set_id: "String",
    status: "String",
    recorded_at: "String",
    ingested_at: "String",
});

define_eval_schema!(BaselineInstanceSchema {
    "eval_baseline_instance",
    baseline_id: "String",
    instance_id: "String" =>
    campaign_id: "String",
    parent_node_id: "String",
    parent_branch_id: "String",
    eval_set_id: "String",
    registration_path: "String?",
    record_path: "String",
    recorded_at: "String",
    ingested_at: "String",
});

define_eval_schema!(BaselineInstanceMetricsSchema {
    "eval_baseline_instance_metrics",
    baseline_id: "String",
    instance_id: "String" =>
    campaign_id: "String",
    tool_calls_total: "Int",
    tool_calls_failed: "Int",
    patch_attempted: "Bool",
    apply_state: "String",
    submission_state: "String",
    projection_state: "String",
    patch_failures: "Int",
    same_file_retries: "Int",
    same_file_streak: "Int",
    aborted: "Bool",
    repair_aborted: "Bool",
    valid_patch: "Bool",
    convergence: "Bool",
    oracle_eligible: "Bool",
    recorded_at: "String",
    ingested_at: "String",
});

pub(crate) const CAMPAIGN_REL: &str = CampaignSchema::RELATION;
pub(crate) const CAMPAIGN_EVAL_POLICY_REL: &str = CampaignEvalPolicySchema::RELATION;
pub(crate) const CAMPAIGN_EVAL_BUDGET_REL: &str = CampaignEvalBudgetSchema::RELATION;
pub(crate) const CAMPAIGN_PROTOCOL_POLICY_REL: &str = CampaignProtocolPolicySchema::RELATION;
pub(crate) const PROFILE_COMMITMENT_REL: &str = ProfileCommitmentSchema::RELATION;
pub(crate) const RUN_PROFILE_POLICY_REL: &str = RunProfilePolicySchema::RELATION;
pub(crate) const CLOSURE_REF_REL: &str = ClosureRefSchema::RELATION;
pub(crate) const CLOSURE_INSTANCE_REL: &str = ClosureInstanceSchema::RELATION;
pub(crate) const CLOSURE_ARTIFACT_REF_REL: &str = ClosureArtifactRefSchema::RELATION;
pub(crate) const CLOSURE_PROTOCOL_PROCEDURE_REL: &str = ClosureProtocolProcedureSchema::RELATION;
pub(crate) const CLOSURE_PROTOCOL_COUNTS_REL: &str = ClosureProtocolCountsSchema::RELATION;
pub(crate) const BASELINE_REL: &str = BaselineSchema::RELATION;
pub(crate) const BASELINE_INSTANCE_REL: &str = BaselineInstanceSchema::RELATION;
pub(crate) const BASELINE_INSTANCE_METRICS_REL: &str = BaselineInstanceMetricsSchema::RELATION;

pub(super) fn ensure_setup_schema<D: EvalDb + ?Sized>(db: &D) -> Result<(), EvalStoreError> {
    CampaignSchema::SCHEMA.ensure_installed(db, "schema.eval_campaign")?;
    CampaignEvalPolicySchema::SCHEMA.ensure_installed(db, "schema.eval_campaign_eval_policy")?;
    CampaignEvalBudgetSchema::SCHEMA.ensure_installed(db, "schema.eval_campaign_eval_budget")?;
    CampaignProtocolPolicySchema::SCHEMA
        .ensure_installed(db, "schema.eval_campaign_protocol_policy")?;
    ProfileCommitmentSchema::SCHEMA.ensure_installed(db, "schema.eval_profile_commitment")?;
    RunProfilePolicySchema::SCHEMA.ensure_installed(db, "schema.eval_run_profile_policy")?;
    ClosureRefSchema::SCHEMA.ensure_installed(db, "schema.eval_closure_ref")?;
    ClosureInstanceSchema::SCHEMA.ensure_installed(db, "schema.eval_closure_instance")?;
    ClosureArtifactRefSchema::SCHEMA.ensure_installed(db, "schema.eval_closure_artifact_ref")?;
    ClosureProtocolProcedureSchema::SCHEMA
        .ensure_installed(db, "schema.eval_closure_protocol_procedure")?;
    ClosureProtocolCountsSchema::SCHEMA
        .ensure_installed(db, "schema.eval_closure_protocol_counts")?;
    BaselineSchema::SCHEMA.ensure_installed(db, "schema.eval_baseline")?;
    BaselineInstanceSchema::SCHEMA.ensure_installed(db, "schema.eval_baseline_instance")?;
    BaselineInstanceMetricsSchema::SCHEMA
        .ensure_installed(db, "schema.eval_baseline_instance_metrics")?;
    Ok(())
}

pub(super) fn put_profile_commitment<D: EvalDb + ?Sized>(
    db: &D,
    campaign_id: &ploke_records::ids::CampaignId,
    admitted: &AdmittedRunProfile,
) -> Result<String, EvalStoreError> {
    let commitment = &admitted.commitment;
    let profile_ref_id = profile_ref_id(campaign_id, admitted);
    let storage_ref = serde_json::to_string(&admitted.profile.storage).map_err(|source| {
        EvalStoreError::Validation {
            field: "eval_profile_commitment.storage_ref",
            detail: source.to_string(),
        }
    })?;
    let mut params = BTreeMap::new();
    params.insert("profile_ref_id".to_string(), profile_ref_id.clone().into());
    params.insert("campaign_id".to_string(), campaign_id.to_string().into());
    params.insert(
        "schema_version".to_string(),
        commitment.schema_version.clone().into(),
    );
    params.insert(
        "profile_name".to_string(),
        admitted.profile.name.clone().into(),
    );
    params.insert(
        "source_ref".to_string(),
        commitment.profile_path.display().to_string().into(),
    );
    params.insert(
        "profile_path".to_string(),
        commitment.profile_path.display().to_string().into(),
    );
    params.insert(
        "content_sha256".to_string(),
        commitment.sha256.clone().into(),
    );
    params.insert(
        "source_path".to_string(),
        option_string_param(
            commitment
                .source_path
                .as_ref()
                .map(|path| path.display().to_string()),
        ),
    );
    params.insert(
        "admitted_at".to_string(),
        commitment.admitted_at.clone().into(),
    );
    params.insert("storage_ref".to_string(), storage_ref.into());
    params.insert(
        "ingested_at".to_string(),
        chrono::Utc::now().to_rfc3339().into(),
    );

    put_eval_params(
        db,
        &ProfileCommitmentSchema::SCHEMA,
        params,
        "put.eval_profile_commitment",
    )?;

    Ok(profile_ref_id)
}

impl CampaignManifest {
    pub(crate) fn put_into_eval_db<D: EvalDb + ?Sized>(
        &self,
        db: &D,
        manifest_path: &Path,
        storage_backend: EvalStorageBackend,
        profile_ref_id: Option<&str>,
    ) -> Result<(), EvalStoreError> {
        let manifest_sha256 = file_sha256(manifest_path, "eval_campaign.manifest_ref")?;
        let prototype_root = manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("prototype1");
        let ingested_at = chrono::Utc::now().to_rfc3339();
        let mut params = BTreeMap::new();
        params.insert(
            "campaign_id".to_string(),
            self.campaign_id.to_string().into(),
        );
        params.insert(
            "schema_version".to_string(),
            self.schema_version.clone().into(),
        );
        params.insert(
            "manifest_ref".to_string(),
            manifest_path.display().to_string().into(),
        );
        params.insert(
            "prototype_root".to_string(),
            prototype_root.display().to_string().into(),
        );
        params.insert("manifest_sha256".to_string(), manifest_sha256.into());
        params.insert(
            "profile_ref_id".to_string(),
            option_string_param(profile_ref_id.map(str::to_string)),
        );
        params.insert(
            "storage_backend".to_string(),
            storage_backend.label().to_string().into(),
        );
        params.insert(
            "benchmark_family".to_string(),
            enum_string(&self.benchmark_family, "eval_campaign.benchmark_family")?.into(),
        );
        params.insert(
            "dataset_sources".to_string(),
            dataset_sources_param(&self.dataset_sources),
        );
        params.insert(
            "model_id".to_string(),
            option_string_param(self.model_id.clone()),
        );
        params.insert(
            "provider_slug".to_string(),
            option_string_param(self.provider_slug.clone()),
        );
        params.insert(
            "route_source".to_string(),
            option_enum_string_param(self.route_source.as_ref(), "eval_campaign.route_source")?,
        );
        params.insert(
            "required_procedures".to_string(),
            string_list_param(&self.required_procedures),
        );
        params.insert(
            "instances_root".to_string(),
            option_path_param(self.instances_root.as_deref()),
        );
        params.insert(
            "batches_root".to_string(),
            option_path_param(self.batches_root.as_deref()),
        );
        params.insert(
            "framework_tools".to_string(),
            framework_tools_param(&self.framework),
        );
        params.insert("ingested_at".to_string(), ingested_at.clone().into());

        put_eval_params(db, &CampaignSchema::SCHEMA, params, "put.eval_campaign")?;
        put_campaign_eval_rows(db, self, &ingested_at)?;
        put_campaign_protocol_row(db, self, &ingested_at)?;

        Ok(())
    }

    pub(crate) fn read_from_eval_db<D: EvalDb + ?Sized>(
        db: &D,
        campaign_id: &ploke_records::ids::CampaignId,
    ) -> Result<Self, EvalStoreError> {
        read_campaign_manifest(db, campaign_id)
    }
}

pub(super) fn put_campaign_manifest<D: EvalDb + ?Sized>(
    db: &D,
    manifest_path: &Path,
    manifest: &CampaignManifest,
    storage_backend: EvalStorageBackend,
    profile_ref_id: Option<&str>,
) -> Result<(), EvalStoreError> {
    manifest.put_into_eval_db(db, manifest_path, storage_backend, profile_ref_id)
}

pub(super) fn put_run_profile_policy<D: EvalDb + ?Sized>(
    db: &D,
    campaign_id: &ploke_records::ids::CampaignId,
    profile_ref_id: &str,
    admitted: &AdmittedRunProfile,
) -> Result<(), EvalStoreError> {
    let profile = &admitted.profile;
    let search = &profile.search;
    let child = search.children;
    let selection = profile.selection;
    let metrics = selection.metrics;
    let imp = metrics.imp_at_k;
    let execution = &profile.execution;
    let broad = execution.broad_tui;
    let control = profile.control;

    let mut params = BTreeMap::new();
    params.insert("campaign_id".to_string(), campaign_id.to_string().into());
    params.insert(
        "profile_ref_id".to_string(),
        profile_ref_id.to_string().into(),
    );
    params.insert(
        "schema_version".to_string(),
        admitted.profile.schema_version.clone().into(),
    );
    params.insert(
        "max_generations".to_string(),
        i64::from(search.max_generations).into(),
    );
    params.insert(
        "max_total_nodes".to_string(),
        i64::from(search.max_total_nodes).into(),
    );
    params.insert("child_min".to_string(), i64::from(child.min).into());
    params.insert("child_max".to_string(), i64::from(child.max).into());
    params.insert(
        "parallel_targets".to_string(),
        option_u32_param(child.parallel_targets),
    );
    params.insert(
        "schedule_mode".to_string(),
        enum_string(&search.schedule, "eval_run_profile_policy.schedule_mode")?.into(),
    );
    params.insert(
        "stop_first_keep".to_string(),
        DataValue::Bool(search.stop_on_first_keep),
    );
    params.insert(
        "require_keep".to_string(),
        DataValue::Bool(search.require_keep_for_continuation),
    );
    params.insert(
        "explore_rejected".to_string(),
        DataValue::Bool(search.explore_from_rejected),
    );
    params.insert(
        "generation_source".to_string(),
        enum_string(
            &profile.generation.source,
            "eval_run_profile_policy.generation_source",
        )?
        .into(),
    );
    params.insert(
        "selection_strategy".to_string(),
        enum_string(
            &selection.strategy,
            "eval_run_profile_policy.selection_strategy",
        )?
        .into(),
    );
    params.insert(
        "selection_evidence".to_string(),
        enum_string(
            &selection.evidence,
            "eval_run_profile_policy.selection_evidence",
        )?
        .into(),
    );
    params.insert(
        "selection_seed".to_string(),
        u64_to_i64(selection.seed, "eval_run_profile_policy.selection_seed")?.into(),
    );
    params.insert(
        "metrics_persist".to_string(),
        DataValue::Bool(metrics.persist),
    );
    params.insert(
        "score_profile".to_string(),
        enum_string(
            &metrics.score_profile,
            "eval_run_profile_policy.score_profile",
        )?
        .into(),
    );
    params.insert("imp_enabled".to_string(), DataValue::Bool(imp.enabled));
    params.insert(
        "imp_budget_k".to_string(),
        usize_to_i64(imp.budget_k, "eval_run_profile_policy.imp_budget_k")?.into(),
    );
    params.insert(
        "imp_archive".to_string(),
        enum_string(&imp.archive_scope, "eval_run_profile_policy.imp_archive")?.into(),
    );
    params.insert(
        "imp_score_points".to_string(),
        imp.score_points_per_imp_point.into(),
    );
    params.insert(
        "imp_required".to_string(),
        DataValue::Bool(imp.require_for_score),
    );
    params.insert(
        "oracle_mode".to_string(),
        enum_string(
            &selection.oracle.mode,
            "eval_run_profile_policy.oracle_mode",
        )?
        .into(),
    );
    params.insert(
        "oracle_required".to_string(),
        DataValue::Bool(selection.oracle.require_evidence),
    );
    params.insert(
        "stop_after".to_string(),
        enum_string(&execution.stop_after, "eval_run_profile_policy.stop_after")?.into(),
    );
    params.insert(
        "observe_stale_secs".to_string(),
        u64_to_i64(
            execution.observe_child_stale_after_secs,
            "eval_run_profile_policy.observe_stale_secs",
        )?
        .into(),
    );
    params.insert(
        "trace_jsonl".to_string(),
        enum_string(
            &execution.trace_jsonl,
            "eval_run_profile_policy.trace_jsonl",
        )?
        .into(),
    );
    params.insert(
        "debug_tools".to_string(),
        DataValue::Bool(execution.debug_tools),
    );
    params.insert(
        "broad_max_attempts".to_string(),
        option_u32_param(broad.max_attempts),
    );
    params.insert(
        "fresh_slots".to_string(),
        option_usize_param(
            broad.fresh_slots_per_child,
            "eval_run_profile_policy.fresh_slots",
        )?,
    );
    params.insert(
        "graph_nearest".to_string(),
        option_usize_param(broad.graph_nearest, "eval_run_profile_policy.graph_nearest")?,
    );
    params.insert(
        "timeout_secs".to_string(),
        option_u64_param(broad.timeout_secs, "eval_run_profile_policy.timeout_secs")?,
    );
    params.insert(
        "control_mode".to_string(),
        enum_string(&control.mode, "eval_run_profile_policy.control_mode")?.into(),
    );
    params.insert(
        "parallel_cap".to_string(),
        option_u32_param(control.parallel_cap),
    );
    params.insert(
        "ingested_at".to_string(),
        chrono::Utc::now().to_rfc3339().into(),
    );

    put_eval_params(
        db,
        &RunProfilePolicySchema::SCHEMA,
        params,
        "put.eval_run_profile_policy",
    )
}

fn put_campaign_eval_rows<D: EvalDb + ?Sized>(
    db: &D,
    manifest: &CampaignManifest,
    ingested_at: &str,
) -> Result<(), EvalStoreError> {
    let eval = &manifest.eval;
    let mut params = BTreeMap::new();
    params.insert(
        "campaign_id".to_string(),
        manifest.campaign_id.to_string().into(),
    );
    params.insert(
        "include_partial".to_string(),
        DataValue::Bool(eval.include_partial),
    );
    params.insert(
        "stop_on_error".to_string(),
        DataValue::Bool(eval.stop_on_error),
    );
    params.insert(
        "limit_count".to_string(),
        option_usize_param(eval.limit, "eval_campaign_eval_policy.limit_count")?,
    );
    params.insert(
        "include_dataset_labels".to_string(),
        string_list_param(&eval.include_dataset_labels),
    );
    params.insert(
        "exclude_dataset_labels".to_string(),
        string_list_param(&eval.exclude_dataset_labels),
    );
    params.insert(
        "batch_prefix".to_string(),
        option_string_param(eval.batch_prefix.clone()),
    );
    params.insert(
        "embedding_model_id".to_string(),
        option_string_param(eval.embedding_model_id.clone()),
    );
    params.insert(
        "embedding_provider_slug".to_string(),
        option_string_param(eval.embedding_provider_slug.clone()),
    );
    params.insert("ingested_at".to_string(), ingested_at.to_string().into());
    put_eval_params(
        db,
        &CampaignEvalPolicySchema::SCHEMA,
        params,
        "put.eval_campaign_eval_policy",
    )?;

    let budget = &eval.budget;
    let mut params = BTreeMap::new();
    params.insert(
        "campaign_id".to_string(),
        manifest.campaign_id.to_string().into(),
    );
    params.insert("max_turns".to_string(), i64::from(budget.max_turns).into());
    params.insert(
        "max_tool_calls".to_string(),
        i64::from(budget.max_tool_calls).into(),
    );
    params.insert(
        "wall_clock_secs".to_string(),
        i64::from(budget.wall_clock_secs).into(),
    );
    params.insert("ingested_at".to_string(), ingested_at.to_string().into());
    put_eval_params(
        db,
        &CampaignEvalBudgetSchema::SCHEMA,
        params,
        "put.eval_campaign_eval_budget",
    )
}

fn put_campaign_protocol_row<D: EvalDb + ?Sized>(
    db: &D,
    manifest: &CampaignManifest,
    ingested_at: &str,
) -> Result<(), EvalStoreError> {
    let protocol = &manifest.protocol;
    let mut params = BTreeMap::new();
    params.insert(
        "campaign_id".to_string(),
        manifest.campaign_id.to_string().into(),
    );
    params.insert(
        "model_id".to_string(),
        option_string_param(protocol.model_id.clone()),
    );
    params.insert(
        "provider_slug".to_string(),
        option_string_param(protocol.provider_slug.clone()),
    );
    params.insert(
        "route_source".to_string(),
        option_enum_string_param(
            protocol.route_source.as_ref(),
            "eval_campaign_protocol_policy.route_source",
        )?,
    );
    params.insert(
        "include_partial".to_string(),
        DataValue::Bool(protocol.include_partial),
    );
    params.insert(
        "include_incompatible".to_string(),
        DataValue::Bool(protocol.include_incompatible),
    );
    params.insert(
        "include_failed".to_string(),
        DataValue::Bool(protocol.include_failed),
    );
    params.insert(
        "stop_on_error".to_string(),
        DataValue::Bool(protocol.stop_on_error),
    );
    params.insert(
        "limit_count".to_string(),
        option_usize_param(
            protocol.limit_runs,
            "eval_campaign_protocol_policy.limit_count",
        )?,
    );
    params.insert(
        "max_concurrency".to_string(),
        usize_to_i64(
            protocol.max_concurrency,
            "eval_campaign_protocol_policy.max_concurrency",
        )?
        .into(),
    );
    params.insert(
        "tool_review_parallelism".to_string(),
        usize_to_i64(
            protocol.tool_review_parallelism,
            "eval_campaign_protocol_policy.tool_review_parallelism",
        )?
        .into(),
    );
    params.insert(
        "max_tokens".to_string(),
        i64::from(protocol.max_tokens).into(),
    );
    params.insert(
        "reasoning_mode".to_string(),
        enum_string(
            &protocol.reasoning.mode,
            "eval_campaign_protocol_policy.reasoning_mode",
        )?
        .into(),
    );
    params.insert(
        "reasoning_effort".to_string(),
        option_enum_string_param(
            protocol.reasoning.effort.as_ref(),
            "eval_campaign_protocol_policy.reasoning_effort",
        )?,
    );
    params.insert("ingested_at".to_string(), ingested_at.to_string().into());
    put_eval_params(
        db,
        &CampaignProtocolPolicySchema::SCHEMA,
        params,
        "put.eval_campaign_protocol_policy",
    )
}

struct CampaignHead {
    schema_version: String,
    benchmark: BenchmarkFamily,
    dataset_sources: Vec<RegistryDatasetSource>,
    model_id: Option<String>,
    provider_slug: Option<String>,
    route_source: Option<ploke_llm::request::models::ModelRouteSource>,
    required_procedures: Vec<String>,
    instances_root: Option<PathBuf>,
    batches_root: Option<PathBuf>,
    framework: FrameworkConfig,
}

fn read_campaign_manifest<D: EvalDb + ?Sized>(
    db: &D,
    campaign_id: &ploke_records::ids::CampaignId,
) -> Result<CampaignManifest, EvalStoreError> {
    let head = read_campaign_head(db, campaign_id)?;
    let mut eval = read_campaign_eval(db, campaign_id)?;
    eval.budget = read_campaign_budget(db, campaign_id)?;

    Ok(CampaignManifest {
        schema_version: head.schema_version,
        campaign_id: campaign_id.clone(),
        benchmark_family: head.benchmark,
        dataset_sources: head.dataset_sources,
        model_id: head.model_id,
        provider_slug: head.provider_slug,
        route_source: head.route_source,
        required_procedures: head.required_procedures,
        instances_root: head.instances_root,
        batches_root: head.batches_root,
        eval,
        protocol: read_campaign_protocol(db, campaign_id)?,
        framework: head.framework,
    })
}

fn read_campaign_head<D: EvalDb + ?Sized>(
    db: &D,
    campaign_id: &ploke_records::ids::CampaignId,
) -> Result<CampaignHead, EvalStoreError> {
    let rows = query_campaign_rows(
        db,
        r#"
?[schema_version, benchmark_family, dataset_sources, model_id, provider_slug, route_source, required_procedures, instances_root, batches_root, framework_tools] :=
    *eval_campaign { campaign_id, schema_version, benchmark_family, dataset_sources, model_id, provider_slug, route_source, required_procedures, instances_root, batches_root, framework_tools },
    campaign_id = $campaign_id
"#,
        campaign_id,
        "read.eval_campaign",
    )?;
    let row = single_row(&rows, "eval_campaign")?;
    Ok(CampaignHead {
        schema_version: read_string(&rows, row, "schema_version")?,
        benchmark: parse_enum(
            read_string(&rows, row, "benchmark_family")?,
            "eval_campaign.benchmark_family",
        )?,
        dataset_sources: read_dataset_sources(&rows, row, "dataset_sources")?,
        model_id: read_optional_string(&rows, row, "model_id")?,
        provider_slug: read_optional_string(&rows, row, "provider_slug")?,
        route_source: parse_optional_enum(
            read_optional_string(&rows, row, "route_source")?,
            "eval_campaign.route_source",
        )?,
        required_procedures: read_string_list(&rows, row, "required_procedures")?,
        instances_root: read_optional_string(&rows, row, "instances_root")?.map(PathBuf::from),
        batches_root: read_optional_string(&rows, row, "batches_root")?.map(PathBuf::from),
        framework: read_framework_tools(&rows, row, "framework_tools")?,
    })
}

fn read_campaign_eval<D: EvalDb + ?Sized>(
    db: &D,
    campaign_id: &ploke_records::ids::CampaignId,
) -> Result<EvalCampaignPolicy, EvalStoreError> {
    let rows = query_campaign_rows(
        db,
        r#"
?[include_partial, stop_on_error, limit_count, include_dataset_labels, exclude_dataset_labels, batch_prefix, embedding_model_id, embedding_provider_slug] :=
    *eval_campaign_eval_policy { campaign_id, include_partial, stop_on_error, limit_count, include_dataset_labels, exclude_dataset_labels, batch_prefix, embedding_model_id, embedding_provider_slug },
    campaign_id = $campaign_id
"#,
        campaign_id,
        "read.eval_campaign_eval_policy",
    )?;
    let row = single_row(&rows, "eval_campaign_eval_policy")?;
    Ok(EvalCampaignPolicy {
        include_partial: read_bool(&rows, row, "include_partial")?,
        stop_on_error: read_bool(&rows, row, "stop_on_error")?,
        limit: read_optional_usize(&rows, row, "limit_count")?,
        include_dataset_labels: read_string_list(&rows, row, "include_dataset_labels")?,
        exclude_dataset_labels: read_string_list(&rows, row, "exclude_dataset_labels")?,
        budget: EvalBudget::default(),
        batch_prefix: read_optional_string(&rows, row, "batch_prefix")?,
        embedding_model_id: read_optional_string(&rows, row, "embedding_model_id")?,
        embedding_provider_slug: read_optional_string(&rows, row, "embedding_provider_slug")?,
    })
}

fn read_campaign_budget<D: EvalDb + ?Sized>(
    db: &D,
    campaign_id: &ploke_records::ids::CampaignId,
) -> Result<EvalBudget, EvalStoreError> {
    let rows = query_campaign_rows(
        db,
        r#"
?[max_turns, max_tool_calls, wall_clock_secs] :=
    *eval_campaign_eval_budget { campaign_id, max_turns, max_tool_calls, wall_clock_secs },
    campaign_id = $campaign_id
"#,
        campaign_id,
        "read.eval_campaign_eval_budget",
    )?;
    let row = single_row(&rows, "eval_campaign_eval_budget")?;
    Ok(EvalBudget {
        max_turns: read_u32(&rows, row, "max_turns")?,
        max_tool_calls: read_u32(&rows, row, "max_tool_calls")?,
        wall_clock_secs: read_u32(&rows, row, "wall_clock_secs")?,
    })
}

fn read_campaign_protocol<D: EvalDb + ?Sized>(
    db: &D,
    campaign_id: &ploke_records::ids::CampaignId,
) -> Result<ProtocolCampaignPolicy, EvalStoreError> {
    let rows = query_campaign_rows(
        db,
        r#"
?[model_id, provider_slug, route_source, include_partial, include_incompatible, include_failed, stop_on_error, limit_count, max_concurrency, tool_review_parallelism, max_tokens, reasoning_mode, reasoning_effort] :=
    *eval_campaign_protocol_policy { campaign_id, model_id, provider_slug, route_source, include_partial, include_incompatible, include_failed, stop_on_error, limit_count, max_concurrency, tool_review_parallelism, max_tokens, reasoning_mode, reasoning_effort },
    campaign_id = $campaign_id
"#,
        campaign_id,
        "read.eval_campaign_protocol_policy",
    )?;
    let row = single_row(&rows, "eval_campaign_protocol_policy")?;
    let reasoning = ProtocolReasoningPolicy {
        mode: parse_enum(
            read_string(&rows, row, "reasoning_mode")?,
            "eval_campaign_protocol_policy.reasoning_mode",
        )?,
        effort: parse_optional_enum(
            read_optional_string(&rows, row, "reasoning_effort")?,
            "eval_campaign_protocol_policy.reasoning_effort",
        )?,
    };
    reasoning
        .validate()
        .map_err(|detail| EvalStoreError::Validation {
            field: "eval_campaign_protocol_policy.reasoning",
            detail,
        })?;
    Ok(ProtocolCampaignPolicy {
        model_id: read_optional_string(&rows, row, "model_id")?,
        provider_slug: read_optional_string(&rows, row, "provider_slug")?,
        route_source: parse_optional_enum(
            read_optional_string(&rows, row, "route_source")?,
            "eval_campaign_protocol_policy.route_source",
        )?,
        include_partial: read_bool(&rows, row, "include_partial")?,
        include_incompatible: read_bool(&rows, row, "include_incompatible")?,
        include_failed: read_bool(&rows, row, "include_failed")?,
        stop_on_error: read_bool(&rows, row, "stop_on_error")?,
        limit_runs: read_optional_usize(&rows, row, "limit_count")?,
        max_concurrency: read_usize(&rows, row, "max_concurrency")?,
        tool_review_parallelism: read_usize(&rows, row, "tool_review_parallelism")?,
        max_tokens: read_u32(&rows, row, "max_tokens")?,
        reasoning,
    })
}

fn query_campaign_rows<D: EvalDb + ?Sized>(
    db: &D,
    script: &str,
    campaign_id: &ploke_records::ids::CampaignId,
    phase: &'static str,
) -> Result<QueryResult, EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert("campaign_id".to_string(), campaign_id.to_string().into());
    db.eval_query_params(script, params)
        .map_err(|source| EvalStoreError::Db { phase, source })
}

fn single_row<'a>(
    rows: &'a QueryResult,
    relation: &'static str,
) -> Result<&'a [DataValue], EvalStoreError> {
    match rows.rows.len() {
        1 => Ok(rows.rows[0].as_slice()),
        0 => Err(EvalStoreError::Validation {
            field: relation,
            detail: "missing required campaign row".to_string(),
        }),
        count => Err(EvalStoreError::Validation {
            field: relation,
            detail: format!("expected exactly one campaign row, found {count}"),
        }),
    }
}

fn field_value<'a>(
    rows: &QueryResult,
    row: &'a [DataValue],
    field: &'static str,
) -> Result<&'a DataValue, EvalStoreError> {
    let index = rows
        .headers
        .iter()
        .position(|header| header == field)
        .ok_or_else(|| EvalStoreError::Validation {
            field,
            detail: "missing query column".to_string(),
        })?;
    row.get(index).ok_or_else(|| EvalStoreError::Validation {
        field,
        detail: format!("missing value at query column {index}"),
    })
}

fn read_string(
    rows: &QueryResult,
    row: &[DataValue],
    field: &'static str,
) -> Result<String, EvalStoreError> {
    match field_value(rows, row, field)? {
        DataValue::Str(value) => Ok(value.to_string()),
        other => Err(type_error(field, "String", other)),
    }
}

fn read_optional_string(
    rows: &QueryResult,
    row: &[DataValue],
    field: &'static str,
) -> Result<Option<String>, EvalStoreError> {
    match field_value(rows, row, field)? {
        DataValue::Null => Ok(None),
        DataValue::Str(value) => Ok(Some(value.to_string())),
        other => Err(type_error(field, "String?", other)),
    }
}

fn read_string_list(
    rows: &QueryResult,
    row: &[DataValue],
    field: &'static str,
) -> Result<Vec<String>, EvalStoreError> {
    match field_value(rows, row, field)? {
        DataValue::List(values) => values
            .iter()
            .map(|value| string_value(value, field))
            .collect(),
        other => Err(type_error(field, "[String]", other)),
    }
}

fn read_dataset_sources(
    rows: &QueryResult,
    row: &[DataValue],
    field: &'static str,
) -> Result<Vec<RegistryDatasetSource>, EvalStoreError> {
    let values = match field_value(rows, row, field)? {
        DataValue::List(values) => values,
        other => return Err(type_error(field, "[[String?;4]]", other)),
    };
    let mut sources = Vec::with_capacity(values.len());
    for value in values {
        let columns = match value {
            DataValue::List(columns) if columns.len() == 4 => columns,
            other => return Err(type_error(field, "[String?;4]", other)),
        };
        sources.push(RegistryDatasetSource {
            key: optional_string_value(&columns[0], field)?,
            path: PathBuf::from(string_value(&columns[1], field)?),
            label: string_value(&columns[2], field)?,
            url: optional_string_value(&columns[3], field)?,
        });
    }
    Ok(sources)
}

fn read_framework_tools(
    rows: &QueryResult,
    row: &[DataValue],
    field: &'static str,
) -> Result<FrameworkConfig, EvalStoreError> {
    let values = match field_value(rows, row, field)? {
        DataValue::List(values) => values,
        other => return Err(type_error(field, "[[String?;2]]", other)),
    };
    let mut tools = BTreeMap::new();
    for value in values {
        let columns = match value {
            DataValue::List(columns) if columns.len() == 2 => columns,
            other => return Err(type_error(field, "[String?;2]", other)),
        };
        tools.insert(
            string_value(&columns[0], field)?,
            FrameworkToolConfig {
                version: optional_string_value(&columns[1], field)?,
            },
        );
    }
    Ok(FrameworkConfig { tools })
}

fn string_value(value: &DataValue, field: &'static str) -> Result<String, EvalStoreError> {
    match value {
        DataValue::Str(value) => Ok(value.to_string()),
        other => Err(type_error(field, "String", other)),
    }
}

fn optional_string_value(
    value: &DataValue,
    field: &'static str,
) -> Result<Option<String>, EvalStoreError> {
    match value {
        DataValue::Null => Ok(None),
        DataValue::Str(value) => Ok(Some(value.to_string())),
        other => Err(type_error(field, "String?", other)),
    }
}

fn read_bool(
    rows: &QueryResult,
    row: &[DataValue],
    field: &'static str,
) -> Result<bool, EvalStoreError> {
    match field_value(rows, row, field)? {
        DataValue::Bool(value) => Ok(*value),
        other => Err(type_error(field, "Bool", other)),
    }
}

fn read_i64(
    rows: &QueryResult,
    row: &[DataValue],
    field: &'static str,
) -> Result<i64, EvalStoreError> {
    match field_value(rows, row, field)? {
        DataValue::Num(cozo::Num::Int(value)) => Ok(*value),
        other => Err(type_error(field, "Int", other)),
    }
}

fn read_usize(
    rows: &QueryResult,
    row: &[DataValue],
    field: &'static str,
) -> Result<usize, EvalStoreError> {
    i64_to_usize(read_i64(rows, row, field)?, field)
}

fn read_optional_usize(
    rows: &QueryResult,
    row: &[DataValue],
    field: &'static str,
) -> Result<Option<usize>, EvalStoreError> {
    match field_value(rows, row, field)? {
        DataValue::Null => Ok(None),
        DataValue::Num(cozo::Num::Int(value)) => i64_to_usize(*value, field).map(Some),
        other => Err(type_error(field, "Int?", other)),
    }
}

fn read_u32(
    rows: &QueryResult,
    row: &[DataValue],
    field: &'static str,
) -> Result<u32, EvalStoreError> {
    u32::try_from(read_i64(rows, row, field)?).map_err(|_| EvalStoreError::Validation {
        field,
        detail: "value does not fit in u32".to_string(),
    })
}

fn parse_enum<T: DeserializeOwned>(
    value: String,
    field: &'static str,
) -> Result<T, EvalStoreError> {
    serde_json::from_value(serde_json::Value::String(value)).map_err(|source| {
        EvalStoreError::Validation {
            field,
            detail: source.to_string(),
        }
    })
}

fn parse_optional_enum<T: DeserializeOwned>(
    value: Option<String>,
    field: &'static str,
) -> Result<Option<T>, EvalStoreError> {
    value.map(|value| parse_enum(value, field)).transpose()
}

fn i64_to_usize(value: i64, field: &'static str) -> Result<usize, EvalStoreError> {
    usize::try_from(value).map_err(|_| EvalStoreError::Validation {
        field,
        detail: format!("value {value} does not fit in usize"),
    })
}

fn type_error(field: &'static str, expected: &str, found: &DataValue) -> EvalStoreError {
    EvalStoreError::Validation {
        field,
        detail: format!("expected {expected}, got {found:?}"),
    }
}

pub(super) fn put_closure_ref<D: EvalDb + ?Sized>(
    db: &D,
    closure_path: &Path,
    state: &ClosureState,
) -> Result<String, EvalStoreError> {
    let content_sha256 = file_sha256(closure_path, "eval_closure_ref.source_ref")?;
    let closure_ref_id =
        closure_ref_id_from_parts(&state.campaign_id, closure_path, &content_sha256);
    let ingested_at = chrono::Utc::now().to_rfc3339();
    let mut params = BTreeMap::new();
    params.insert("closure_ref_id".to_string(), closure_ref_id.clone().into());
    params.insert(
        "campaign_id".to_string(),
        state.campaign_id.to_string().into(),
    );
    params.insert("run_id".to_string(), DataValue::Null);
    params.insert("store_scope".to_string(), "campaign".into());
    params.insert(
        "source_ref".to_string(),
        closure_path.display().to_string().into(),
    );
    params.insert("content_sha256".to_string(), content_sha256.into());
    params.insert(
        "schema_version".to_string(),
        state.schema_version.clone().into(),
    );
    params.insert("recorded_at".to_string(), state.updated_at.clone().into());
    params.insert("ingested_at".to_string(), ingested_at.clone().into());

    put_eval_params(
        db,
        &ClosureRefSchema::SCHEMA,
        params,
        "put.eval_closure_ref",
    )?;
    put_closure_instance_rows(db, &closure_ref_id, state, &ingested_at)?;

    Ok(closure_ref_id)
}

pub(super) fn put_baseline<D: EvalDb + ?Sized>(
    db: &D,
    parent: &ParentIdentity,
    baseline: &CompleteBaseline,
    closure_ref_id: Option<&str>,
    evaluation_id: Option<&str>,
    record_ref: Option<&str>,
    recorded_at: String,
) -> Result<String, EvalStoreError> {
    let source_kind = if parent.generation() == 0 {
        "generation0_closure"
    } else {
        "selected_child_eval"
    };
    let baseline_id = baseline_id(parent, baseline, source_kind);
    let ingested_at = chrono::Utc::now().to_rfc3339();
    let mut params = BTreeMap::new();
    params.insert("baseline_id".to_string(), baseline_id.clone().into());
    params.insert(
        "campaign_id".to_string(),
        baseline.campaign_id().to_string().into(),
    );
    params.insert(
        "parent_id".to_string(),
        parent.parent_id().to_string().into(),
    );
    params.insert(
        "parent_node_id".to_string(),
        baseline.parent_node_id().to_string().into(),
    );
    params.insert(
        "parent_branch_id".to_string(),
        baseline.parent_branch_id().to_string().into(),
    );
    params.insert("source_kind".to_string(), source_kind.into());
    params.insert(
        "closure_ref_id".to_string(),
        option_string_param(closure_ref_id.map(str::to_string)),
    );
    params.insert(
        "evaluation_id".to_string(),
        option_string_param(evaluation_id.map(str::to_string)),
    );
    params.insert(
        "record_ref".to_string(),
        option_string_param(record_ref.map(str::to_string)),
    );
    params.insert(
        "eval_set_id".to_string(),
        baseline.eval_set_id().to_string().into(),
    );
    params.insert("status".to_string(), "complete".into());
    params.insert("recorded_at".to_string(), recorded_at.clone().into());
    params.insert("ingested_at".to_string(), ingested_at.clone().into());

    put_eval_params(db, &BaselineSchema::SCHEMA, params, "put.eval_baseline")?;
    put_baseline_instance_rows(db, &baseline_id, baseline, &recorded_at, &ingested_at)?;

    Ok(baseline_id)
}

fn put_closure_instance_rows<D: EvalDb + ?Sized>(
    db: &D,
    closure_ref_id: &str,
    state: &ClosureState,
    ingested_at: &str,
) -> Result<(), EvalStoreError> {
    for row in &state.instances {
        let mut params = BTreeMap::new();
        params.insert(
            "closure_ref_id".to_string(),
            closure_ref_id.to_string().into(),
        );
        params.insert("instance_id".to_string(), row.instance_id.clone().into());
        params.insert(
            "campaign_id".to_string(),
            state.campaign_id.to_string().into(),
        );
        params.insert(
            "dataset_label".to_string(),
            row.dataset_label.clone().into(),
        );
        params.insert("repo_family".to_string(), row.repo_family.clone().into());
        params.insert(
            "registry_status".to_string(),
            enum_string(
                &row.registry_status,
                "eval_closure_instance.registry_status",
            )?
            .into(),
        );
        params.insert(
            "eval_status".to_string(),
            enum_string(&row.eval_status, "eval_closure_instance.eval_status")?.into(),
        );
        params.insert(
            "protocol_status".to_string(),
            enum_string(
                &row.protocol_status,
                "eval_closure_instance.protocol_status",
            )?
            .into(),
        );
        params.insert(
            "eval_failure".to_string(),
            option_string_param(row.eval_failure.clone()),
        );
        params.insert(
            "protocol_failure".to_string(),
            option_string_param(row.protocol_failure.clone()),
        );
        params.insert(
            "last_event_at".to_string(),
            option_string_param(row.last_event_at.clone()),
        );
        params.insert("recorded_at".to_string(), state.updated_at.clone().into());
        params.insert("ingested_at".to_string(), ingested_at.to_string().into());
        put_eval_params(
            db,
            &ClosureInstanceSchema::SCHEMA,
            params,
            "put.eval_closure_instance",
        )?;

        put_closure_artifact_rows(db, closure_ref_id, state, row, ingested_at)?;
        put_closure_protocol_rows(db, closure_ref_id, state, row, ingested_at)?;
    }
    Ok(())
}

fn put_closure_artifact_rows<D: EvalDb + ?Sized>(
    db: &D,
    closure_ref_id: &str,
    state: &ClosureState,
    row: &crate::closure::ClosureInstanceRow,
    ingested_at: &str,
) -> Result<(), EvalStoreError> {
    let artifacts = &row.artifacts;
    let optional = [
        ("registration_path", artifacts.registration_path.as_deref()),
        ("run_manifest", artifacts.run_manifest.as_deref()),
        ("run_root", artifacts.run_root.as_deref()),
        ("record_path", artifacts.record_path.as_deref()),
        ("execution_log", artifacts.execution_log.as_deref()),
        ("indexing_status", artifacts.indexing_status.as_deref()),
        ("parse_failure", artifacts.parse_failure.as_deref()),
        ("snapshot_status", artifacts.snapshot_status.as_deref()),
        ("msb_submission", artifacts.msb_submission.as_deref()),
        (
            "protocol_artifacts_dir",
            artifacts.protocol_artifacts_dir.as_deref(),
        ),
        ("protocol_anchor", artifacts.protocol_anchor.as_deref()),
    ];
    for (kind, path) in optional {
        put_optional_closure_artifact_row(
            db,
            closure_ref_id,
            state,
            &row.instance_id,
            kind,
            0,
            path,
            ingested_at,
        )?;
    }
    for (index, path) in artifacts.batch_failure_sources.iter().enumerate() {
        put_closure_artifact_row(
            db,
            closure_ref_id,
            state,
            &row.instance_id,
            "batch_failure_source",
            usize_to_i64(index, "eval_closure_artifact_ref.artifact_index")?,
            path,
            ingested_at,
        )?;
    }
    Ok(())
}

fn put_optional_closure_artifact_row<D: EvalDb + ?Sized>(
    db: &D,
    closure_ref_id: &str,
    state: &ClosureState,
    instance_id: &str,
    artifact_kind: &str,
    artifact_index: i64,
    path: Option<&Path>,
    ingested_at: &str,
) -> Result<(), EvalStoreError> {
    if let Some(path) = path {
        put_closure_artifact_row(
            db,
            closure_ref_id,
            state,
            instance_id,
            artifact_kind,
            artifact_index,
            path,
            ingested_at,
        )?;
    }
    Ok(())
}

fn put_closure_artifact_row<D: EvalDb + ?Sized>(
    db: &D,
    closure_ref_id: &str,
    state: &ClosureState,
    instance_id: &str,
    artifact_kind: &str,
    artifact_index: i64,
    path: &Path,
    ingested_at: &str,
) -> Result<(), EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert(
        "closure_ref_id".to_string(),
        closure_ref_id.to_string().into(),
    );
    params.insert("instance_id".to_string(), instance_id.to_string().into());
    params.insert(
        "artifact_kind".to_string(),
        artifact_kind.to_string().into(),
    );
    params.insert("artifact_index".to_string(), artifact_index.into());
    params.insert(
        "campaign_id".to_string(),
        state.campaign_id.to_string().into(),
    );
    params.insert("path".to_string(), path.display().to_string().into());
    params.insert("recorded_at".to_string(), state.updated_at.clone().into());
    params.insert("ingested_at".to_string(), ingested_at.to_string().into());
    put_eval_params(
        db,
        &ClosureArtifactRefSchema::SCHEMA,
        params,
        "put.eval_closure_artifact_ref",
    )
}

fn put_closure_protocol_rows<D: EvalDb + ?Sized>(
    db: &D,
    closure_ref_id: &str,
    state: &ClosureState,
    row: &crate::closure::ClosureInstanceRow,
    ingested_at: &str,
) -> Result<(), EvalStoreError> {
    for (procedure, status) in &row.protocol_procedures {
        let mut params = BTreeMap::new();
        params.insert(
            "closure_ref_id".to_string(),
            closure_ref_id.to_string().into(),
        );
        params.insert("instance_id".to_string(), row.instance_id.clone().into());
        params.insert("procedure".to_string(), procedure.clone().into());
        params.insert(
            "campaign_id".to_string(),
            state.campaign_id.to_string().into(),
        );
        params.insert(
            "status".to_string(),
            enum_string(status, "eval_closure_protocol_procedure.status")?.into(),
        );
        params.insert("recorded_at".to_string(), state.updated_at.clone().into());
        params.insert("ingested_at".to_string(), ingested_at.to_string().into());
        put_eval_params(
            db,
            &ClosureProtocolProcedureSchema::SCHEMA,
            params,
            "put.eval_closure_protocol_procedure",
        )?;
    }

    if let Some(counts) = row.protocol_counts.as_ref() {
        let mut params = BTreeMap::new();
        params.insert(
            "closure_ref_id".to_string(),
            closure_ref_id.to_string().into(),
        );
        params.insert("instance_id".to_string(), row.instance_id.clone().into());
        params.insert(
            "campaign_id".to_string(),
            state.campaign_id.to_string().into(),
        );
        params.insert(
            "total_calls".to_string(),
            usize_to_i64(
                counts.total_calls,
                "eval_closure_protocol_counts.total_calls",
            )?
            .into(),
        );
        params.insert(
            "reviewed_calls".to_string(),
            usize_to_i64(
                counts.reviewed_calls,
                "eval_closure_protocol_counts.reviewed_calls",
            )?
            .into(),
        );
        params.insert(
            "total_segments".to_string(),
            usize_to_i64(
                counts.total_segments,
                "eval_closure_protocol_counts.total_segments",
            )?
            .into(),
        );
        params.insert(
            "usable_segments".to_string(),
            usize_to_i64(
                counts.usable_segments,
                "eval_closure_protocol_counts.usable_segments",
            )?
            .into(),
        );
        params.insert(
            "mismatched_segments".to_string(),
            usize_to_i64(
                counts.mismatched_segments,
                "eval_closure_protocol_counts.mismatched_segments",
            )?
            .into(),
        );
        params.insert(
            "missing_segments".to_string(),
            usize_to_i64(
                counts.missing_segments,
                "eval_closure_protocol_counts.missing_segments",
            )?
            .into(),
        );
        params.insert("recorded_at".to_string(), state.updated_at.clone().into());
        params.insert("ingested_at".to_string(), ingested_at.to_string().into());
        put_eval_params(
            db,
            &ClosureProtocolCountsSchema::SCHEMA,
            params,
            "put.eval_closure_protocol_counts",
        )?;
    }
    Ok(())
}

fn put_baseline_instance_rows<D: EvalDb + ?Sized>(
    db: &D,
    baseline_id: &str,
    baseline: &CompleteBaseline,
    recorded_at: &str,
    ingested_at: &str,
) -> Result<(), EvalStoreError> {
    for instance in baseline.instances() {
        let mut params = BTreeMap::new();
        params.insert("baseline_id".to_string(), baseline_id.to_string().into());
        params.insert(
            "instance_id".to_string(),
            instance.instance_id.clone().into(),
        );
        params.insert(
            "campaign_id".to_string(),
            baseline.campaign_id().to_string().into(),
        );
        params.insert(
            "parent_node_id".to_string(),
            baseline.parent_node_id().to_string().into(),
        );
        params.insert(
            "parent_branch_id".to_string(),
            baseline.parent_branch_id().to_string().into(),
        );
        params.insert(
            "eval_set_id".to_string(),
            baseline.eval_set_id().to_string().into(),
        );
        params.insert(
            "registration_path".to_string(),
            option_string_param(
                instance
                    .registration_path
                    .as_ref()
                    .map(|path| path.display().to_string()),
            ),
        );
        params.insert(
            "record_path".to_string(),
            instance.record_path.display().to_string().into(),
        );
        params.insert("recorded_at".to_string(), recorded_at.to_string().into());
        params.insert("ingested_at".to_string(), ingested_at.to_string().into());
        put_eval_params(
            db,
            &BaselineInstanceSchema::SCHEMA,
            params,
            "put.eval_baseline_instance",
        )?;
        put_baseline_instance_metrics_row(
            db,
            baseline_id,
            baseline,
            instance,
            recorded_at,
            ingested_at,
        )?;
    }
    Ok(())
}

fn put_baseline_instance_metrics_row<D: EvalDb + ?Sized>(
    db: &D,
    baseline_id: &str,
    baseline: &CompleteBaseline,
    instance: &crate::intervention::BaselineInstance,
    recorded_at: &str,
    ingested_at: &str,
) -> Result<(), EvalStoreError> {
    let metrics = &instance.metrics;
    let mut params = BTreeMap::new();
    params.insert("baseline_id".to_string(), baseline_id.to_string().into());
    params.insert(
        "instance_id".to_string(),
        instance.instance_id.clone().into(),
    );
    params.insert(
        "campaign_id".to_string(),
        baseline.campaign_id().to_string().into(),
    );
    params.insert(
        "tool_calls_total".to_string(),
        usize_to_i64(
            metrics.tool_calls_total,
            "eval_baseline_instance_metrics.tool_calls_total",
        )?
        .into(),
    );
    params.insert(
        "tool_calls_failed".to_string(),
        usize_to_i64(
            metrics.tool_calls_failed,
            "eval_baseline_instance_metrics.tool_calls_failed",
        )?
        .into(),
    );
    params.insert(
        "patch_attempted".to_string(),
        DataValue::Bool(metrics.patch_attempted),
    );
    params.insert(
        "apply_state".to_string(),
        enum_string(
            &metrics.patch_apply_state,
            "eval_baseline_instance_metrics.apply_state",
        )?
        .into(),
    );
    params.insert(
        "submission_state".to_string(),
        enum_string(
            &metrics.submission_artifact_state,
            "eval_baseline_instance_metrics.submission_state",
        )?
        .into(),
    );
    params.insert(
        "projection_state".to_string(),
        enum_string(
            &metrics.patch_projection_check_state,
            "eval_baseline_instance_metrics.projection_state",
        )?
        .into(),
    );
    params.insert(
        "patch_failures".to_string(),
        usize_to_i64(
            metrics.partial_patch_failures,
            "eval_baseline_instance_metrics.patch_failures",
        )?
        .into(),
    );
    params.insert(
        "same_file_retries".to_string(),
        usize_to_i64(
            metrics.same_file_patch_retry_count,
            "eval_baseline_instance_metrics.same_file_retries",
        )?
        .into(),
    );
    params.insert(
        "same_file_streak".to_string(),
        usize_to_i64(
            metrics.same_file_patch_max_streak,
            "eval_baseline_instance_metrics.same_file_streak",
        )?
        .into(),
    );
    params.insert("aborted".to_string(), DataValue::Bool(metrics.aborted));
    params.insert(
        "repair_aborted".to_string(),
        DataValue::Bool(metrics.aborted_repair_loop),
    );
    params.insert(
        "valid_patch".to_string(),
        DataValue::Bool(metrics.nonempty_valid_patch),
    );
    params.insert(
        "convergence".to_string(),
        DataValue::Bool(metrics.convergence),
    );
    params.insert(
        "oracle_eligible".to_string(),
        DataValue::Bool(metrics.oracle_eligible),
    );
    params.insert("recorded_at".to_string(), recorded_at.to_string().into());
    params.insert("ingested_at".to_string(), ingested_at.to_string().into());
    put_eval_params(
        db,
        &BaselineInstanceMetricsSchema::SCHEMA,
        params,
        "put.eval_baseline_instance_metrics",
    )
}

fn enum_string<T: Serialize>(value: &T, field: &'static str) -> Result<String, EvalStoreError> {
    match serde_json::to_value(value).map_err(|source| EvalStoreError::Validation {
        field,
        detail: source.to_string(),
    })? {
        serde_json::Value::String(value) => Ok(value),
        other => Err(EvalStoreError::Validation {
            field,
            detail: format!("expected string enum value, got {other}"),
        }),
    }
}

fn usize_to_i64(value: usize, field: &'static str) -> Result<i64, EvalStoreError> {
    i64::try_from(value).map_err(|_| EvalStoreError::Validation {
        field,
        detail: format!("value {value} does not fit in Int"),
    })
}

fn u64_to_i64(value: u64, field: &'static str) -> Result<i64, EvalStoreError> {
    i64::try_from(value).map_err(|_| EvalStoreError::Validation {
        field,
        detail: format!("value {value} does not fit in Int"),
    })
}

fn option_u32_param(value: Option<u32>) -> DataValue {
    value
        .map(|value| DataValue::from(i64::from(value)))
        .unwrap_or(DataValue::Null)
}

fn option_u64_param(value: Option<u64>, field: &'static str) -> Result<DataValue, EvalStoreError> {
    value
        .map(|value| u64_to_i64(value, field).map(DataValue::from))
        .unwrap_or(Ok(DataValue::Null))
}

fn profile_ref_id(
    campaign_id: &ploke_records::ids::CampaignId,
    admitted: &AdmittedRunProfile,
) -> String {
    hash_parts(&[
        "p1.eval.profile_commitment.v1",
        campaign_id.as_str(),
        &admitted.commitment.profile_path.display().to_string(),
        &admitted.commitment.sha256,
    ])
}

fn closure_ref_id_from_parts(
    campaign_id: &ploke_records::ids::CampaignId,
    closure_path: &Path,
    content_sha256: &str,
) -> String {
    hash_parts(&[
        "p1.eval.closure_ref.v1",
        campaign_id.as_str(),
        &closure_path.display().to_string(),
        content_sha256,
    ])
}

fn baseline_id(parent: &ParentIdentity, baseline: &CompleteBaseline, source_kind: &str) -> String {
    hash_parts(&[
        "p1.eval.baseline.v1",
        baseline.campaign_id().as_str(),
        parent.parent_id(),
        baseline.parent_node_id(),
        baseline.parent_branch_id(),
        baseline.eval_set_id(),
        source_kind,
    ])
}

fn file_sha256(path: &Path, field: &'static str) -> Result<String, EvalStoreError> {
    let bytes = fs::read(path).map_err(|source| EvalStoreError::Io {
        phase: field,
        path: path.to_path_buf(),
        source,
    })?;
    Ok(sha256_hex(&bytes))
}

fn option_string_param(value: Option<String>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
}

fn string_list_param(values: &[String]) -> DataValue {
    DataValue::List(values.iter().cloned().map(DataValue::from).collect())
}

fn dataset_sources_param(values: &[RegistryDatasetSource]) -> DataValue {
    DataValue::List(
        values
            .iter()
            .map(|source| {
                DataValue::List(vec![
                    option_string_param(source.key.clone()),
                    DataValue::from(source.path.display().to_string()),
                    DataValue::from(source.label.clone()),
                    option_string_param(source.url.clone()),
                ])
            })
            .collect(),
    )
}

fn framework_tools_param(value: &FrameworkConfig) -> DataValue {
    DataValue::List(
        value
            .tools
            .iter()
            .map(|(name, tool)| {
                DataValue::List(vec![
                    DataValue::from(name.clone()),
                    option_string_param(tool.version.clone()),
                ])
            })
            .collect(),
    )
}

fn option_path_param(value: Option<&Path>) -> DataValue {
    value
        .map(|path| DataValue::from(path.display().to_string()))
        .unwrap_or(DataValue::Null)
}

fn option_enum_string_param<T: Serialize>(
    value: Option<&T>,
    field: &'static str,
) -> Result<DataValue, EvalStoreError> {
    value
        .map(|value| enum_string(value, field).map(DataValue::from))
        .unwrap_or(Ok(DataValue::Null))
}

fn option_usize_param(
    value: Option<usize>,
    field: &'static str,
) -> Result<DataValue, EvalStoreError> {
    value
        .map(|value| usize_to_i64(value, field).map(DataValue::from))
        .unwrap_or(Ok(DataValue::Null))
}

fn hash_parts(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    sha256_hex(&hasher.finalize())
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
