use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::cli::prototype1_state::backend::{
    WORKSPACE_EXCEPT_AUTHORITY_FILENAMES, WORKSPACE_EXCEPT_AUTHORITY_PREFIXES,
    path_matches_surface_policy, prototype_surface_for_policy,
};

use super::{graph, surface};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SurfacePolicy {
    WorkspaceExceptCore(surface::ProtectedCore),
    #[allow(dead_code)] // feature-2 seam
    GraphNeighborhood(surface::Grant),
}

impl SurfacePolicy {
    pub(crate) fn workspace_except_core() -> Self {
        Self::WorkspaceExceptCore(workspace_except_protected_core())
    }

    pub(crate) fn from_legacy(policy: LegacyBroadEditPolicy) -> Self {
        match policy {
            LegacyBroadEditPolicy::WorkspaceExceptPlokeEval => Self::workspace_except_core(),
        }
    }

    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::WorkspaceExceptCore(_) => "workspace except ploke-eval",
            Self::GraphNeighborhood(_) => "graph neighborhood",
        }
    }

    pub(crate) fn prototype_surface(&self) -> crate::cli::Prototype1EditSurface {
        prototype_surface_for_policy(self.clone())
    }

    pub(crate) fn write_scope(&self) -> ploke_tui::utils::path_scoping::WriteScope {
        match self {
            Self::WorkspaceExceptCore(_) => ploke_tui::utils::path_scoping::WriteScope::new()
                .deny_prefixes(
                    WORKSPACE_EXCEPT_AUTHORITY_PREFIXES
                        .iter()
                        .map(PathBuf::from),
                )
                .deny_filenames(
                    WORKSPACE_EXCEPT_AUTHORITY_FILENAMES
                        .iter()
                        .map(|name| (*name).to_string()),
                ),
            Self::GraphNeighborhood(_) => ploke_tui::utils::path_scoping::WriteScope::new(),
        }
    }

    pub(crate) fn classify_paths(
        &self,
        workspace_path: &Path,
        paths: &[PathBuf],
    ) -> Option<super::tui_adapter::Reject> {
        use super::tui_adapter::Reject;

        let surface = self.prototype_surface();
        let mut protected = Vec::new();
        let mut outside = Vec::new();
        for path in paths {
            let rel = if path.is_absolute() {
                match path.strip_prefix(workspace_path) {
                    Ok(rel) => rel.to_path_buf(),
                    Err(_) => {
                        outside.push(path.clone());
                        continue;
                    }
                }
            } else {
                path.clone()
            };
            if validate_normal_repo_relpath(&rel).is_err() {
                outside.push(path.clone());
            } else if !path_matches_surface_policy(surface, &rel) {
                protected.push(rel);
            }
        }
        if !protected.is_empty() {
            Some(Reject::Protected { paths: protected })
        } else if !outside.is_empty() {
            Some(Reject::Outside { paths: outside })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LegacyBroadEditPolicy {
    WorkspaceExceptPlokeEval,
}

impl From<LegacyBroadEditPolicy> for SurfacePolicy {
    fn from(value: LegacyBroadEditPolicy) -> Self {
        Self::from_legacy(value)
    }
}

impl Serialize for SurfacePolicy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let label = match self {
            Self::WorkspaceExceptCore(_) => "workspace_except_ploke_eval",
            Self::GraphNeighborhood(_) => "graph_neighborhood",
        };
        serializer.serialize_str(label)
    }
}

impl<'de> Deserialize<'de> for SurfacePolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let label = String::deserialize(deserializer)?;
        match label.as_str() {
            "workspace_except_ploke_eval" => Ok(Self::workspace_except_core()),
            other => Err(serde::de::Error::custom(format!(
                "unsupported surface policy {other:?}"
            ))),
        }
    }
}

fn workspace_except_protected_core() -> surface::ProtectedCore {
    let policy_hash = surface::Hash::new("policy:workspace-except-core");
    let mut spans = Vec::new();
    for prefix in WORKSPACE_EXCEPT_AUTHORITY_PREFIXES {
        spans.push(graph::Span::new(
            graph::Target::new(prefix, "forbidden-prefix"),
            prefix,
            0,
            usize::MAX,
            policy_hash.clone(),
        ));
    }
    for name in WORKSPACE_EXCEPT_AUTHORITY_FILENAMES {
        let path = PathBuf::from(name);
        spans.push(graph::Span::new(
            graph::Target::new(&path, "forbidden-filename"),
            path,
            0,
            usize::MAX,
            policy_hash.clone(),
        ));
    }
    surface::ProtectedCore::new(spans)
}

fn validate_normal_repo_relpath(path: &Path) -> Result<(), ()> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(());
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => return Err(()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ploke_records::history::SurfaceEvidenceRecord;

    use super::*;

    #[test]
    fn surface_policy_roundtrips_legacy_serde_label() {
        let policy = SurfacePolicy::workspace_except_core();
        let encoded = serde_json::to_string(&policy).expect("serialize policy");
        assert_eq!(encoded, "\"workspace_except_ploke_eval\"");
        let decoded: SurfacePolicy = serde_json::from_str(&encoded).expect("deserialize policy");
        assert_eq!(decoded, policy);
    }

    #[test]
    fn surface_evidence_record_policy_label_unchanged() {
        let json = serde_json::json!({
            "schema_version": 2,
            "producer_id": "prototype1:tui-edit-surface:deterministic-v1",
            "proposal_id": "proposal-accepted",
            "run_id": "run-accepted",
            "policy": "workspace_except_ploke_eval",
            "target_relpath": "src/lib.rs",
            "base": {
                "artifact_id": "git-tree:source",
                "hash": "sha256:source"
            },
            "after": {
                "artifact_id": "git-commit:derived",
                "hash": "sha256:applied"
            },
            "patch_id": "patch:attempt-1",
            "source_content_hash": "sha256:source",
            "proposed_content_hash": "sha256:proposed",
            "proposal_producer": { "kind": "non_router" },
            "generator_surface": {
                "projection_id": "tui-generator-projection",
                "projection_hash": "sha256:projection",
                "bounds_digest": "sha256:bounds",
                "source_kind": "named",
                "source_id": "tui-edit-surface",
                "source_version": "deterministic-v1"
            },
            "touches": [{
                "target_relpath": "src/lib.rs",
                "target_name": "direct-splice:eof-comment",
                "span_relpath": "src/lib.rs",
                "start": 12,
                "end": 12,
                "base_hash": "sha256:source",
                "replacement": " println!(\"hi\");",
                "replacement_hash": "sha256:replacement"
            }],
            "touches_digest": "sha256:touches",
            "delta_id": "surface-delta:sha256:delta",
            "delta_digest": "sha256:delta",
            "check_status": "checked",
            "apply_status": "applied"
        });
        let parsed: SurfaceEvidenceRecord =
            serde_json::from_value(json.clone()).expect("deserialize surface evidence");
        let roundtrip = serde_json::to_value(&parsed).expect("serialize surface evidence");
        assert_eq!(roundtrip, json);
    }

    #[test]
    fn workspace_except_core_rejects_protected_paths() {
        let policy = SurfacePolicy::workspace_except_core();
        let rejection = policy
            .classify_paths(
                Path::new("/tmp/workspace"),
                &[PathBuf::from("crates/ploke-eval/src/lib.rs")],
            )
            .expect("protected path should be rejected");
        assert!(matches!(
            rejection,
            super::super::tui_adapter::Reject::Protected { .. }
        ));
    }
}
