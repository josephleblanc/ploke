use std::backtrace::Backtrace;
use std::fmt;
use std::fs;
use std::panic::Location;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ploke_error::{DiagnosticField, DiagnosticInfo, DiagnosticSite, DiagnosticSpan, SourceSpan};
use serde::Serialize;
use toml;

use thiserror::Error;

/// Classifies which manifest is being read or parsed for diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ManifestKind {
    /// The target crate's `Cargo.toml` (e.g. discovery for a member crate).
    Crate,
    /// The workspace root `Cargo.toml` when that path is known to be a workspace boundary.
    WorkspaceRoot,
    /// A manifest inspected while walking ancestors from a crate (workspace discovery).
    AncestorWorkspace,
}

impl fmt::Display for ManifestKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ManifestKind::Crate => "crate",
            ManifestKind::WorkspaceRoot => "workspace_root",
            ManifestKind::AncestorWorkspace => "ancestor_workspace",
        })
    }
}

/// Structured context for manifest read/parse operations and [`WithDiscoveryManifestRead`] /
/// [`WithDiscoveryManifestToml`].
#[derive(Clone)]
pub struct ManifestCtx<'a> {
    pub kind: ManifestKind,
    pub manifest_path: PathBuf,
    pub crate_path: Option<PathBuf>,
    pub content: Option<&'a str>,
}

impl<'a> ManifestCtx<'a> {
    pub fn with_content<'b>(&self, content: &'b str) -> ManifestCtx<'b> {
        ManifestCtx {
            kind: self.kind,
            manifest_path: self.manifest_path.clone(),
            crate_path: self.crate_path.clone(),
            content: Some(content),
        }
    }
}

/// Adapter for [`Result<String, std::io::Error>`] → [`DiscoveryError::ManifestRead`].
pub struct ManifestReadOutcome<'a> {
    result: Result<String, std::io::Error>,
    ctx: ManifestCtx<'a>,
}

/// Maps an I/O result from reading a manifest into [`DiscoveryError::ManifestRead`] via [`ManifestReadOutcome::for_read`].
pub trait WithDiscoveryManifestRead: Sized {
    #[track_caller]
    fn with_discovery_err(self, ctx: ManifestCtx<'_>) -> ManifestReadOutcome<'_>;
}

impl WithDiscoveryManifestRead for Result<String, std::io::Error> {
    #[track_caller]
    fn with_discovery_err(self, ctx: ManifestCtx<'_>) -> ManifestReadOutcome<'_> {
        ManifestReadOutcome { result: self, ctx }
    }
}

impl<'a> ManifestReadOutcome<'a> {
    #[track_caller]
    pub fn for_read(self) -> Result<String, DiscoveryError> {
        let emission_site = DiagnosticSite::from_location(Location::caller());
        self.result
            .map_err(|e| DiscoveryError::manifest_read_at_site(self.ctx, e, emission_site.clone()))
    }
}

/// Adapter for [`Result<T, toml::de::Error>`] → [`DiscoveryError::ManifestParse`].
pub struct ManifestTomlOutcome<'a, T> {
    result: Result<T, toml::de::Error>,
    ctx: ManifestCtx<'a>,
}

/// Maps a TOML deserialize error into [`DiscoveryError::ManifestParse`] via [`ManifestTomlOutcome::for_toml`].
pub trait WithDiscoveryManifestToml<T>: Sized {
    #[track_caller]
    fn with_discovery_err(self, ctx: ManifestCtx<'_>) -> ManifestTomlOutcome<'_, T>;
}

impl<T> WithDiscoveryManifestToml<T> for Result<T, toml::de::Error> {
    #[track_caller]
    fn with_discovery_err(self, ctx: ManifestCtx<'_>) -> ManifestTomlOutcome<'_, T> {
        ManifestTomlOutcome { result: self, ctx }
    }
}

impl<'a, T> ManifestTomlOutcome<'a, T> {
    #[track_caller]
    pub fn for_toml(self) -> Result<T, DiscoveryError> {
        let emission_site = DiagnosticSite::from_location(Location::caller());
        self.result.map_err(|e| {
            DiscoveryError::manifest_parse_toml_at_site(self.ctx, e, emission_site.clone())
        })
    }
}

/// Adapter for [`Result<T, cargo_toml::Error>`] → [`DiscoveryError`] (read vs parse, see [`discovery_error_from_cargo_toml`]).
pub struct ManifestCargoTomlOutcome<'a, T> {
    result: Result<T, cargo_toml::Error>,
    ctx: ManifestCtx<'a>,
}

/// Maps a `cargo_toml` load/parse error into [`DiscoveryError`] via [`ManifestCargoTomlOutcome::for_cargo_toml`].
pub trait WithDiscoveryManifestCargoToml<T>: Sized {
    #[track_caller]
    fn with_discovery_err(self, ctx: ManifestCtx<'_>) -> ManifestCargoTomlOutcome<'_, T>;
}

impl<T> WithDiscoveryManifestCargoToml<T> for Result<T, cargo_toml::Error> {
    #[track_caller]
    fn with_discovery_err(self, ctx: ManifestCtx<'_>) -> ManifestCargoTomlOutcome<'_, T> {
        ManifestCargoTomlOutcome {
            result: self,
            ctx,
        }
    }
}

impl<'a, T> ManifestCargoTomlOutcome<'a, T> {
    #[track_caller]
    pub fn for_cargo_toml(self) -> Result<T, DiscoveryError> {
        let emission_site = DiagnosticSite::from_location(Location::caller());
        self.result
            .map_err(|e| discovery_error_from_cargo_toml_at_site(self.ctx, e, emission_site))
    }
}

/// Cloneable parse error payload for [`DiscoveryError::ManifestParse`] (TOML deserialize, `cargo_toml`, or other).
#[derive(Clone)]
pub struct ManifestParseSource(pub Arc<dyn std::error::Error + Send + Sync + 'static>);

impl fmt::Debug for ManifestParseSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ManifestParseSource")
            .field(&format_args!("{}", self.0.as_ref()))
            .finish()
    }
}

impl fmt::Display for ManifestParseSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.0.as_ref(), f)
    }
}

impl std::error::Error for ManifestParseSource {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.0.as_ref())
    }
}

/// If `err` or a nested [`cargo_toml::Error::Workspace`] contains a TOML deserialize failure, return it for span extraction.
pub fn toml_de_error_from_cargo_toml(err: &cargo_toml::Error) -> Option<&toml::de::Error> {
    match err {
        cargo_toml::Error::Parse(e) => Some(e),
        cargo_toml::Error::Workspace(boxed) => toml_de_error_from_cargo_toml(&boxed.0),
        _ => None,
    }
}

/// Maps [`cargo_toml::Error`] from [`cargo_toml::Manifest::from_path`] into [`DiscoveryError`].
#[track_caller]
pub fn discovery_error_from_cargo_toml(
    ctx: ManifestCtx<'_>,
    err: cargo_toml::Error,
) -> DiscoveryError {
    let emission_site = DiagnosticSite::from_location(Location::caller());
    discovery_error_from_cargo_toml_at_site(ctx, err, emission_site)
}

fn discovery_error_from_cargo_toml_at_site(
    ctx: ManifestCtx<'_>,
    err: cargo_toml::Error,
    emission_site: DiagnosticSite,
) -> DiscoveryError {
    match err {
        cargo_toml::Error::Io(e) => DiscoveryError::manifest_read_at_site(ctx, e, emission_site),
        cargo_toml::Error::Parse(e) => {
            let content = fs::read_to_string(&ctx.manifest_path).ok();
            let ctx = match content.as_ref() {
                Some(c) => ctx.with_content(c.as_str()),
                None => ctx,
            };
            DiscoveryError::manifest_parse_toml_at_site(ctx, *e, emission_site)
        }
        e => {
            let content = fs::read_to_string(&ctx.manifest_path).ok();
            let ctx = match content.as_ref() {
                Some(c) => ctx.with_content(c.as_str()),
                None => ctx,
            };
            let span = toml_de_error_from_cargo_toml(&e).and_then(|te| {
                ctx.content
                    .and_then(|c| toml_error_span(ctx.manifest_path.clone(), c, te))
            });
            DiscoveryError::manifest_parse_dyn_at_site(
                ctx,
                ManifestParseSource(Arc::new(e)),
                span,
                emission_site,
            )
        }
    }
}

/// Errors that can occur during the discovery phase.
#[derive(Error, Debug, Clone)] // Add Clone derive
pub enum DiscoveryError {
    /// Failed to read a `Cargo.toml` (or other manifest path) during discovery.
    #[error("Failed to read manifest at {manifest_path} ({manifest_kind}): {source}")]
    ManifestRead {
        manifest_kind: ManifestKind,
        manifest_path: PathBuf,
        crate_path: Option<PathBuf>,
        emission_site: DiagnosticSite,
        backtrace: Arc<Backtrace>,
        #[source]
        source: Arc<std::io::Error>, // Wrap in Arc
    },
    /// Failed to deserialize or complete a manifest (TOML, `cargo_toml`, etc.).
    #[error("Failed to parse manifest at {manifest_path} ({manifest_kind}): {source}")]
    ManifestParse {
        manifest_kind: ManifestKind,
        manifest_path: PathBuf,
        crate_path: Option<PathBuf>,
        span: Option<SourceSpan>,
        emission_site: DiagnosticSite,
        backtrace: Arc<Backtrace>,
        #[source]
        source: ManifestParseSource,
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
            DiscoveryError::ManifestRead { .. } => "manifest_read",
            DiscoveryError::ManifestParse { .. } => "manifest_parse",
            DiscoveryError::MissingPackageName { .. } => "manifest_missing_package_name",
            DiscoveryError::MissingPackageVersion { .. } => "manifest_missing_package_version",
            DiscoveryError::CratePathNotFound { .. } => "crate_path_not_found",
            DiscoveryError::Walkdir { .. } => "discovery_walkdir",
            DiscoveryError::SrcNotFound { .. } => "src_not_found",
            DiscoveryError::NonFatalErrors(_) => "discovery_non_fatal_errors",
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
            DiscoveryError::ManifestParse { source, .. } => {
                Some(manifest_parse_detail_message(source))
            }
            DiscoveryError::NonFatalErrors(errors) => Some(format!(
                "{} non-fatal discovery errors were collected",
                errors.len()
            )),
            _ => None,
        }
    }

    fn diagnostic_source_path(&self) -> Option<&Path> {
        match self {
            DiscoveryError::ManifestRead { manifest_path, .. }
            | DiscoveryError::ManifestParse { manifest_path, .. } => Some(manifest_path.as_path()),
            DiscoveryError::MissingPackageName { path }
            | DiscoveryError::MissingPackageVersion { path }
            | DiscoveryError::CratePathNotFound { path }
            | DiscoveryError::Walkdir { path, .. }
            | DiscoveryError::SrcNotFound { path } => Some(path.as_path()),
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
            DiscoveryError::ManifestParse { span, .. } => {
                span.as_ref().map(|span| span as &dyn DiagnosticSpan)
            }
            _ => None,
        }
    }

    fn diagnostic_context(&self) -> Vec<DiagnosticField> {
        match self {
            DiscoveryError::ManifestRead {
                manifest_kind,
                crate_path,
                manifest_path,
                ..
            }
            | DiscoveryError::ManifestParse {
                manifest_kind,
                crate_path,
                manifest_path,
                ..
            } => {
                let mut fields = vec![
                    DiagnosticField {
                        key: "manifest_kind",
                        value: manifest_kind.to_string(),
                    },
                    DiagnosticField {
                        key: "manifest_path",
                        value: manifest_path.display().to_string(),
                    },
                ];
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
            DiscoveryError::ManifestRead { emission_site, .. }
            | DiscoveryError::ManifestParse { emission_site, .. }
            | DiscoveryError::Walkdir { emission_site, .. } => Some(emission_site),
            _ => None,
        }
    }

    fn diagnostic_backtrace(&self) -> Option<&Backtrace> {
        match self {
            DiscoveryError::ManifestRead { backtrace, .. }
            | DiscoveryError::ManifestParse { backtrace, .. }
            | DiscoveryError::Walkdir { backtrace, .. } => Some(backtrace.as_ref()),
            _ => None,
        }
    }
}

impl DiscoveryError {
    fn manifest_read_at_site(
        ctx: ManifestCtx<'_>,
        source: std::io::Error,
        emission_site: DiagnosticSite,
    ) -> Self {
        Self::ManifestRead {
            manifest_kind: ctx.kind,
            manifest_path: ctx.manifest_path,
            crate_path: ctx.crate_path,
            emission_site,
            backtrace: Arc::new(Backtrace::force_capture()),
            source: Arc::new(source),
        }
    }

    #[track_caller]
    pub fn manifest_read(ctx: ManifestCtx<'_>, source: std::io::Error) -> Self {
        Self::manifest_read_at_site(
            ctx,
            source,
            DiagnosticSite::from_location(Location::caller()),
        )
    }

    fn manifest_parse_toml_at_site(
        ctx: ManifestCtx<'_>,
        source: toml::de::Error,
        emission_site: DiagnosticSite,
    ) -> Self {
        let span = ctx
            .content
            .and_then(|content| toml_error_span(ctx.manifest_path.clone(), content, &source));
        Self::ManifestParse {
            manifest_kind: ctx.kind,
            manifest_path: ctx.manifest_path,
            crate_path: ctx.crate_path,
            span,
            emission_site,
            backtrace: Arc::new(Backtrace::force_capture()),
            source: ManifestParseSource(Arc::new(source)),
        }
    }

    fn manifest_parse_dyn_at_site(
        ctx: ManifestCtx<'_>,
        source: ManifestParseSource,
        span: Option<SourceSpan>,
        emission_site: DiagnosticSite,
    ) -> Self {
        Self::ManifestParse {
            manifest_kind: ctx.kind,
            manifest_path: ctx.manifest_path,
            crate_path: ctx.crate_path,
            span,
            emission_site,
            backtrace: Arc::new(Backtrace::force_capture()),
            source,
        }
    }

    #[track_caller]
    pub fn manifest_parse(ctx: ManifestCtx<'_>, source: toml::de::Error) -> Self {
        Self::manifest_parse_toml_at_site(
            ctx,
            source,
            DiagnosticSite::from_location(Location::caller()),
        )
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

    pub fn diagnostic_source_span(&self) -> Option<&SourceSpan> {
        match self {
            DiscoveryError::ManifestParse { span, .. } => span.as_ref(),
            _ => None,
        }
    }
}

fn manifest_parse_detail_message(source: &ManifestParseSource) -> String {
    source
        .0
        .as_ref()
        .downcast_ref::<toml::de::Error>()
        .map(|e| e.message().into())
        .unwrap_or_else(|| source.to_string())
}

pub fn toml_error_span(
    path: PathBuf,
    content: &str,
    error: &toml::de::Error,
) -> Option<SourceSpan> {
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
/// - **FatalError::FileOperation**: Used for manifest read and walkdir errors during
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
        match err {
            DiscoveryError::ManifestRead {
                manifest_path,
                source,
                ..
            } => ploke_error::FatalError::FileOperation {
                operation: "read",
                path: manifest_path,
                source,
            }
            .into(),
            DiscoveryError::ManifestParse {
                manifest_path,
                source,
                ..
            } => ploke_error::FatalError::PathResolution {
                path: format!("Failed to parse manifest at {}", manifest_path.display()),
                source: Some(source.0.clone()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_de_error_from_cargo_toml_extracts_parse() {
        let bad = "not toml {{{";
        let tom_err = toml::from_str::<toml::Value>(bad).unwrap_err();
        let cargo_err = cargo_toml::Error::Parse(Box::new(tom_err));
        assert!(toml_de_error_from_cargo_toml(&cargo_err).is_some());
    }

    #[test]
    fn toml_de_error_from_cargo_toml_recurses_workspace() {
        let bad = "x";
        let tom_err = toml::from_str::<toml::Value>(bad).unwrap_err();
        let inner = cargo_toml::Error::Parse(Box::new(tom_err));
        let ws = cargo_toml::Error::Workspace(Box::new((inner, None)));
        assert!(toml_de_error_from_cargo_toml(&ws).is_some());
    }

    #[test]
    fn discovery_error_from_cargo_toml_non_parse_maps_to_manifest_parse() {
        let ctx = ManifestCtx {
            kind: ManifestKind::Crate,
            manifest_path: PathBuf::from("/tmp/nonexistent/Cargo.toml"),
            crate_path: None,
            content: None,
        };
        let err = cargo_toml::Error::InheritedUnknownValue;
        let d = discovery_error_from_cargo_toml(ctx, err);
        assert!(matches!(d, DiscoveryError::ManifestParse { .. }));
    }
}
