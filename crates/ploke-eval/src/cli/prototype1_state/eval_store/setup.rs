use std::{collections::BTreeMap, fs, path::Path};

use cozo::DataValue;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::{
    CampaignManifest,
    cli::prototype1_state::{
        identity::ParentIdentity,
        profile::{AdmittedRunProfile, EvalStorageBackend},
    },
    closure::ClosureState,
    intervention::CompleteBaseline,
};

use super::{cozo_schema::eval_relation_exists, cozo_store::EvalDb, error::EvalStoreError};

pub(crate) const CAMPAIGN_REL: &str = "eval_campaign";
pub(crate) const PROFILE_COMMITMENT_REL: &str = "eval_profile_commitment";
pub(crate) const CLOSURE_REF_REL: &str = "eval_closure_ref";
pub(crate) const BASELINE_REL: &str = "eval_baseline";

pub(super) fn ensure_setup_schema<D: EvalDb + ?Sized>(db: &D) -> Result<(), EvalStoreError> {
    if !eval_relation_exists(db, CAMPAIGN_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_campaign {
    campaign_id: String =>
    schema_version: String,
    manifest_ref: String,
    prototype_root: String,
    manifest_sha256: String,
    profile_ref_id: String?,
    storage_backend: String?,
    ingested_at: String
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_campaign",
            source,
        })?;
    }

    if !eval_relation_exists(db, PROFILE_COMMITMENT_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_profile_commitment {
    profile_ref_id: String =>
    campaign_id: String,
    schema_version: String,
    profile_name: String,
    source_ref: String,
    profile_path: String,
    content_sha256: String,
    source_path: String?,
    admitted_at: String,
    storage_ref: String,
    ingested_at: String
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_profile_commitment",
            source,
        })?;
    }

    if !eval_relation_exists(db, CLOSURE_REF_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_closure_ref {
    closure_ref_id: String =>
    campaign_id: String,
    run_id: String?,
    store_scope: String,
    source_ref: String,
    content_sha256: String,
    summary_json: String,
    recorded_at: String,
    ingested_at: String
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_closure_ref",
            source,
        })?;
    }

    if !eval_relation_exists(db, BASELINE_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_baseline {
    baseline_id: String =>
    campaign_id: String,
    parent_id: String,
    parent_node_id: String,
    parent_branch_id: String,
    source_kind: String,
    closure_ref_id: String?,
    evaluation_id: String?,
    record_ref: String?,
    eval_set_id: String,
    status: String,
    instance_count: Int,
    summary_json: String,
    recorded_at: String,
    ingested_at: String
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_baseline",
            source,
        })?;
    }

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

    db.eval_query_mut_params(
        r#"
?[
    profile_ref_id,
    campaign_id,
    schema_version,
    profile_name,
    source_ref,
    profile_path,
    content_sha256,
    source_path,
    admitted_at,
    storage_ref,
    ingested_at
] :=
    profile_ref_id = $profile_ref_id,
    campaign_id = $campaign_id,
    schema_version = $schema_version,
    profile_name = $profile_name,
    source_ref = $source_ref,
    profile_path = $profile_path,
    content_sha256 = $content_sha256,
    source_path = $source_path,
    admitted_at = $admitted_at,
    storage_ref = $storage_ref,
    ingested_at = $ingested_at
:put eval_profile_commitment {
    profile_ref_id =>
    campaign_id,
    schema_version,
    profile_name,
    source_ref,
    profile_path,
    content_sha256,
    source_path,
    admitted_at,
    storage_ref,
    ingested_at
}
"#,
        params,
    )
    .map_err(|source| EvalStoreError::Db {
        phase: "put.eval_profile_commitment",
        source,
    })?;

    Ok(profile_ref_id)
}

pub(super) fn put_campaign_manifest<D: EvalDb + ?Sized>(
    db: &D,
    manifest_path: &Path,
    manifest: &CampaignManifest,
    storage_backend: EvalStorageBackend,
    profile_ref_id: Option<&str>,
) -> Result<(), EvalStoreError> {
    let manifest_sha256 = file_sha256(manifest_path, "eval_campaign.manifest_ref")?;
    let prototype_root = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1");
    let mut params = BTreeMap::new();
    params.insert(
        "campaign_id".to_string(),
        manifest.campaign_id.to_string().into(),
    );
    params.insert(
        "schema_version".to_string(),
        manifest.schema_version.clone().into(),
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
        storage_backend_label(storage_backend).to_string().into(),
    );
    params.insert(
        "ingested_at".to_string(),
        chrono::Utc::now().to_rfc3339().into(),
    );

    db.eval_query_mut_params(
        r#"
?[
    campaign_id,
    schema_version,
    manifest_ref,
    prototype_root,
    manifest_sha256,
    profile_ref_id,
    storage_backend,
    ingested_at
] :=
    campaign_id = $campaign_id,
    schema_version = $schema_version,
    manifest_ref = $manifest_ref,
    prototype_root = $prototype_root,
    manifest_sha256 = $manifest_sha256,
    profile_ref_id = $profile_ref_id,
    storage_backend = $storage_backend,
    ingested_at = $ingested_at
:put eval_campaign {
    campaign_id =>
    schema_version,
    manifest_ref,
    prototype_root,
    manifest_sha256,
    profile_ref_id,
    storage_backend,
    ingested_at
}
"#,
        params,
    )
    .map_err(|source| EvalStoreError::Db {
        phase: "put.eval_campaign",
        source,
    })?;

    Ok(())
}

pub(super) fn put_closure_ref<D: EvalDb + ?Sized>(
    db: &D,
    closure_path: &Path,
    state: &ClosureState,
) -> Result<String, EvalStoreError> {
    let content_sha256 = file_sha256(closure_path, "eval_closure_ref.source_ref")?;
    let closure_ref_id =
        closure_ref_id_from_parts(&state.campaign_id, closure_path, &content_sha256);
    let summary_json = closure_summary_json(state)?;
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
    params.insert("summary_json".to_string(), summary_json.into());
    params.insert("recorded_at".to_string(), state.updated_at.clone().into());
    params.insert(
        "ingested_at".to_string(),
        chrono::Utc::now().to_rfc3339().into(),
    );

    db.eval_query_mut_params(
        r#"
?[
    closure_ref_id,
    campaign_id,
    run_id,
    store_scope,
    source_ref,
    content_sha256,
    summary_json,
    recorded_at,
    ingested_at
] :=
    closure_ref_id = $closure_ref_id,
    campaign_id = $campaign_id,
    run_id = $run_id,
    store_scope = $store_scope,
    source_ref = $source_ref,
    content_sha256 = $content_sha256,
    summary_json = $summary_json,
    recorded_at = $recorded_at,
    ingested_at = $ingested_at
:put eval_closure_ref {
    closure_ref_id =>
    campaign_id,
    run_id,
    store_scope,
    source_ref,
    content_sha256,
    summary_json,
    recorded_at,
    ingested_at
}
"#,
        params,
    )
    .map_err(|source| EvalStoreError::Db {
        phase: "put.eval_closure_ref",
        source,
    })?;

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
    let summary_json = baseline_summary_json(baseline)?;
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
    params.insert(
        "instance_count".to_string(),
        (baseline.instances().len() as i64).into(),
    );
    params.insert("summary_json".to_string(), summary_json.into());
    params.insert("recorded_at".to_string(), recorded_at.into());
    params.insert(
        "ingested_at".to_string(),
        chrono::Utc::now().to_rfc3339().into(),
    );

    db.eval_query_mut_params(
        r#"
?[
    baseline_id,
    campaign_id,
    parent_id,
    parent_node_id,
    parent_branch_id,
    source_kind,
    closure_ref_id,
    evaluation_id,
    record_ref,
    eval_set_id,
    status,
    instance_count,
    summary_json,
    recorded_at,
    ingested_at
] :=
    baseline_id = $baseline_id,
    campaign_id = $campaign_id,
    parent_id = $parent_id,
    parent_node_id = $parent_node_id,
    parent_branch_id = $parent_branch_id,
    source_kind = $source_kind,
    closure_ref_id = $closure_ref_id,
    evaluation_id = $evaluation_id,
    record_ref = $record_ref,
    eval_set_id = $eval_set_id,
    status = $status,
    instance_count = $instance_count,
    summary_json = $summary_json,
    recorded_at = $recorded_at,
    ingested_at = $ingested_at
:put eval_baseline {
    baseline_id =>
    campaign_id,
    parent_id,
    parent_node_id,
    parent_branch_id,
    source_kind,
    closure_ref_id,
    evaluation_id,
    record_ref,
    eval_set_id,
    status,
    instance_count,
    summary_json,
    recorded_at,
    ingested_at
}
"#,
        params,
    )
    .map_err(|source| EvalStoreError::Db {
        phase: "put.eval_baseline",
        source,
    })?;

    Ok(baseline_id)
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

fn storage_backend_label(backend: EvalStorageBackend) -> &'static str {
    match backend {
        EvalStorageBackend::Fs => "fs",
        EvalStorageBackend::DbMirror => "db-mirror",
        EvalStorageBackend::Database => "database",
        EvalStorageBackend::DualStrict => "dual-strict",
    }
}

fn file_sha256(path: &Path, field: &'static str) -> Result<String, EvalStoreError> {
    let bytes = fs::read(path).map_err(|source| EvalStoreError::Io {
        phase: field,
        path: path.to_path_buf(),
        source,
    })?;
    Ok(sha256_hex(&bytes))
}

fn closure_summary_json(state: &ClosureState) -> Result<String, EvalStoreError> {
    serde_json::to_string(&json!({
        "schema_version": state.schema_version,
        "updated_at": state.updated_at,
        "registry_status": state.registry.status,
        "eval_status": state.eval.status,
        "protocol_status": state.protocol.status,
        "instances": state.instances.len(),
        "expected_eval_total": state.eval.expected_total,
        "expected_protocol_total": state.protocol.expected_total,
    }))
    .map_err(|source| EvalStoreError::Validation {
        field: "eval_closure_ref.summary_json",
        detail: source.to_string(),
    })
}

fn baseline_summary_json(baseline: &CompleteBaseline) -> Result<String, EvalStoreError> {
    let instances: Vec<_> = baseline
        .instances()
        .iter()
        .map(|instance| {
            json!({
                "instance_id": instance.instance_id,
                "record_path": instance.record_path,
                "registration_path": instance.registration_path,
            })
        })
        .collect();
    serde_json::to_string(&json!({
        "eval_set_id": baseline.eval_set_id(),
        "instances": instances,
    }))
    .map_err(|source| EvalStoreError::Validation {
        field: "eval_baseline.summary_json",
        detail: source.to_string(),
    })
}

fn option_string_param(value: Option<String>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
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
