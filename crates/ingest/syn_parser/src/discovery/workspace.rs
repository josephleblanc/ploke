use std::fs;
use std::path::{Path, PathBuf};

use cargo_toml::Manifest;
use itertools::Itertools;
use ploke_core::workspace_glob::expand_simple_workspace_member;
use serde::{Deserialize, Serialize};

use crate::discovery::DiscoveryError;
use crate::discovery::ManifestCtx;
use crate::discovery::ManifestKind;
use crate::discovery::WithDiscoveryManifestCargoToml;
use crate::discovery::WithDiscoveryManifestRead;

/// Partial view of a manifest that may or may not declare workspace metadata. `workspace = None`
/// signals that the inspected manifest isn't a workspace boundary.
#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct WorkspaceManifestMetadata {
    pub workspace: Option<WorkspaceMetadataSection>,
}

/// Captures the `[workspace]` table when parsing ancestor manifests.
#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct WorkspaceMetadataSection {
    #[serde(skip)]
    pub path: PathBuf,
    pub exclude: Option<Vec<PathBuf>>,
    pub resolver: Option<String>,
    pub members: Vec<PathBuf>,
    pub package: Option<WorkspacePackageMetadata>,
}

impl WorkspaceMetadataSection {
    pub fn package_version(&self) -> Option<&str> {
        self.package
            .as_ref()
            .and_then(WorkspacePackageMetadata::version)
    }
}

/// Captures the `[workspace.package]` metadata that may hold the shared version.
#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct WorkspacePackageMetadata {
    version: Option<String>,
}

impl WorkspacePackageMetadata {
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
}

/// Resolve a workspace-inherited version by loading the nearest ancestor workspace manifest.
///
/// # Success
/// Returns the `workspace.package.version` string discovered at or above `crate_root`.
///
/// # Errors
/// * [`DiscoveryError::WorkspacePackageVersionMissing`] when the workspace manifest lacks a version.
/// * Any error bubbled up from [`locate_workspace_manifest`].
///
/// # Examples
/// Create a miniature workspace layout in a temporary directory to prove that inheritance works:
/// ```
/// use syn_parser::discovery::resolve_workspace_version;
/// use tempfile::tempdir;
/// use std::fs;
///
/// let tmp = tempdir().unwrap();
/// let ws_root = tmp.path();
/// fs::create_dir_all(ws_root.join("member/src")).unwrap();
/// fs::write(
///     ws_root.join("Cargo.toml"),
///     r#"[workspace]
/// members = ["member"]
///
/// [workspace.package]
/// version = "7.8.9"
/// "#,
/// ).unwrap();
/// fs::write(
///     ws_root.join("member/Cargo.toml"),
///     r#"[package]
/// name = "member"
/// version.workspace = true
/// edition = "2021"
/// "#,
/// ).unwrap();
///
/// let member_root = ws_root.join("member");
/// let version = resolve_workspace_version(&member_root).unwrap();
/// assert_eq!(version, "7.8.9");
/// ```
pub fn resolve_workspace_version(crate_root: &Path) -> Result<String, DiscoveryError> {
    let (workspace_manifest_path, workspace_metadata) = locate_workspace_manifest(crate_root)?;

    workspace_metadata
        .workspace
        .as_ref()
        .and_then(|section| section.package.as_ref())
        .and_then(|package| package.version.clone())
        .ok_or_else(|| DiscoveryError::WorkspacePackageVersionMissing {
            crate_path: crate_root.to_path_buf(),
            manifest_path: workspace_manifest_path.clone(),
        })
}

/// Search upwards from the crate root until a `[workspace]` manifest is found.
///
/// # Success
/// Returns the manifest path and parsed metadata for the nearest ancestor that declares a
/// `[workspace]` table, including the crate's own manifest.
///
/// # Errors
/// * [`DiscoveryError::ManifestRead`] on IO failures.
/// * [`DiscoveryError::ManifestParse`] on TOML parse failures.
/// * [`DiscoveryError::WorkspaceManifestNotFound`] if no workspace manifests exist up to filesystem root.
///
/// # Examples
/// Demonstrate the missing-workspace error using a throwaway project tree:
/// ```
/// use syn_parser::discovery::{locate_workspace_manifest, DiscoveryError};
/// use tempfile::tempdir;
/// use std::fs;
///
/// let tmp = tempdir().unwrap();
/// let crate_root = tmp.path().join("solo");
/// fs::create_dir_all(crate_root.join("src")).unwrap();
/// fs::write(
///     crate_root.join("Cargo.toml"),
///     r#"[package]
/// name = "solo"
/// version.workspace = true
/// edition = "2021"
/// "#,
/// ).unwrap();
///
/// let err = locate_workspace_manifest(&crate_root).unwrap_err();
/// assert!(matches!(err, DiscoveryError::WorkspaceManifestNotFound { .. }));
/// ```
pub fn locate_workspace_manifest(
    crate_root: &Path,
) -> Result<(PathBuf, WorkspaceManifestMetadata), DiscoveryError> {
    let mut current_dir = Some(crate_root);
    let target_crate_path = crate_root.to_path_buf();

    while let Some(dir) = current_dir {
        let candidate_manifest = dir.join("Cargo.toml");
        if !candidate_manifest.is_file() {
            current_dir = dir.parent();
            continue;
        }

        let metadata =
            try_parse_manifest(dir, ManifestKind::AncestorWorkspace).map_err(|mut e| {
                if let DiscoveryError::ManifestRead {
                    ref mut crate_path, ..
                } = e
                {
                    *crate_path = Some(target_crate_path.clone());
                } else if let DiscoveryError::ManifestParse {
                    ref mut crate_path, ..
                } = e
                {
                    *crate_path = Some(target_crate_path.clone());
                }
                e
            })?;

        if metadata.workspace.is_some() {
            return Ok((candidate_manifest, metadata));
        }

        current_dir = dir.parent();
    }

    Err(DiscoveryError::WorkspaceManifestNotFound {
        crate_path: target_crate_path,
    })
}

/// Parse `target_dir/Cargo.toml` into [`WorkspaceManifestMetadata`] using [`cargo_toml::Manifest`].
///
/// Parsing uses [`Manifest::from_str`] (not [`Manifest::from_path`]) so workspace discovery can
/// read `[workspace]` without requiring Cargo’s manifest completion (e.g. when walking ancestors
/// past crates that use workspace inheritance).
///
/// `kind` classifies the manifest for diagnostics ([`DiscoveryError::ManifestRead`] /
/// [`DiscoveryError::ManifestParse`]). Callers walking from a crate toward the filesystem root
/// should use [`ManifestKind::AncestorWorkspace`]; workspace roots (e.g. [`crate::parse_workspace`])
/// use [`ManifestKind::WorkspaceRoot`]; a crate directory’s own manifest uses [`ManifestKind::Crate`].
pub fn try_parse_manifest(
    target_dir: &Path,
    kind: ManifestKind,
) -> Result<WorkspaceManifestMetadata, DiscoveryError> {
    let candidate_manifest = target_dir.join("Cargo.toml");
    let ctx = ManifestCtx {
        kind,
        manifest_path: candidate_manifest.clone(),
        crate_path: None,
        content: None,
    };
    // Use `from_str` without `complete_from_path` so ancestor walks can inspect manifests that
    // declare `package.version.workspace = true` even when no workspace root exists yet (matches
    // the legacy `toml::from_str` behavior for [`WorkspaceManifestMetadata`]).
    let content = fs::read_to_string(&candidate_manifest)
        .with_discovery_err(ctx.clone())
        .for_read()?;
    let manifest: Manifest = Manifest::from_str(&content)
        .with_discovery_err(ctx.with_content(&content))
        .for_cargo_toml()?;

    let workspace_root = candidate_manifest.parent().ok_or_else(|| {
        DiscoveryError::ParentNotFound {
            workspace_path: candidate_manifest.clone(),
        }
    })?;

    let workspace = manifest.workspace.map(|ws| {
        let mut members: Vec<PathBuf> = ws
            .members
            .iter()
            .flat_map(|m| expand_simple_workspace_member(workspace_root, Path::new(m)))
            .collect();
        members.sort();
        members.dedup();

        let exclude = if ws.exclude.is_empty() {
            None
        } else {
            Some(ws.exclude.iter().map(|e| workspace_root.join(e)).collect())
        };

        WorkspaceMetadataSection {
            path: workspace_root.to_path_buf(),
            exclude,
            resolver: ws.resolver.map(|r| r.to_string()),
            members,
            package: ws.package.as_ref().map(|p| WorkspacePackageMetadata {
                version: p.version.clone(),
            }),
        }
    });

    Ok(WorkspaceManifestMetadata { workspace })
}

#[derive(Clone, Debug, Default)]
pub struct WorkspaceMetaBuilder {
    exclude: Option<Vec<String>>,
    resolver: Option<String>,
    members: Option<Vec<String>>,
    package: Option<WorkspacePackageMetadata>,
    path: PathBuf,
    build_status: BuildStatus,
}

impl WorkspaceMetaBuilder {
    pub fn from_dir_path(fp: &Path) -> Result<Self, DiscoveryError> {
        let cargo_toml_path = fp.join("Cargo.toml");
        Self::from_toml_path(&cargo_toml_path)
    }

    pub fn from_toml_path(fp: &Path) -> Result<Self, DiscoveryError> {
        let ctx = ManifestCtx {
            kind: ManifestKind::WorkspaceRoot,
            manifest_path: fp.to_path_buf(),
            crate_path: None,
            content: None,
        };
        let cargo_content = fs::read_to_string(fp)
            .with_discovery_err(ctx.clone())
            .for_read()?;

        let manifest: Manifest = Manifest::from_str(&cargo_content)
            .with_discovery_err(ctx.with_content(&cargo_content))
            .for_cargo_toml()?;

        let path = fp.parent().ok_or_else(|| DiscoveryError::ParentNotFound {
            workspace_path: fp.to_path_buf(),
        })?;

        let Some(ws) = manifest.workspace else {
            return Ok(WorkspaceMetaBuilder {
                exclude: None,
                resolver: None,
                members: None,
                package: None,
                path: path.to_path_buf(),
                build_status: BuildStatus::Empty,
            });
        };

        let build_status = if !ws.members.is_empty() {
            BuildStatus::Ready
        } else {
            BuildStatus::Empty
        };

        Ok(WorkspaceMetaBuilder {
            exclude: if ws.exclude.is_empty() {
                None
            } else {
                Some(ws.exclude.clone())
            },
            resolver: ws.resolver.map(|r| r.to_string()),
            members: Some(ws.members.clone()),
            package: ws.package.as_ref().map(|p| WorkspacePackageMetadata {
                version: p.version.clone(),
            }),
            path: path.to_path_buf(),
            build_status,
        })
    }

    pub fn build(self) -> Result<WorkspaceMetadataSection, DiscoveryError> {
        let WorkspaceMetaBuilder {
            exclude,
            resolver,
            members,
            path,
            build_status,
            package,
        } = self;

        if build_status != BuildStatus::Ready {
            return Err(DiscoveryError::WorkspaceBuildNotReady {
                workspace_path: path.clone(),
                build_status: build_status.to_string(),
            });
        }
        let exclude_final: Option<Vec<PathBuf>> =
            exclude.map(|ex| ex.into_iter().map(|s| path.join(s)).collect_vec());
        let members_list = members.ok_or_else(|| DiscoveryError::WorkspaceMembersNone {
            workspace_path: path.clone(),
            build_status: build_status.to_string(),
        })?;
        if members_list.is_empty() {
            return Err(DiscoveryError::WorkspaceNoMembers {
                workspace_path: path.clone(),
                build_status: build_status.to_string(),
            });
        }

        let mut members_final: Vec<PathBuf> = members_list
            .into_iter()
            .flat_map(|s| expand_simple_workspace_member(path.as_path(), Path::new(&s)))
            .collect();
        members_final.sort();
        members_final.dedup();

        Ok(WorkspaceMetadataSection {
            path,
            exclude: exclude_final,
            resolver,
            members: members_final,
            package,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Default)]
enum BuildStatus {
    #[default]
    Empty,
    Ready,
}

impl std::fmt::Display for BuildStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildStatus::Empty => write!(f, "Empty"),
            BuildStatus::Ready => write!(f, "Ready"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use ploke_common::workspace_root;
    use ploke_error::Error as PlokeError;
    use tempfile::tempdir;

    #[test]
    fn simple() -> Result<(), PlokeError> {
        let tmp = tempdir().unwrap();

        let workspace_root = tmp.path().join("test-workspace");
        let crate_root = workspace_root.join("solo");

        fs::create_dir_all(crate_root.join("src")).unwrap();
        fs::write(
            crate_root.join("Cargo.toml"),
            r#"[package]
name = "test-crate"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();

        fs::write(
            workspace_root.join("Cargo.toml"),
            r#"[workspace]
members = [ "solo" ]
"#,
        )
        .unwrap();

        let (path, meta): (PathBuf, WorkspaceManifestMetadata) =
            locate_workspace_manifest(&crate_root)?;

        println!(
            "found workspace path: {}\nmetadata: {:#?}",
            path.display(),
            meta
        );
        Ok(())
    }

    #[test]
    fn nested() -> Result<(), PlokeError> {
        let tmp = tempdir().unwrap();

        let workspace_root = tmp.path().join("test-workspace");

        // --- solo crate setup ---
        let crate_root = workspace_root.join("solo");
        fs::create_dir_all(crate_root.join("src")).unwrap();
        fs::write(
            crate_root.join("Cargo.toml"),
            r#"[package]
name = "solo"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();

        // --- nested/common crate setup ---
        let nested_dir = workspace_root.join("nested");

        let common_root = nested_dir.join("test-common");
        fs::create_dir_all(common_root.join("src")).unwrap();
        fs::write(
            common_root.join("Cargo.toml"),
            r#"[package]
name = "test-common"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();

        // --- nested/deeper_nested/inner-crate crate setup ---
        let deeper_nested_dir = nested_dir.join("deeper_nested");

        let inner_root = deeper_nested_dir.join("inner-crate");
        fs::create_dir_all(inner_root.join("src")).unwrap();
        fs::write(
            inner_root.join("Cargo.toml"),
            r#"[package]
name = "inner-crate"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();

        // --- workspace setup ---
        fs::write(
            workspace_root.join("Cargo.toml"),
            r#"[workspace]
members = [
    "solo",
    "nested/common",
    "nested/deeper_nested/inner-crate"
]"#,
        )
        .unwrap();

        let (path, meta): (PathBuf, WorkspaceManifestMetadata) =
            locate_workspace_manifest(&crate_root)?;
        eprintln!(
            "found workspace path: {}\nmetadata: {:#?}",
            path.display(),
            meta
        );

        let (path, meta): (PathBuf, WorkspaceManifestMetadata) =
            locate_workspace_manifest(&common_root)?;
        eprintln!(
            "found workspace path: {}\nmetadata: {:#?}",
            path.display(),
            meta
        );

        let (path, meta): (PathBuf, WorkspaceManifestMetadata) =
            locate_workspace_manifest(&inner_root)?;
        eprintln!(
            "found workspace path: {}\nmetadata: {:#?}",
            path.display(),
            meta
        );

        let workspace_builder = WorkspaceMetaBuilder::from_dir_path(&workspace_root)?;
        eprintln!("workspace_builder: {:#?}", workspace_builder);
        assert!(workspace_builder.members.is_some(), "expect Some members");

        let expected_members = ["solo", "nested/common", "nested/deeper_nested/inner-crate"];
        if let Some(member_list) = workspace_builder.members.as_ref() {
            for m in member_list {
                assert!(expected_members.contains(&m.as_str()));
            }
        }
        assert_eq!(BuildStatus::Ready, workspace_builder.build_status);

        let workspace_meta: WorkspaceMetadataSection = workspace_builder.build()?;
        println!("-- WorkspaceMetadataSection--\n{:#?}", workspace_meta);
        for m in workspace_meta.members {
            let has_expected = expected_members.iter().any(|exp| m.ends_with(exp));
            assert!(has_expected);
        }
        Ok(())
    }

    #[test]
    fn simple_self() -> Result<(), PlokeError> {
        let target_crate_root: PathBuf = workspace_root().join("ingest/syn_parser");
        let (path, meta): (PathBuf, WorkspaceManifestMetadata) =
            locate_workspace_manifest(&target_crate_root)?;
        println!(
            "found workspace path: {}\nmetadata: {:#?}",
            path.display(),
            meta
        );

        Ok(())
    }

    #[test]
    fn committed_workspace_fixture_locates_nested_members() -> Result<(), PlokeError> {
        let fixture_workspace_root = workspace_root().join("tests/fixture_workspace/ws_fixture_01");
        let nested_member_root = fixture_workspace_root.join("nested/member_nested");

        let (manifest_path, metadata) = locate_workspace_manifest(&nested_member_root)?;
        let workspace = metadata
            .workspace
            .expect("committed fixture should parse as a workspace");

        assert_eq!(manifest_path, fixture_workspace_root.join("Cargo.toml"));
        assert_eq!(workspace.path, fixture_workspace_root);
        assert_eq!(
            workspace.members,
            vec![
                workspace.path.join("member_root"),
                workspace.path.join("nested/member_nested"),
            ]
        );
        assert_eq!(workspace.package_version(), Some("0.2.0"));

        Ok(())
    }
}
