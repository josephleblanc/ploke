use std::backtrace::Backtrace;
use std::path::{Path, PathBuf};
use std::panic::Location;
use std::sync::Arc;

use ploke_error::{DiagnosticField, DiagnosticInfo, DiagnosticSite, DiagnosticSpan, SourceSpan};
use serde::Serialize;
use toml;

use thiserror::Error;

/// Errors that can occur during the discovery phase.
#[derive(Error, Debug, Clone)] // Add Clone derive
pub enum DiscoveryError {
    /// An I/O error occurred while accessing a path.
    #[error("I/O error accessing path {path}: {source}")]
    Io {
        path: PathBuf,
        emission_site: DiagnosticSite,
        backtrace: Arc<Backtrace>,
        #[source]
        source: Arc<std::io::Error>, // Wrap in Arc
    },
    /// Failed to parse a `Cargo.toml` file.
    #[error("Failed to parse Cargo.toml at {path}: {source}")]
    TomlParse {
        path: PathBuf,
        span: Option<SourceSpan>,
        emission_site: DiagnosticSite,
        backtrace: Arc<Backtrace>,
        #[source]
        source: Arc<toml::de::Error>, // Wrap in Arc
    },
    /// The `package.name` field was missing from a `Cargo.toml` file.
    #[error("Missing 'package.name' in Cargo.toml at {path}")]
    MissingPackageName { path: PathBuf }, // This variant is already Clone
    /// The `package.version` field was missing from a `Cargo.toml` file.
    #[error("Missing 'package.version' in Cargo.toml at {path}")]
    MissingPackageVersion { path: PathBuf },
    /// The target crate path was not found.
    #[error("Target crate path not found: {path}")]
    CratePathNotFound { path: PathBuf },
    /// An error occurred while walking a directory.
    #[error("Walkdir error in {path}: {source}")]
    Walkdir {
        path: PathBuf,
        emission_site: DiagnosticSite,
        backtrace: Arc<Backtrace>,
        #[source]
        source: Arc<walkdir::Error>, // Wrap in Arc
    },
    /// The source directory was not found for a crate.
    #[error("Source directory not found for crate at: {path}")]
    SrcNotFound { path: PathBuf }, // Critical error: Cannot proceed without source files.
    /// Multiple non-fatal errors occurred during discovery.
    #[error("Multiple non-fatal errors occurred during discovery")]
    NonFatalErrors(Box<Vec<DiscoveryError>>), // Box to avoid large enum variant
    /// Failed to read a workspace manifest needed to resolve package version inheritance.
    #[error(
        "Failed to read workspace Cargo.toml at {manifest_path} while resolving crate at {crate_path:?}: {source}"
    )]
    WorkspaceManifestRead {
        crate_path: Option<PathBuf>,
        manifest_path: PathBuf,
        emission_site: DiagnosticSite,
        backtrace: Arc<Backtrace>,
        #[source]
        source: Arc<std::io::Error>,
    },
    /// Failed to parse a workspace manifest needed to resolve package version inheritance.
    #[error(
        "Failed to parse workspace Cargo.toml at {manifest_path} while resolving crate at {crate_path:?}: {source}"
    )]
    WorkspaceManifestParse {
        crate_path: Option<PathBuf>,
        manifest_path: PathBuf,
        span: Option<SourceSpan>,
        emission_site: DiagnosticSite,
        backtrace: Arc<Backtrace>,
        #[source]
        source: Arc<toml::de::Error>,
    },
    /// Could not find a workspace manifest when attempting to resolve `package.version.workspace = true`.
    #[error("Unable to locate workspace Cargo.toml when resolving crate at {crate_path}")]
    WorkspaceManifestNotFound { crate_path: PathBuf },
    /// Workspace manifest exists, but `workspace.package.version` is missing.
    #[error(
        "workspace.package.version missing in workspace Cargo.toml at {manifest_path} (required by crate at {crate_path})"
    )]
    WorkspacePackageVersionMissing {
        crate_path: PathBuf,
        manifest_path: PathBuf,
    },
    /// `package.version.workspace` was set to `false`, which is unsupported.
    #[error(
        "`package.version.workspace` must be true when inheriting version for crate at {crate_path} (manifest {manifest_path})"
    )]
    WorkspaceVersionFlagDisabled {
        crate_path: PathBuf,
        manifest_path: PathBuf,
    },

    #[error(
        "Cannot build the WorkspaceManifest with empty members, build listed as Incomplete or Empty for workspace located at {workspace_path}"
    )]
    WorkspaceBuildNotReady {
        workspace_path: PathBuf,
        build_status: String,
    },

    #[error(
        "Cannot build workspace with members = None, located at workspace located at {workspace_path} with build_status {build_status}"
    )]
    WorkspaceNoMembers {
        workspace_path: PathBuf,
        build_status: String,
    },

    #[error(
        "Cannot build workspace with no members, located at workspace located at {workspace_path} with build_status {build_status}"
    )]
    WorkspaceMembersNone {
        workspace_path: PathBuf,
        build_status: String,
    },

    #[error(
        "Workspace parsed at {workspace_path} for crate {crate_path} does not contain [{expected}] section when it was expected."
    )]
    WorkspaceMissingSection {
        workspace_path: PathBuf,
        crate_path: PathBuf,
        expected: String,
    },

    #[error(
        "Crate at {crate_path} resolved to workspace {discovered_workspace_path}, but discovery expected workspace {expected_workspace_path}"
    )]
    WorkspacePathMismatch {
        crate_path: PathBuf,
        expected_workspace_path: PathBuf,
        discovered_workspace_path: PathBuf,
    },

    #[error(
        "Discovery found multiple workspaces for one run. Expected workspace: {expected_workspace_path}. Discovered workspaces: {discovered_workspace_paths:?}. Target crates: {crate_paths:?}"
    )]
    MultipleWorkspacesDetected {
        expected_workspace_path: PathBuf,
        discovered_workspace_paths: Vec<PathBuf>,
        crate_paths: Vec<PathBuf>,
    },

    #[error("Parent directory for Cargo.toml not found")]
    ParentNotFound { workspace_path: PathBuf },
}

impl DiagnosticInfo for DiscoveryError {
    fn diagnostic_kind(&self) -> &'static str {
        match self {
            DiscoveryError::Io { .. } => "discovery_io",
            DiscoveryError::TomlParse { .. } => "manifest_toml_parse",
            DiscoveryError::MissingPackageName { .. } => "manifest_missing_package_name",
            DiscoveryError::MissingPackageVersion { .. } => "manifest_missing_package_version",
            DiscoveryError::CratePathNotFound { .. } => "crate_path_not_found",
            DiscoveryError::Walkdir { .. } => "discovery_walkdir",
            DiscoveryError::SrcNotFound { .. } => "src_not_found",
            DiscoveryError::NonFatalErrors(_) => "discovery_non_fatal_errors",
            DiscoveryError::WorkspaceManifestRead { .. } => "workspace_manifest_read",
            DiscoveryError::WorkspaceManifestParse { .. } => "workspace_manifest_parse",
            DiscoveryError::WorkspaceManifestNotFound { .. } => "workspace_manifest_not_found",
            DiscoveryError::WorkspacePackageVersionMissing { .. } => {
                "workspace_package_version_missing"
            }
            DiscoveryError::WorkspaceVersionFlagDisabled { .. } => {
                "workspace_version_flag_disabled"
            }
            DiscoveryError::WorkspaceBuildNotReady { .. } => "workspace_build_not_ready",
            DiscoveryError::WorkspaceNoMembers { .. } => "workspace_no_members",
            DiscoveryError::WorkspaceMembersNone { .. } => "workspace_members_none",
            DiscoveryError::WorkspaceMissingSection { .. } => "workspace_missing_section",
            DiscoveryError::WorkspacePathMismatch { .. } => "workspace_path_mismatch",
            DiscoveryError::MultipleWorkspacesDetected { .. } => "multiple_workspaces_detected",
            DiscoveryError::ParentNotFound { .. } => "parent_not_found",
        }
    }

    fn diagnostic_summary(&self) -> String {
        self.to_string()
    }

    fn diagnostic_detail(&self) -> Option<String> {
        match self {
            DiscoveryError::TomlParse { source, .. }
            | DiscoveryError::WorkspaceManifestParse { source, .. } => Some(source.message().into()),
            DiscoveryError::NonFatalErrors(errors) => {
                Some(format!("{} non-fatal discovery errors were collected", errors.len()))
            }
            _ => None,
        }
    }

    fn diagnostic_source_path(&self) -> Option<&Path> {
        match self {
            DiscoveryError::Io { path, .. }
            | DiscoveryError::TomlParse { path, .. }
            | DiscoveryError::MissingPackageName { path }
            | DiscoveryError::MissingPackageVersion { path }
            | DiscoveryError::CratePathNotFound { path }
            | DiscoveryError::Walkdir { path, .. }
            | DiscoveryError::SrcNotFound { path } => Some(path.as_path()),
            DiscoveryError::WorkspaceManifestRead { manifest_path, .. }
            | DiscoveryError::WorkspaceManifestParse { manifest_path, .. } => {
                Some(manifest_path.as_path())
            }
            DiscoveryError::WorkspaceManifestNotFound { crate_path }
            | DiscoveryError::WorkspacePackageVersionMissing {
                crate_path,
                manifest_path: _,
            }
            | DiscoveryError::WorkspaceVersionFlagDisabled {
                crate_path,
                manifest_path: _,
            } => Some(crate_path.as_path()),
            DiscoveryError::WorkspaceBuildNotReady { workspace_path, .. }
            | DiscoveryError::WorkspaceNoMembers { workspace_path, .. }
            | DiscoveryError::WorkspaceMembersNone { workspace_path, .. }
            | DiscoveryError::ParentNotFound { workspace_path } => Some(workspace_path.as_path()),
            DiscoveryError::WorkspaceMissingSection { workspace_path, .. } => {
                Some(workspace_path.as_path())
            }
            DiscoveryError::WorkspacePathMismatch {
                discovered_workspace_path,
                ..
            } => Some(discovered_workspace_path.as_path()),
            DiscoveryError::MultipleWorkspacesDetected {
                expected_workspace_path,
                ..
            } => Some(expected_workspace_path.as_path()),
            DiscoveryError::NonFatalErrors(_) => None,
        }
    }

    fn diagnostic_span(&self) -> Option<&dyn DiagnosticSpan> {
        match self {
            DiscoveryError::TomlParse { span, .. }
            | DiscoveryError::WorkspaceManifestParse { span, .. } => {
                span.as_ref().map(|span| span as &dyn DiagnosticSpan)
            }
            _ => None,
        }
    }

    fn diagnostic_context(&self) -> Vec<DiagnosticField> {
        match self {
            DiscoveryError::WorkspaceManifestRead {
                crate_path,
                manifest_path,
                ..
            }
            | DiscoveryError::WorkspaceManifestParse {
                crate_path,
                manifest_path,
                ..
            } => {
                let mut fields = vec![DiagnosticField {
                    key: "manifest_path",
                    value: manifest_path.display().to_string(),
                }];
                if let Some(crate_path) = crate_path {
                    fields.push(DiagnosticField {
                        key: "crate_path",
                        value: crate_path.display().to_string(),
                    });
                }
                fields
            }
            DiscoveryError::WorkspacePackageVersionMissing {
                crate_path,
                manifest_path,
            }
            | DiscoveryError::WorkspaceVersionFlagDisabled {
                crate_path,
                manifest_path,
            } => vec![
                DiagnosticField {
                    key: "crate_path",
                    value: crate_path.display().to_string(),
                },
                DiagnosticField {
                    key: "manifest_path",
                    value: manifest_path.display().to_string(),
                },
            ],
            _ => Vec::new(),
        }
    }

    fn diagnostic_emission_site(&self) -> Option<&DiagnosticSite> {
        match self {
            DiscoveryError::Io { emission_site, .. }
            | DiscoveryError::TomlParse { emission_site, .. }
            | DiscoveryError::Walkdir { emission_site, .. }
            | DiscoveryError::WorkspaceManifestRead { emission_site, .. }
            | DiscoveryError::WorkspaceManifestParse { emission_site, .. } => Some(emission_site),
            _ => None,
        }
    }

    fn diagnostic_backtrace(&self) -> Option<&Backtrace> {
        match self {
            DiscoveryError::Io { backtrace, .. }
            | DiscoveryError::TomlParse { backtrace, .. }
            | DiscoveryError::Walkdir { backtrace, .. }
            | DiscoveryError::WorkspaceManifestRead { backtrace, .. }
            | DiscoveryError::WorkspaceManifestParse { backtrace, .. } => Some(backtrace.as_ref()),
            _ => None,
        }
    }
}

impl DiscoveryError {
    #[track_caller]
    pub fn io(path: PathBuf, source: std::io::Error) -> Self {
        Self::Io {
            path,
            emission_site: DiagnosticSite::from_location(Location::caller()),
            backtrace: Arc::new(Backtrace::force_capture()),
            source: Arc::new(source),
        }
    }

    #[track_caller]
    pub fn toml_parse(path: PathBuf, span: Option<SourceSpan>, source: toml::de::Error) -> Self {
        Self::TomlParse {
            path,
            span,
            emission_site: DiagnosticSite::from_location(Location::caller()),
            backtrace: Arc::new(Backtrace::force_capture()),
            source: Arc::new(source),
        }
    }

    #[track_caller]
    pub fn walkdir(path: PathBuf, source: walkdir::Error) -> Self {
        Self::Walkdir {
            path,
            emission_site: DiagnosticSite::from_location(Location::caller()),
            backtrace: Arc::new(Backtrace::force_capture()),
            source: Arc::new(source),
        }
    }

    #[track_caller]
    pub fn workspace_manifest_read(
        crate_path: Option<PathBuf>,
        manifest_path: PathBuf,
        source: std::io::Error,
    ) -> Self {
        Self::WorkspaceManifestRead {
            crate_path,
            manifest_path,
            emission_site: DiagnosticSite::from_location(Location::caller()),
            backtrace: Arc::new(Backtrace::force_capture()),
            source: Arc::new(source),
        }
    }

    #[track_caller]
    pub fn workspace_manifest_parse(
        crate_path: Option<PathBuf>,
        manifest_path: PathBuf,
        span: Option<SourceSpan>,
        source: toml::de::Error,
    ) -> Self {
        Self::WorkspaceManifestParse {
            crate_path,
            manifest_path,
            span,
            emission_site: DiagnosticSite::from_location(Location::caller()),
            backtrace: Arc::new(Backtrace::force_capture()),
            source: Arc::new(source),
        }
    }

    pub fn diagnostic_source_span(&self) -> Option<&SourceSpan> {
        match self {
            DiscoveryError::TomlParse { span, .. }
            | DiscoveryError::WorkspaceManifestParse { span, .. } => span.as_ref(),
            _ => None,
        }
    }
}

pub fn toml_error_span(path: PathBuf, content: &str, error: &toml::de::Error) -> Option<SourceSpan> {
    let range = error.span()?;
    let (line, col) = byte_offset_to_line_col(content, range.start);
    Some(
        SourceSpan::new(path)
            .with_range(range.start, range.end)
            .with_line_col(line, col),
    )
}

fn byte_offset_to_line_col(content: &str, offset: usize) -> (u32, u32) {
    let clamped = offset.min(content.len());
    let slice = &content[..clamped];
    let line = slice.bytes().filter(|b| *b == b'\n').count() as u32 + 1;
    let col = slice
        .rsplit_once('\n')
        .map(|(_, tail)| tail.chars().count() as u32 + 1)
        .unwrap_or_else(|| slice.chars().count() as u32 + 1);
    (line, col)
}

impl Serialize for DiscoveryError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{self:?}"))
    }
}

/// Conversion from DiscoveryError to ploke_error::Er
///
/// This conversion maps discovery-phase errors to appropriate ploke_error variants
/// based on their severity and impact on the overall parsing process:
///
/// - **FatalError::FileOperation**: Used for I/O errors and walkdir errors during
///   file discovery, as these indicate filesystem access issues.
/// - **FatalError::PathResolution**: Used for TOML parse errors and missing
///   critical files (Cargo.toml, src directory). These prevent any meaningful parsing.
/// - **WarningError**: Used for non-fatal discovery issues that don't prevent
///   parsing but may affect completeness.
///
/// This mapping maintains the dependency direction (syn_parser depends on ploke_error)
/// while providing clear error categorization for upstream error handling.
impl From<DiscoveryError> for ploke_error::Error {
    fn from(err: DiscoveryError) -> Self {
        let err_string = err.to_string();
        match err {
            DiscoveryError::Io { path, source, .. } => ploke_error::FatalError::FileOperation {
                operation: "read",
                path,
                source,
            }
            .into(),
            DiscoveryError::TomlParse { path, source, .. } => ploke_error::FatalError::PathResolution {
                path: format!("Failed to parse Cargo.toml at {}", path.display()),
                source: Some(source),
            }
            .into(),
            DiscoveryError::MissingPackageName { path } => {
                ploke_error::FatalError::PathResolution {
                    path: format!("Missing package.name in Cargo.toml at {}", path.display()),
                    source: None,
                }
                .into()
            }
            DiscoveryError::MissingPackageVersion { path } => {
                ploke_error::FatalError::PathResolution {
                    path: format!(
                        "Missing package.version in Cargo.toml at {}",
                        path.display()
                    ),
                    source: None,
                }
                .into()
            }
            DiscoveryError::CratePathNotFound { path } => ploke_error::FatalError::PathResolution {
                path: format!("Crate path not found: {}", path.display()),
                source: None,
            }
            .into(),
            DiscoveryError::Walkdir { path, source, .. } => {
                // Convert walkdir::Error to std::io::Error using string representation
                let io_error = std::io::Error::other(source.to_string());
                ploke_error::FatalError::FileOperation {
                    operation: "walk",
                    path,
                    source: Arc::new(io_error),
                }
                .into()
            }
            DiscoveryError::SrcNotFound { path } => ploke_error::FatalError::PathResolution {
                path: format!(
                    "Source directory not found for crate at: {}",
                    path.display()
                ),
                source: None,
            }
            .into(),
            DiscoveryError::NonFatalErrors(errors) => {
                // Convert the boxed vector to a warning about multiple issues
                ploke_error::WarningError::UnresolvedRef {
                    name: "Discovery phase".to_string(),
                    location: Some(format!("{} non-fatal errors occurred", errors.len())),
                }
                .into()
            }
            DiscoveryError::WorkspaceManifestRead { source, .. } => {
                ploke_error::FatalError::PathResolution {
                    path: err_string,
                    source: Some(source),
                }
                .into()
            }
            DiscoveryError::WorkspaceManifestParse {
                crate_path,
                manifest_path,
                span: _,
                source,
                ..
            } => ploke_error::FatalError::PathResolution {
                path: format!(
                    "Failed to parse workspace manifest {} for crate {:?}",
                    manifest_path.display(),
                    crate_path
                ),
                source: Some(source),
            }
            .into(),
            DiscoveryError::WorkspaceManifestNotFound { crate_path } => {
                ploke_error::FatalError::PathResolution {
                    path: format!(
                        "Workspace manifest not found for crate {}",
                        crate_path.display(),
                    ),
                    source: None,
                }
                .into()
            }
            DiscoveryError::WorkspacePackageVersionMissing {
                crate_path,
                manifest_path,
            } => ploke_error::FatalError::PathResolution {
                path: format!(
                    "workspace.package.version missing in {} required by crate {}",
                    manifest_path.display(),
                    crate_path.display()
                ),
                source: None,
            }
            .into(),
            DiscoveryError::WorkspaceVersionFlagDisabled {
                crate_path,
                manifest_path,
            } => ploke_error::FatalError::PathResolution {
                path: format!(
                    "`package.version.workspace` must be true in {} for crate {}",
                    manifest_path.display(),
                    crate_path.display()
                ),
                source: None,
            }
            .into(),
            DiscoveryError::WorkspaceBuildNotReady {
                workspace_path,
                build_status,
            } => ploke_error::FatalError::PathResolution {
                path: format!(
                    "Cannot build workspace manifest at {} (build_status: {})",
                    workspace_path.display(),
                    build_status
                ),
                source: None,
            }
            .into(),
            DiscoveryError::WorkspaceNoMembers {
                workspace_path,
                build_status,
            } => ploke_error::FatalError::PathResolution {
                path: format!(
                    "Workspace at {} has no members (build_status: {})",
                    workspace_path.display(),
                    build_status
                ),
                source: None,
            }
            .into(),
            DiscoveryError::WorkspaceMembersNone {
                workspace_path,
                build_status,
            } => ploke_error::FatalError::PathResolution {
                path: format!(
                    "Workspace at {} has members = None (build_status: {})",
                    workspace_path.display(),
                    build_status
                ),
                source: None,
            }
            .into(),
            ref err @ DiscoveryError::ParentNotFound { ref workspace_path } => {
                ploke_error::FatalError::PathResolution {
                    path: workspace_path.display().to_string(),
                    source: Some(Arc::new(err.clone())),
                }
                .into()
            }
            ref err @ DiscoveryError::WorkspaceMissingSection {
                ref workspace_path, ..
            } => ploke_error::FatalError::PathResolution {
                path: workspace_path.display().to_string(),
                source: Some(Arc::new(err.clone())),
            }
            .into(),
            ref err @ DiscoveryError::WorkspacePathMismatch {
                ref expected_workspace_path,
                ..
            } => ploke_error::FatalError::PathResolution {
                path: expected_workspace_path.display().to_string(),
                source: Some(Arc::new(err.clone())),
            }
            .into(),
            ref err @ DiscoveryError::MultipleWorkspacesDetected {
                ref expected_workspace_path,
                ..
            } => ploke_error::FatalError::PathResolution {
                path: expected_workspace_path.display().to_string(),
                source: Some(Arc::new(err.clone())),
            }
            .into(),
        }
    }
}
