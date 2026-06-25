use std::{collections::BTreeMap, path::Path};

use cozo::DataValue;
use ploke_records::ids::CampaignId;

use super::{
    cozo_store::{EvalDb, mutate_owner_db},
    error::EvalStoreError,
    evidence::{hash_parts, sha256_bytes},
    schema::{EvalRelationSchema, define_eval_schema, put_eval_params},
};
use crate::cli::prototype1_state::history::ArtifactSurface;

define_eval_schema!(ArtifactSchema {
    "eval_artifact",
    artifact_id: "String" =>
    campaign_id: "String",
    tree_hash: "String?",
    git_branch: "String?",
    git_commit: "String?",
    source: "String",
    store_scope: "String",
    created_by: "String?",
    parent_artifact_id: "String?",
});

define_eval_schema!(ArtifactSurfaceSchema {
    "eval_artifact_surface",
    surface_id: "String" =>
    campaign_id: "String",
    artifact_id: "String",
    immutable_root: "String?",
    mutated_root: "String?",
    ambient_root: "String?",
    surface_hash: "String?",
    source_ref: "String?",
    recorded_at: "String?",
});

define_eval_schema!(ArtifactRefSchema {
    "eval_artifact_ref",
    artifact_ref_id: "String" =>
    campaign_id: "String",
    artifact_id: "String?",
    kind: "String",
    source_ref: "String",
    content_sha256: "String?",
    recorded_at: "String?",
});

pub(crate) const ARTIFACT_REL: &str = ArtifactSchema::RELATION;
pub(crate) const ARTIFACT_SURFACE_REL: &str = ArtifactSurfaceSchema::RELATION;
pub(crate) const ARTIFACT_REF_REL: &str = ArtifactRefSchema::RELATION;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactEvidence {
    pub(crate) campaign_id: CampaignId,
    pub(crate) artifact_id: String,
    pub(crate) tree_hash: Option<String>,
    pub(crate) git_branch: Option<String>,
    pub(crate) git_commit: Option<String>,
    pub(crate) source: String,
    pub(crate) store_scope: String,
    pub(crate) created_by: Option<String>,
    pub(crate) parent_artifact_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactSurfaceEvidence {
    pub(crate) campaign_id: CampaignId,
    pub(crate) artifact_id: String,
    pub(crate) immutable_root: Option<String>,
    pub(crate) mutated_root: Option<String>,
    pub(crate) ambient_root: Option<String>,
    pub(crate) surface_hash: Option<String>,
    pub(crate) source_ref: Option<String>,
    pub(crate) recorded_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactRefEvidence {
    pub(crate) campaign_id: CampaignId,
    pub(crate) artifact_id: Option<String>,
    pub(crate) kind: String,
    pub(crate) source_ref: String,
    pub(crate) content_sha256: Option<String>,
    pub(crate) recorded_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactProvenanceEvidence {
    pub(crate) artifact: ArtifactEvidence,
    pub(crate) surface: Option<ArtifactSurfaceEvidence>,
    pub(crate) refs: Vec<ArtifactRefEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactProvenanceReceipt {
    pub(crate) artifact_id: String,
    pub(crate) surface_id: Option<String>,
    pub(crate) artifact_ref_ids: Vec<String>,
}

struct ArtifactRows {
    artifact: EvalArtifactRow,
    surface: Option<EvalArtifactSurfaceRow>,
    refs: Vec<EvalArtifactRefRow>,
}

struct EvalArtifactRow {
    artifact_id: String,
    campaign_id: String,
    tree_hash: Option<String>,
    git_branch: Option<String>,
    git_commit: Option<String>,
    source: String,
    store_scope: String,
    created_by: Option<String>,
    parent_artifact_id: Option<String>,
}

struct EvalArtifactSurfaceRow {
    surface_id: String,
    campaign_id: String,
    artifact_id: String,
    immutable_root: Option<String>,
    mutated_root: Option<String>,
    ambient_root: Option<String>,
    surface_hash: Option<String>,
    source_ref: Option<String>,
    recorded_at: Option<String>,
}

struct EvalArtifactRefRow {
    artifact_ref_id: String,
    campaign_id: String,
    artifact_id: Option<String>,
    kind: String,
    source_ref: String,
    content_sha256: Option<String>,
    recorded_at: Option<String>,
}

pub(super) fn ensure_artifact_schema<D: EvalDb + ?Sized>(db: &D) -> Result<(), EvalStoreError> {
    ArtifactSchema::SCHEMA.ensure_installed(db, "schema.eval_artifact")?;
    ArtifactSurfaceSchema::SCHEMA.ensure_installed(db, "schema.eval_artifact_surface")?;
    ArtifactRefSchema::SCHEMA.ensure_installed(db, "schema.eval_artifact_ref")?;
    Ok(())
}

pub(crate) fn write_artifact_provenance_to_owner_db(
    db_path: &Path,
    evidence: ArtifactProvenanceEvidence,
) -> Result<ArtifactProvenanceReceipt, EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        ensure_artifact_schema(db)?;
        let rows = artifact_rows(evidence)?;
        put_artifact_row(db, &rows.artifact)?;
        if let Some(surface) = &rows.surface {
            put_artifact_surface_row(db, surface)?;
        }
        for artifact_ref in &rows.refs {
            put_artifact_ref_row(db, artifact_ref)?;
        }
        Ok(ArtifactProvenanceReceipt {
            artifact_id: rows.artifact.artifact_id,
            surface_id: rows.surface.map(|surface| surface.surface_id),
            artifact_ref_ids: rows
                .refs
                .into_iter()
                .map(|artifact_ref| artifact_ref.artifact_ref_id)
                .collect(),
        })
    })
}

pub(crate) fn artifact_surface_hash(surface: &ArtifactSurface) -> Result<String, EvalStoreError> {
    let bytes = serde_json::to_vec(surface).map_err(|source| EvalStoreError::Validation {
        field: "artifact_surface.payload",
        detail: source.to_string(),
    })?;
    Ok(sha256_bytes(&bytes))
}

fn artifact_rows(evidence: ArtifactProvenanceEvidence) -> Result<ArtifactRows, EvalStoreError> {
    require_non_empty("artifact.artifact_id", &evidence.artifact.artifact_id)?;
    require_non_empty("artifact.source", &evidence.artifact.source)?;
    require_non_empty("artifact.store_scope", &evidence.artifact.store_scope)?;
    if let Some(parent_artifact_id) = &evidence.artifact.parent_artifact_id {
        require_non_empty("artifact.parent_artifact_id", parent_artifact_id)?;
    }
    let artifact = EvalArtifactRow {
        artifact_id: evidence.artifact.artifact_id.clone(),
        campaign_id: evidence.artifact.campaign_id.to_string(),
        tree_hash: evidence.artifact.tree_hash,
        git_branch: evidence.artifact.git_branch,
        git_commit: evidence.artifact.git_commit,
        source: evidence.artifact.source,
        store_scope: evidence.artifact.store_scope,
        created_by: evidence.artifact.created_by,
        parent_artifact_id: evidence.artifact.parent_artifact_id,
    };
    let surface = evidence.surface.map(artifact_surface_row).transpose()?;
    let refs = evidence
        .refs
        .into_iter()
        .map(artifact_ref_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ArtifactRows {
        artifact,
        surface,
        refs,
    })
}

fn artifact_surface_row(
    evidence: ArtifactSurfaceEvidence,
) -> Result<EvalArtifactSurfaceRow, EvalStoreError> {
    require_non_empty("artifact_surface.artifact_id", &evidence.artifact_id)?;
    if let Some(surface_hash) = &evidence.surface_hash {
        require_non_empty("artifact_surface.surface_hash", surface_hash)?;
    }
    if let Some(source_ref) = &evidence.source_ref {
        require_non_empty("artifact_surface.source_ref", source_ref)?;
    }
    let surface_id = surface_id(
        &evidence.campaign_id,
        &evidence.artifact_id,
        evidence.surface_hash.as_deref(),
        evidence.source_ref.as_deref(),
    );
    Ok(EvalArtifactSurfaceRow {
        surface_id,
        campaign_id: evidence.campaign_id.to_string(),
        artifact_id: evidence.artifact_id,
        immutable_root: evidence.immutable_root,
        mutated_root: evidence.mutated_root,
        ambient_root: evidence.ambient_root,
        surface_hash: evidence.surface_hash,
        source_ref: evidence.source_ref,
        recorded_at: evidence.recorded_at,
    })
}

fn artifact_ref_row(evidence: ArtifactRefEvidence) -> Result<EvalArtifactRefRow, EvalStoreError> {
    require_non_empty("artifact_ref.kind", &evidence.kind)?;
    require_non_empty("artifact_ref.source_ref", &evidence.source_ref)?;
    if let Some(artifact_id) = &evidence.artifact_id {
        require_non_empty("artifact_ref.artifact_id", artifact_id)?;
    }
    let artifact_ref_id = artifact_ref_id(
        &evidence.campaign_id,
        evidence.artifact_id.as_deref(),
        &evidence.kind,
        &evidence.source_ref,
        evidence.content_sha256.as_deref(),
    );
    Ok(EvalArtifactRefRow {
        artifact_ref_id,
        campaign_id: evidence.campaign_id.to_string(),
        artifact_id: evidence.artifact_id,
        kind: evidence.kind,
        source_ref: evidence.source_ref,
        content_sha256: evidence.content_sha256,
        recorded_at: evidence.recorded_at,
    })
}

fn put_artifact_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalArtifactRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &ArtifactSchema::SCHEMA,
        artifact_params(row),
        "put.eval_artifact",
    )?;
    Ok(())
}

fn put_artifact_surface_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalArtifactSurfaceRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &ArtifactSurfaceSchema::SCHEMA,
        artifact_surface_params(row),
        "put.eval_artifact_surface",
    )?;
    Ok(())
}

fn put_artifact_ref_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalArtifactRefRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &ArtifactRefSchema::SCHEMA,
        artifact_ref_params(row),
        "put.eval_artifact_ref",
    )?;
    Ok(())
}

fn artifact_params(row: &EvalArtifactRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("artifact_id".to_string(), row.artifact_id.clone().into());
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("tree_hash".to_string(), option_string(&row.tree_hash));
    params.insert("git_branch".to_string(), option_string(&row.git_branch));
    params.insert("git_commit".to_string(), option_string(&row.git_commit));
    params.insert("source".to_string(), row.source.clone().into());
    params.insert("store_scope".to_string(), row.store_scope.clone().into());
    params.insert("created_by".to_string(), option_string(&row.created_by));
    params.insert(
        "parent_artifact_id".to_string(),
        option_string(&row.parent_artifact_id),
    );
    params
}

fn artifact_surface_params(row: &EvalArtifactSurfaceRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("surface_id".to_string(), row.surface_id.clone().into());
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("artifact_id".to_string(), row.artifact_id.clone().into());
    params.insert(
        "immutable_root".to_string(),
        option_string(&row.immutable_root),
    );
    params.insert("mutated_root".to_string(), option_string(&row.mutated_root));
    params.insert("ambient_root".to_string(), option_string(&row.ambient_root));
    params.insert("surface_hash".to_string(), option_string(&row.surface_hash));
    params.insert("source_ref".to_string(), option_string(&row.source_ref));
    params.insert("recorded_at".to_string(), option_string(&row.recorded_at));
    params
}

fn artifact_ref_params(row: &EvalArtifactRefRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert(
        "artifact_ref_id".to_string(),
        row.artifact_ref_id.clone().into(),
    );
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("artifact_id".to_string(), option_string(&row.artifact_id));
    params.insert("kind".to_string(), row.kind.clone().into());
    params.insert("source_ref".to_string(), row.source_ref.clone().into());
    params.insert(
        "content_sha256".to_string(),
        option_string(&row.content_sha256),
    );
    params.insert("recorded_at".to_string(), option_string(&row.recorded_at));
    params
}

fn option_string(value: &Option<String>) -> DataValue {
    value
        .clone()
        .map(DataValue::from)
        .unwrap_or(DataValue::Null)
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), EvalStoreError> {
    if value.is_empty() {
        return Err(EvalStoreError::Validation {
            field,
            detail: "must not be empty".to_string(),
        });
    }
    Ok(())
}

fn surface_id(
    campaign_id: &CampaignId,
    artifact_id: &str,
    surface_hash: Option<&str>,
    source_ref: Option<&str>,
) -> String {
    hash_parts(&[
        "p1.eval.artifact_surface.v1",
        campaign_id.as_str(),
        artifact_id,
        surface_hash.unwrap_or(""),
        source_ref.unwrap_or(""),
    ])
}

fn artifact_ref_id(
    campaign_id: &CampaignId,
    artifact_id: Option<&str>,
    kind: &str,
    source_ref: &str,
    content_sha256: Option<&str>,
) -> String {
    hash_parts(&[
        "p1.eval.artifact_ref.v1",
        campaign_id.as_str(),
        artifact_id.unwrap_or(""),
        kind,
        source_ref,
        content_sha256.unwrap_or(""),
    ])
}
