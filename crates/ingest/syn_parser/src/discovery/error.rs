use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use toml;

use crate::error::SynParserError;
use thiserror::Error;

/// Errors that can occur during the discovery phase.
#[derive(Error, Debug, Clone)] // Add Clone derive
pub enum DiscoveryError {
    /// An I/O error occurred while accessing a path.
    #[error("I/O error accessing path {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: Arc<std::io::Error>, // Wrap in Arc
    },
    /// Failed to parse a `Cargo.toml` file.
    #[error("Failed to parse Cargo.toml at {path}: {source}")]
    TomlParse {
        path: PathBuf,
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

impl TryFrom<DiscoveryError> for SynParserError {
    type Error = SynParserError;

    fn try_from(value: DiscoveryError) -> Result<Self, Self::Error> {
        use DiscoveryError::*;
        let source_string = value.to_string();
        Ok(match value {
            MissingPackageName { path } => SynParserError::SimpleDiscovery {
                path: path.display().to_string(),
            },
            MissingPackageVersion { path } => SynParserError::SimpleDiscovery {
                path: path.display().to_string(),
            },
            CratePathNotFound { path } => SynParserError::SimpleDiscovery {
                path: path.display().to_string(),
            },
            SrcNotFound { path } => SynParserError::SimpleDiscovery {
                path: path.display().to_string(),
            },
            Io { path, .. } => SynParserError::ComplexDiscovery {
                name: "".to_string(),
                path: path.display().to_string(),
                source_string: "Io".to_string(),
            },
            TomlParse { path, .. } => SynParserError::ComplexDiscovery {
                name: "".to_string(),
                path: path.display().to_string(),
                source_string: "Toml".to_string(),
            },
            Walkdir { path, .. } => SynParserError::ComplexDiscovery {
                name: "".to_string(),
                path: path.display().to_string(),
                source_string: "walkdir".to_string(),
            },
            WorkspaceManifestRead {
                crate_path,
                manifest_path,
                ..
            } => SynParserError::ComplexDiscovery {
                name: crate_path
                    .map(|path_buf| path_buf.display().to_string())
                    .unwrap_or_else(|| String::from("None")),
                path: manifest_path.display().to_string(),
                source_string: "WorkspaceManifestRead".to_string(),
            },
            WorkspaceManifestParse { manifest_path, .. } => SynParserError::ComplexDiscovery {
                name: "WorkspaceManifestParse".to_string(),
                path: manifest_path.display().to_string(),
                source_string,
            },
            WorkspaceManifestNotFound { crate_path } => SynParserError::SimpleDiscovery {
                path: crate_path.display().to_string(),
            },
            WorkspacePackageVersionMissing {
                crate_path,
                manifest_path,
            } => SynParserError::ComplexDiscovery {
                name: crate_path.display().to_string(),
                path: manifest_path.display().to_string(),
                source_string: "WorkspacePackageVersionMissing".to_string(),
            },
            WorkspaceVersionFlagDisabled {
                crate_path,
                manifest_path,
            } => SynParserError::ComplexDiscovery {
                name: crate_path.display().to_string(),
                path: manifest_path.display().to_string(),
                source_string: "WorkspaceVersionFlagDisabled".to_string(),
            },
            NonFatalErrors(errors) => {
                let nested = errors
                    .into_iter()
                    .map(SynParserError::try_from)
                    .collect::<Result<Vec<_>, _>>()?;
                SynParserError::MultipleErrors(nested)
            }
            WorkspaceBuildNotReady {
                workspace_path,
                build_status,
            } => SynParserError::ComplexDiscovery {
                name: "workspace".to_string(),
                path: workspace_path.display().to_string(),
                source_string: format!("WorkspaceBuildNotReady (build_status: {build_status})"),
            },
            WorkspaceNoMembers {
                workspace_path,
                build_status,
            } => SynParserError::ComplexDiscovery {
                name: "workspace".to_string(),
                path: workspace_path.display().to_string(),
                source_string: format!("WorkspaceNoMembers (build_status: {build_status})"),
            },
            WorkspaceMembersNone {
                workspace_path,
                build_status,
            } => SynParserError::ComplexDiscovery {
                name: "workspace".to_string(),
                path: workspace_path.display().to_string(),
                source_string: format!("WorkspaceMembersNone (build_status: {build_status})"),
            },
            ParentNotFound { workspace_path } => SynParserError::SimpleDiscovery {
                path: workspace_path.display().to_string(),
            },
            WorkspaceMissingSection { workspace_path, .. } => SynParserError::ComplexDiscovery {
                name: "workspace".to_string(),
                path: workspace_path.display().to_string(),
                source_string,
            },
            WorkspacePathMismatch {
                expected_workspace_path,
                ..
            } => SynParserError::ComplexDiscovery {
                name: "workspace".to_string(),
                path: expected_workspace_path.display().to_string(),
                source_string,
            },
            MultipleWorkspacesDetected {
                expected_workspace_path,
                ..
            } => SynParserError::ComplexDiscovery {
                name: "workspace".to_string(),
                path: expected_workspace_path.display().to_string(),
                source_string,
            },
        })
    }
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
            DiscoveryError::Io { path, source } => ploke_error::FatalError::FileOperation {
                operation: "read",
                path,
                source,
            }
            .into(),
            DiscoveryError::TomlParse { path, source } => ploke_error::FatalError::PathResolution {
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
            DiscoveryError::Walkdir { path, source } => {
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
                source,
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
