use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use ploke_common::workspace_root;
use ploke_core::embeddings::{
    EmbeddingDType, EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
};
use ploke_db::{Database, DbError, create_index_primary, multi_embedding::db_ext::EmbeddingExt};
use ploke_error::Error;
use uuid::Uuid;

use once_cell::sync::Lazy;

static SHARED_FIXTURE_DBS: Lazy<Mutex<HashMap<&'static str, Arc<Database>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub const PLOKE_DB_SNAPSHOT_FIXTURE_DIR_ENV: &str = "PLOKE_DB_SNAPSHOT_FIXTURE_DIR";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureAccess {
    ImmutableShared,
    FreshMutable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureImportMode {
    PlainBackup,
    BackupWithEmbeddings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureStatus {
    Active,
    Planned,
    TypedTypeGraph,
    Legacy,
    Orphaned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixturePathScope {
    /// The backup contains checkout-specific absolute roots and should prefer a
    /// root-scoped local copy when one has been generated.
    CheckoutLocal,
    /// The backup is generated from pinned external source, so it can be shared
    /// across local worktrees without encoding this repository checkout root.
    SharedSnapshot,
    /// Historical backup retained only until its remaining consumers are
    /// confirmed or deleted.
    LegacyAbsolute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixturePathOrigin {
    CheckoutLocal,
    SharedSnapshot,
    Registered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedFixturePath {
    path: PathBuf,
    registered_path: PathBuf,
    origin: FixturePathOrigin,
}

impl CheckedFixturePath {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn registered_path(&self) -> &Path {
        &self.registered_path
    }

    pub fn into_path(self) -> PathBuf {
        self.path
    }
}

pub struct LoadedBackupFixture {
    path: CheckedFixturePath,
    db: Database,
}

impl LoadedBackupFixture {
    pub fn path(&self) -> &CheckedFixturePath {
        &self.path
    }

    pub fn into_db(self) -> Database {
        self.db
    }

    pub fn into_parts(self) -> (CheckedFixturePath, Database) {
        (self.path, self.db)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FixturePathCandidate {
    path: PathBuf,
    registered_path: PathBuf,
    origin: FixturePathOrigin,
}

impl FixturePathCandidate {
    fn checked(self) -> CheckedFixturePath {
        CheckedFixturePath {
            path: self.path,
            registered_path: self.registered_path,
            origin: self.origin,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureCreationStrategy {
    Automated(FixtureAutomation),
    Manual(FixtureManualRecreation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureAutomation {
    FixtureCrateMultiEmbedding {
        fixture_name: &'static str,
        output_stem: &'static str,
    },
    FixtureCrateLocalEmbeddings {
        fixture_name: &'static str,
        output_stem: &'static str,
    },
    WorkspaceFixture {
        fixture_name: &'static str,
        output_stem: &'static str,
    },
    WorkspaceCrate {
        crate_name: &'static str,
        output_stem: &'static str,
    },
    /// Clone or reuse a pinned GitHub corpus checkout and transform it into a
    /// plain graph backup. These fixtures are intentionally source-pinned:
    /// the checkout identity is part of the fixture contract, not ambient
    /// local state under `tests/fixture_github_clones/corpus`.
    GithubCorpusCrate {
        normalized_repo: &'static str,
        checkout_slug: &'static str,
        clone_url: &'static str,
        rev: &'static str,
        output_stem: &'static str,
    },
    /// Clone or reuse a pinned GitHub corpus checkout, parse it with typed
    /// graph relations, and run the OpenRouter embedding indexer before backup.
    GithubCorpusCrateOpenRouterEmbeddings {
        normalized_repo: &'static str,
        checkout_slug: &'static str,
        clone_url: &'static str,
        rev: &'static str,
        output_stem: &'static str,
    },
    /// Clone or reuse a pinned GitHub corpus checkout, parse selected member
    /// crates from its Cargo workspace, and transform them into a plain graph
    /// backup. Use this when the source-pinned corpus shape crosses local
    /// workspace crates or the repository root is a virtual manifest.
    GithubCorpusWorkspaceTargets {
        normalized_repo: &'static str,
        checkout_slug: &'static str,
        clone_url: &'static str,
        rev: &'static str,
        target_relative_paths: &'static [&'static str],
        output_stem: &'static str,
    },
    /// Clone or reuse a pinned GitHub corpus workspace checkout, parse selected
    /// member crates, and run the OpenRouter embedding indexer before backup.
    GithubCorpusWorkspaceTargetsOpenRouterEmbeddings {
        normalized_repo: &'static str,
        checkout_slug: &'static str,
        clone_url: &'static str,
        rev: &'static str,
        target_relative_paths: &'static [&'static str],
        output_stem: &'static str,
    },
    /// Extract a single member crate from a workspace fixture.
    /// This creates a DB with only one crate's graph plus workspace metadata,
    /// simulating the "focused crate" scenario in a workspace.
    FixtureWorkspaceMember {
        fixture_name: &'static str,
        member_crate: &'static str,
        output_stem: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixtureManualRecreation {
    pub output_stem: &'static str,
    pub summary: &'static str,
    pub steps: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixtureEmbeddingExpectation {
    pub provider: &'static str,
    pub model: &'static str,
    pub dims: u32,
    pub dtype: &'static str,
    pub vectors_present: bool,
    pub active_set_expected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixtureDb {
    pub id: &'static str,
    pub rel_path: &'static str,
    pub parsed_targets: &'static [&'static str],
    pub status: FixtureStatus,
    pub creation: FixtureCreationStrategy,
    pub path_scope: FixturePathScope,
    pub default_access: FixtureAccess,
    pub import_mode: FixtureImportMode,
    pub requires_primary_index: bool,
    pub bm25_index_expected: bool,
    pub embedding: Option<FixtureEmbeddingExpectation>,
    pub last_updated: &'static str,
    pub notes: &'static str,
}

impl FixtureDb {
    /// Returns the current candidate path without validating that the backup
    /// contents match this checkout. Operational consumers should use
    /// [`FixtureDb::checked_path`] or [`load_backup_fixture_db`].
    pub fn path(&self) -> PathBuf {
        self.path_candidate().path
    }

    pub fn checked_path(&self) -> Result<CheckedFixturePath, Error> {
        let fixture = backup_db_fixture(self.id).ok_or_else(|| {
            Error::from(DbError::Cozo(format!(
                "Backup fixture {} is not registered in BACKUP_DB_FIXTURES",
                self.id
            )))
        })?;
        Ok(load_backup_fixture_db(fixture)?.path)
    }

    pub fn registered_path(&self) -> PathBuf {
        workspace_root().join(self.rel_path)
    }

    pub fn repo_path(&self) -> PathBuf {
        self.registered_path()
    }

    pub fn shared_snapshot_path(&self) -> PathBuf {
        backup_db_snapshot_fixture_dir().join(self.filename())
    }

    pub fn checkout_local_path(&self) -> Option<PathBuf> {
        match self.path_scope {
            FixturePathScope::CheckoutLocal => Some(
                workspace_root()
                    .join("tests/backup_dbs/local")
                    .join(format!(
                        "{}__root-{}.sqlite",
                        self.output_stem(),
                        workspace_root_key()
                    )),
            ),
            FixturePathScope::SharedSnapshot | FixturePathScope::LegacyAbsolute => None,
        }
    }

    fn path_candidate(&self) -> FixturePathCandidate {
        let registered_path = self.registered_path();
        if let Some(path) = self.checkout_local_path()
            && path.exists()
        {
            return FixturePathCandidate {
                path,
                registered_path,
                origin: FixturePathOrigin::CheckoutLocal,
            };
        }
        if matches!(
            self.path_scope,
            FixturePathScope::CheckoutLocal | FixturePathScope::SharedSnapshot
        ) {
            return FixturePathCandidate {
                path: self.shared_snapshot_path(),
                registered_path,
                origin: FixturePathOrigin::SharedSnapshot,
            };
        }
        FixturePathCandidate {
            path: registered_path.clone(),
            registered_path,
            origin: FixturePathOrigin::Registered,
        }
    }

    pub fn filename(&self) -> &'static str {
        self.rel_path
            .rsplit('/')
            .next()
            .expect("fixture rel_path should include a filename")
    }

    pub fn expected_embedding_set(&self) -> Option<EmbeddingSet> {
        self.embedding
            .map(FixtureEmbeddingExpectation::embedding_set)
    }

    pub fn output_stem(&self) -> &'static str {
        match self.creation {
            FixtureCreationStrategy::Automated(FixtureAutomation::FixtureCrateMultiEmbedding {
                output_stem,
                ..
            })
            | FixtureCreationStrategy::Automated(
                FixtureAutomation::FixtureCrateLocalEmbeddings { output_stem, .. },
            )
            | FixtureCreationStrategy::Automated(FixtureAutomation::WorkspaceFixture {
                output_stem,
                ..
            })
            | FixtureCreationStrategy::Automated(FixtureAutomation::WorkspaceCrate {
                output_stem,
                ..
            })
            | FixtureCreationStrategy::Automated(FixtureAutomation::GithubCorpusCrate {
                output_stem,
                ..
            })
            | FixtureCreationStrategy::Automated(
                FixtureAutomation::GithubCorpusCrateOpenRouterEmbeddings { output_stem, .. },
            )
            | FixtureCreationStrategy::Automated(
                FixtureAutomation::GithubCorpusWorkspaceTargets { output_stem, .. },
            )
            | FixtureCreationStrategy::Automated(
                FixtureAutomation::GithubCorpusWorkspaceTargetsOpenRouterEmbeddings {
                    output_stem,
                    ..
                },
            )
            | FixtureCreationStrategy::Automated(FixtureAutomation::FixtureWorkspaceMember {
                output_stem,
                ..
            }) => output_stem,
            FixtureCreationStrategy::Manual(FixtureManualRecreation { output_stem, .. }) => {
                output_stem
            }
        }
    }
}

pub fn backup_db_snapshot_fixture_dir() -> PathBuf {
    if let Some(path) = env::var_os(PLOKE_DB_SNAPSHOT_FIXTURE_DIR_ENV) {
        return PathBuf::from(path);
    }
    user_config_local_dir()
        .join("ploke")
        .join("db_snapshot_fixtures")
}

fn user_config_local_dir() -> PathBuf {
    if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(path);
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".config");
    }
    panic!(
        "could not determine config dir; set {PLOKE_DB_SNAPSHOT_FIXTURE_DIR_ENV}, XDG_CONFIG_HOME, or HOME"
    )
}

fn workspace_root_key() -> String {
    let root = workspace_root();
    let id = Uuid::new_v5(&Uuid::NAMESPACE_URL, root.display().to_string().as_bytes());
    let mut simple = id.simple().to_string();
    simple.truncate(12);
    simple
}

impl FixtureEmbeddingExpectation {
    pub fn embedding_set(self) -> EmbeddingSet {
        let dtype = match self.dtype {
            "f32" | "F32" => EmbeddingDType::F32,
            "f64" | "F64" => EmbeddingDType::F64,
            other => panic!("unsupported fixture embedding dtype: {other}"),
        };
        EmbeddingSet::new(
            EmbeddingProviderSlug::new_from_str(self.provider),
            EmbeddingModelId::new_from_str(self.model),
            EmbeddingShape::new(self.dims, dtype),
        )
    }
}

pub const FIXTURE_NODES_CANONICAL: FixtureDb = FixtureDb {
    id: "fixture_nodes_canonical",
    rel_path: "tests/backup_dbs/fixture_nodes_canonical_2026-05-17.sqlite",
    parsed_targets: &["tests/fixture_crates/fixture_nodes"],
    status: FixtureStatus::Active,
    creation: FixtureCreationStrategy::Automated(FixtureAutomation::FixtureCrateMultiEmbedding {
        fixture_name: "fixture_nodes",
        output_stem: "fixture_nodes_canonical",
    }),
    path_scope: FixturePathScope::CheckoutLocal,
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::PlainBackup,
    requires_primary_index: true,
    bm25_index_expected: false,
    embedding: None,
    last_updated: "2026-05-17",
    notes: "Canonical current-schema fixture_nodes backup. It is imported as a plain active fixture without claiming typed graph fixture coverage. Regeneration intentionally uses setup_db_full_multi_embedding so the saved snapshot includes the current multi-embedding schema relations expected by downstream tests without seeding local vectors.",
};

pub const FIXTURE_NODES_LOCAL_EMBEDDINGS: FixtureDb = FixtureDb {
    id: "fixture_nodes_local_embeddings",
    rel_path: "tests/backup_dbs/fixture_nodes_local_embeddings_2026-05-17.sqlite",
    parsed_targets: &["tests/fixture_crates/fixture_nodes"],
    status: FixtureStatus::Active,
    creation: FixtureCreationStrategy::Automated(FixtureAutomation::FixtureCrateLocalEmbeddings {
        fixture_name: "fixture_nodes",
        output_stem: "fixture_nodes_local_embeddings",
    }),
    path_scope: FixturePathScope::CheckoutLocal,
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::BackupWithEmbeddings,
    requires_primary_index: true,
    bm25_index_expected: false,
    embedding: Some(FixtureEmbeddingExpectation {
        provider: "local",
        model: "sentence-transformers/all-MiniLM-L6-v2",
        dims: 384,
        dtype: "f32",
        vectors_present: true,
        active_set_expected: true,
    }),
    last_updated: "2026-05-17",
    notes: "Local-embedding fixture_nodes backup used by ploke-rag and the headless TUI harness. Regeneration seeds the multi-embedding schema from repo fixture code, forces CPU local indexing, and rejects outputs that leave nodes unembedded before backing up the DB.",
};

pub const FIXTURE_NODES_MULTI_EMBEDDING_SCHEMA_V1: FixtureDb = FixtureDb {
    id: "fixture_nodes_multi_embedding_schema_v1_legacy",
    rel_path: "tests/backup_dbs/fixture_nodes_multi_embedding_schema_v1_bfc25988-15c1-5e58-9aa8-3d33b5e58b92",
    parsed_targets: &["tests/fixture_crates/fixture_nodes"],
    status: FixtureStatus::Legacy,
    creation: FixtureCreationStrategy::Manual(FixtureManualRecreation {
        output_stem: "fixture_nodes_multi_embedding_schema_v1_legacy",
        summary: "This is a legacy schema snapshot with no active in-repo consumers.",
        steps: &[
            "Do not recreate this fixture unless you have first confirmed that a current test or workflow still depends on it.",
            "If it is still needed, capture the exact schema/version requirements in docs before regenerating it.",
        ],
    }),
    path_scope: FixturePathScope::LegacyAbsolute,
    default_access: FixtureAccess::FreshMutable,
    import_mode: FixtureImportMode::PlainBackup,
    requires_primary_index: false,
    bm25_index_expected: false,
    embedding: None,
    last_updated: "2026-03-20",
    notes: "Legacy schema snapshot with no active in-repo callers; retain only until usage is confirmed or it is removed.",
};

pub const PLOKE_DB_PRIMARY: FixtureDb = FixtureDb {
    id: "ploke_db_primary",
    rel_path: "tests/backup_dbs/ploke_db_primary_2026-05-06.sqlite",
    parsed_targets: &["crates/ploke-db"],
    status: FixtureStatus::Active,
    creation: FixtureCreationStrategy::Automated(FixtureAutomation::WorkspaceCrate {
        crate_name: "ploke-db",
        output_stem: "ploke_db_primary",
    }),
    path_scope: FixturePathScope::CheckoutLocal,
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::PlainBackup,
    requires_primary_index: true,
    bm25_index_expected: false,
    embedding: None,
    last_updated: "2026-05-06",
    notes: "Current-schema `crates/ploke-db` graph backup recreated from source via setup_db_full_crate(\"ploke-db\") and used by get_code_edges regression tests.",
};

pub const WS_FIXTURE_01_CANONICAL: FixtureDb = FixtureDb {
    id: "ws_fixture_01_canonical",
    rel_path: "tests/backup_dbs/ws_fixture_01_canonical_2026-05-17.sqlite",
    parsed_targets: &["tests/fixture_workspace/ws_fixture_01"],
    status: FixtureStatus::Active,
    creation: FixtureCreationStrategy::Automated(FixtureAutomation::WorkspaceFixture {
        fixture_name: "ws_fixture_01",
        output_stem: "ws_fixture_01_canonical",
    }),
    path_scope: FixturePathScope::CheckoutLocal,
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::PlainBackup,
    requires_primary_index: true,
    bm25_index_expected: false,
    embedding: None,
    last_updated: "2026-05-17",
    notes: "Canonical plain backup for the committed multi-member workspace fixture `tests/fixture_workspace/ws_fixture_01`. Regeneration parses the on-disk workspace fixture, transforms `workspace_metadata` plus crate graphs into a fresh DB, and writes a strict plain-backup snapshot without assuming any embedding model contract.",
};

/// Single-member workspace fixture for testing focused-crate scenarios.
/// This contains only `member_root` from ws_fixture_01, simulating a workspace
/// where only one crate has been indexed/loaded.
pub const WS_FIXTURE_01_MEMBER_SINGLE: FixtureDb = FixtureDb {
    id: "ws_fixture_01_member_single",
    rel_path: "tests/backup_dbs/ws_fixture_01_member_single_2026-05-06.sqlite",
    parsed_targets: &["tests/fixture_workspace/ws_fixture_01/member_root"],
    status: FixtureStatus::Active,
    creation: FixtureCreationStrategy::Automated(FixtureAutomation::FixtureWorkspaceMember {
        fixture_name: "ws_fixture_01",
        member_crate: "member_root",
        output_stem: "ws_fixture_01_member_single",
    }),
    path_scope: FixturePathScope::CheckoutLocal,
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::PlainBackup,
    requires_primary_index: true,
    bm25_index_expected: false,
    embedding: None,
    last_updated: "2026-05-06",
    notes: "Single-member slice of ws_fixture_01 containing only `member_root`. Used for testing scenarios where a workspace is loaded but only one member crate is indexed/focused. Regeneration extracts only the member crate's graph plus workspace metadata (without other members).",
};

pub const CORPUS_SEMVER_TYPE_GRAPH: FixtureDb = FixtureDb {
    id: "corpus_semver_type_graph",
    rel_path: "tests/backup_dbs/corpus_semver_type_graph_2026-05-17.sqlite",
    parsed_targets: &["github:dtolnay/semver@8591f2344b52b31d85b538de58b76a676fe9ff90"],
    status: FixtureStatus::TypedTypeGraph,
    creation: FixtureCreationStrategy::Automated(FixtureAutomation::GithubCorpusCrate {
        normalized_repo: "dtolnay/semver",
        checkout_slug: "dtolnay__semver",
        clone_url: "https://github.com/dtolnay/semver.git",
        rev: "8591f2344b52b31d85b538de58b76a676fe9ff90",
        output_stem: "corpus_semver_type_graph",
    }),
    path_scope: FixturePathScope::SharedSnapshot,
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::PlainBackup,
    requires_primary_index: false,
    bm25_index_expected: false,
    embedding: None,
    last_updated: "2026-05-17",
    notes: "Corpus-backed type graph contract fixture for graphRAG traversal tests over semver's VersionReq/Version/Comparator type surface. Recreate with `cargo run -p xtask -- recreate-backup-db --fixture corpus_semver_type_graph`.",
};

pub const CORPUS_SEMVER_OPENROUTER_EMBEDDINGS: FixtureDb = FixtureDb {
    id: "corpus_semver_openrouter_embeddings",
    rel_path: "tests/backup_dbs/corpus_semver_openrouter_embeddings_2026-05-17.sqlite",
    parsed_targets: &["github:dtolnay/semver@8591f2344b52b31d85b538de58b76a676fe9ff90"],
    status: FixtureStatus::TypedTypeGraph,
    creation: FixtureCreationStrategy::Automated(
        FixtureAutomation::GithubCorpusCrateOpenRouterEmbeddings {
            normalized_repo: "dtolnay/semver",
            checkout_slug: "dtolnay__semver",
            clone_url: "https://github.com/dtolnay/semver.git",
            rev: "8591f2344b52b31d85b538de58b76a676fe9ff90",
            output_stem: "corpus_semver_openrouter_embeddings",
        },
    ),
    path_scope: FixturePathScope::SharedSnapshot,
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::BackupWithEmbeddings,
    requires_primary_index: false,
    bm25_index_expected: false,
    embedding: Some(FixtureEmbeddingExpectation {
        provider: "openrouter",
        model: "mistralai/codestral-embed-2505",
        dims: 1536,
        dtype: "f32",
        vectors_present: true,
        active_set_expected: true,
    }),
    last_updated: "2026-05-17",
    notes: "Source-pinned semver corpus backup with OpenRouter vectors for RAG/TUI matrix materialization. It is generated from the same pinned checkout as corpus_semver_type_graph and keeps dense-vector fixture requirements out of plain DB traversal tests.",
};

pub const CORPUS_MEMCHR_TYPE_GRAPH: FixtureDb = FixtureDb {
    id: "corpus_memchr_type_graph",
    rel_path: "tests/backup_dbs/corpus_memchr_type_graph_2026-05-17.sqlite",
    parsed_targets: &["github:BurntSushi/memchr@24f5daa5257e00e87007c936761600e034827905"],
    status: FixtureStatus::TypedTypeGraph,
    creation: FixtureCreationStrategy::Automated(FixtureAutomation::GithubCorpusCrate {
        normalized_repo: "BurntSushi/memchr",
        checkout_slug: "BurntSushi__memchr",
        clone_url: "https://github.com/BurntSushi/memchr.git",
        rev: "24f5daa5257e00e87007c936761600e034827905",
        output_stem: "corpus_memchr_type_graph",
    }),
    path_scope: FixturePathScope::SharedSnapshot,
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::PlainBackup,
    requires_primary_index: false,
    bm25_index_expected: false,
    embedding: None,
    last_updated: "2026-05-17",
    notes: "Corpus-backed type graph contract fixture for iterator-return and trait-impl graphRAG traversal over memchr.",
};

pub const CORPUS_MEMCHR_OPENROUTER_EMBEDDINGS: FixtureDb = FixtureDb {
    id: "corpus_memchr_openrouter_embeddings",
    rel_path: "tests/backup_dbs/corpus_memchr_openrouter_embeddings_2026-05-17.sqlite",
    parsed_targets: &["github:BurntSushi/memchr@24f5daa5257e00e87007c936761600e034827905"],
    status: FixtureStatus::TypedTypeGraph,
    creation: FixtureCreationStrategy::Automated(
        FixtureAutomation::GithubCorpusCrateOpenRouterEmbeddings {
            normalized_repo: "BurntSushi/memchr",
            checkout_slug: "BurntSushi__memchr",
            clone_url: "https://github.com/BurntSushi/memchr.git",
            rev: "24f5daa5257e00e87007c936761600e034827905",
            output_stem: "corpus_memchr_openrouter_embeddings",
        },
    ),
    path_scope: FixturePathScope::SharedSnapshot,
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::BackupWithEmbeddings,
    requires_primary_index: false,
    bm25_index_expected: false,
    embedding: Some(FixtureEmbeddingExpectation {
        provider: "openrouter",
        model: "mistralai/codestral-embed-2505",
        dims: 1536,
        dtype: "f32",
        vectors_present: true,
        active_set_expected: true,
    }),
    last_updated: "2026-05-17",
    notes: "Source-pinned memchr corpus backup with OpenRouter vectors for RAG/TUI matrix materialization, including direct request_code_context coverage for memchr iterator return types.",
};

pub const CORPUS_GENERIC_ARRAY_TYPE_GRAPH: FixtureDb = FixtureDb {
    id: "corpus_generic_array_type_graph",
    rel_path: "tests/backup_dbs/corpus_generic_array_type_graph_2026-05-17.sqlite",
    parsed_targets: &["github:fizyk20/generic-array@80bab87431c2e29823dc551a3311324812838a23"],
    status: FixtureStatus::TypedTypeGraph,
    creation: FixtureCreationStrategy::Automated(FixtureAutomation::GithubCorpusCrate {
        normalized_repo: "fizyk20/generic-array",
        checkout_slug: "fizyk20__generic-array",
        clone_url: "https://github.com/fizyk20/generic-array.git",
        rev: "80bab87431c2e29823dc551a3311324812838a23",
        output_stem: "corpus_generic_array_type_graph",
    }),
    path_scope: FixturePathScope::SharedSnapshot,
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::PlainBackup,
    requires_primary_index: false,
    bm25_index_expected: false,
    embedding: None,
    last_updated: "2026-05-17",
    notes: "Corpus-backed type graph contract fixture for const-generic alias and GenericArray traversal.",
};

pub const CORPUS_GENERIC_ARRAY_OPENROUTER_EMBEDDINGS: FixtureDb = FixtureDb {
    id: "corpus_generic_array_openrouter_embeddings",
    rel_path: "tests/backup_dbs/corpus_generic_array_openrouter_embeddings_2026-05-17.sqlite",
    parsed_targets: &["github:fizyk20/generic-array@80bab87431c2e29823dc551a3311324812838a23"],
    status: FixtureStatus::TypedTypeGraph,
    creation: FixtureCreationStrategy::Automated(
        FixtureAutomation::GithubCorpusCrateOpenRouterEmbeddings {
            normalized_repo: "fizyk20/generic-array",
            checkout_slug: "fizyk20__generic-array",
            clone_url: "https://github.com/fizyk20/generic-array.git",
            rev: "80bab87431c2e29823dc551a3311324812838a23",
            output_stem: "corpus_generic_array_openrouter_embeddings",
        },
    ),
    path_scope: FixturePathScope::SharedSnapshot,
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::BackupWithEmbeddings,
    requires_primary_index: false,
    bm25_index_expected: false,
    embedding: Some(FixtureEmbeddingExpectation {
        provider: "openrouter",
        model: "mistralai/codestral-embed-2505",
        dims: 1536,
        dtype: "f32",
        vectors_present: true,
        active_set_expected: true,
    }),
    last_updated: "2026-05-17",
    notes: "Source-pinned generic-array corpus backup with OpenRouter vectors for RAG matrix materialization over const-generic aliases, trait bounds, trait supers, and where-clause owners.",
};

pub const CORPUS_CHRONO_TYPE_GRAPH: FixtureDb = FixtureDb {
    id: "corpus_chrono_type_graph",
    rel_path: "tests/backup_dbs/corpus_chrono_type_graph_2026-05-17.sqlite",
    parsed_targets: &["github:chronotope/chrono@120686c82c5da90377e815edb82c9a80b6b4f2be"],
    status: FixtureStatus::TypedTypeGraph,
    creation: FixtureCreationStrategy::Automated(FixtureAutomation::GithubCorpusCrate {
        normalized_repo: "chronotope/chrono",
        checkout_slug: "chronotope__chrono",
        clone_url: "https://github.com/chronotope/chrono.git",
        rev: "120686c82c5da90377e815edb82c9a80b6b4f2be",
        output_stem: "corpus_chrono_type_graph",
    }),
    path_scope: FixturePathScope::SharedSnapshot,
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::PlainBackup,
    requires_primary_index: false,
    bm25_index_expected: false,
    embedding: None,
    last_updated: "2026-05-17",
    notes: "Corpus-backed type graph contract fixture for LocalResult/MappedLocalTime and timezone generic traversal.",
};

pub const CORPUS_CHRONO_OPENROUTER_EMBEDDINGS: FixtureDb = FixtureDb {
    id: "corpus_chrono_openrouter_embeddings",
    rel_path: "tests/backup_dbs/corpus_chrono_openrouter_embeddings_2026-05-17.sqlite",
    parsed_targets: &["github:chronotope/chrono@120686c82c5da90377e815edb82c9a80b6b4f2be"],
    status: FixtureStatus::TypedTypeGraph,
    creation: FixtureCreationStrategy::Automated(
        FixtureAutomation::GithubCorpusCrateOpenRouterEmbeddings {
            normalized_repo: "chronotope/chrono",
            checkout_slug: "chronotope__chrono",
            clone_url: "https://github.com/chronotope/chrono.git",
            rev: "120686c82c5da90377e815edb82c9a80b6b4f2be",
            output_stem: "corpus_chrono_openrouter_embeddings",
        },
    ),
    path_scope: FixturePathScope::SharedSnapshot,
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::BackupWithEmbeddings,
    requires_primary_index: false,
    bm25_index_expected: false,
    embedding: Some(FixtureEmbeddingExpectation {
        provider: "openrouter",
        model: "mistralai/codestral-embed-2505",
        dims: 1536,
        dtype: "f32",
        vectors_present: true,
        active_set_expected: true,
    }),
    last_updated: "2026-05-17",
    notes: "Source-pinned chrono corpus backup with OpenRouter vectors for RAG matrix materialization over slice, array, tuple, associated-type-bound, and where-bound type contexts.",
};

pub const CORPUS_AXUM_TYPE_GRAPH: FixtureDb = FixtureDb {
    id: "corpus_axum_type_graph",
    rel_path: "tests/backup_dbs/corpus_axum_type_graph_2026-05-17.sqlite",
    parsed_targets: &["github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1"],
    status: FixtureStatus::TypedTypeGraph,
    creation: FixtureCreationStrategy::Automated(FixtureAutomation::GithubCorpusWorkspaceTargets {
        normalized_repo: "tokio-rs/axum",
        checkout_slug: "tokio-rs__axum",
        clone_url: "https://github.com/tokio-rs/axum.git",
        rev: "a3446d68bc03d61fb8e7513052bad2825d0c0db1",
        target_relative_paths: &["axum", "axum-core", "axum-macros"],
        output_stem: "corpus_axum_type_graph",
    }),
    path_scope: FixturePathScope::SharedSnapshot,
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::PlainBackup,
    requires_primary_index: false,
    bm25_index_expected: false,
    embedding: None,
    last_updated: "2026-05-17",
    notes: "Source-pinned axum workspace backup for TypeNode matrix coverage over trait objects, impl Trait, and nested parenthesized no-target rows.",
};

pub const CORPUS_AXUM_CALL_GRAPH: FixtureDb = FixtureDb {
    id: "corpus_axum_call_graph",
    rel_path: "tests/backup_dbs/corpus_axum_call_graph_2026-06-28.sqlite",
    parsed_targets: &["github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1"],
    status: FixtureStatus::Active,
    creation: FixtureCreationStrategy::Automated(FixtureAutomation::GithubCorpusWorkspaceTargets {
        normalized_repo: "tokio-rs/axum",
        checkout_slug: "tokio-rs__axum",
        clone_url: "https://github.com/tokio-rs/axum.git",
        rev: "a3446d68bc03d61fb8e7513052bad2825d0c0db1",
        target_relative_paths: &["axum", "axum-core", "axum-macros"],
        output_stem: "corpus_axum_call_graph",
    }),
    path_scope: FixturePathScope::SharedSnapshot,
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::PlainBackup,
    requires_primary_index: false,
    bm25_index_expected: false,
    embedding: None,
    last_updated: "2026-06-28",
    notes: "Source-pinned axum workspace backup for real-target call graph query contracts over helper callers, call sites, self-method calls, and documented unsupported proc-macro, closure, and dynamic dispatch shapes.",
};

pub const CORPUS_AXUM_OPENROUTER_EMBEDDINGS: FixtureDb = FixtureDb {
    id: "corpus_axum_openrouter_embeddings",
    rel_path: "tests/backup_dbs/corpus_axum_openrouter_embeddings_2026-05-17.sqlite",
    parsed_targets: &["github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1"],
    status: FixtureStatus::TypedTypeGraph,
    creation: FixtureCreationStrategy::Automated(
        FixtureAutomation::GithubCorpusWorkspaceTargetsOpenRouterEmbeddings {
            normalized_repo: "tokio-rs/axum",
            checkout_slug: "tokio-rs__axum",
            clone_url: "https://github.com/tokio-rs/axum.git",
            rev: "a3446d68bc03d61fb8e7513052bad2825d0c0db1",
            target_relative_paths: &["axum", "axum-core", "axum-macros"],
            output_stem: "corpus_axum_openrouter_embeddings",
        },
    ),
    path_scope: FixturePathScope::SharedSnapshot,
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::BackupWithEmbeddings,
    requires_primary_index: false,
    bm25_index_expected: false,
    embedding: Some(FixtureEmbeddingExpectation {
        provider: "openrouter",
        model: "mistralai/codestral-embed-2505",
        dims: 1536,
        dtype: "f32",
        vectors_present: true,
        active_set_expected: true,
    }),
    last_updated: "2026-05-17",
    notes: "Source-pinned axum workspace backup with OpenRouter vectors for RAG/TUI matrix materialization of trait object and impl Trait cases, plus DB no-target coverage for nested parenthesized types.",
};

pub const PLOKE_DB_ORPHANED: FixtureDb = FixtureDb {
    id: "ploke_db_orphaned",
    rel_path: "tests/backup_dbs/ploke-db_af8e3a20-728d-5967-8523-da8a5ccdae45",
    parsed_targets: &["crates/ploke-db"],
    status: FixtureStatus::Orphaned,
    creation: FixtureCreationStrategy::Manual(FixtureManualRecreation {
        output_stem: "ploke_db_orphaned",
        summary: "No active in-repo consumer is currently registered for this snapshot.",
        steps: &[
            "Do not regenerate this fixture until a concrete consumer has been identified and documented.",
        ],
    }),
    path_scope: FixturePathScope::LegacyAbsolute,
    default_access: FixtureAccess::FreshMutable,
    import_mode: FixtureImportMode::PlainBackup,
    requires_primary_index: false,
    bm25_index_expected: false,
    embedding: None,
    last_updated: "2026-03-20",
    notes: "No active references were found during the 2026-03-20 inventory; keep under review before removing.",
};

pub const BACKUP_DB_FIXTURES: &[&FixtureDb] = &[
    &FIXTURE_NODES_CANONICAL,
    &FIXTURE_NODES_LOCAL_EMBEDDINGS,
    &FIXTURE_NODES_MULTI_EMBEDDING_SCHEMA_V1,
    &PLOKE_DB_PRIMARY,
    &WS_FIXTURE_01_CANONICAL,
    &WS_FIXTURE_01_MEMBER_SINGLE,
    &CORPUS_SEMVER_TYPE_GRAPH,
    &CORPUS_SEMVER_OPENROUTER_EMBEDDINGS,
    &CORPUS_MEMCHR_TYPE_GRAPH,
    &CORPUS_MEMCHR_OPENROUTER_EMBEDDINGS,
    &CORPUS_GENERIC_ARRAY_TYPE_GRAPH,
    &CORPUS_GENERIC_ARRAY_OPENROUTER_EMBEDDINGS,
    &CORPUS_CHRONO_TYPE_GRAPH,
    &CORPUS_CHRONO_OPENROUTER_EMBEDDINGS,
    &CORPUS_AXUM_TYPE_GRAPH,
    &CORPUS_AXUM_CALL_GRAPH,
    &CORPUS_AXUM_OPENROUTER_EMBEDDINGS,
    &PLOKE_DB_ORPHANED,
];

pub fn all_backup_db_fixtures() -> &'static [&'static FixtureDb] {
    BACKUP_DB_FIXTURES
}

pub fn active_backup_db_fixtures() -> impl Iterator<Item = &'static FixtureDb> {
    BACKUP_DB_FIXTURES
        .iter()
        .copied()
        .filter(|fixture| fixture.status == FixtureStatus::Active)
}

pub fn backup_db_fixture(id: &str) -> Option<&'static FixtureDb> {
    BACKUP_DB_FIXTURES
        .iter()
        .copied()
        .find(|fixture| fixture.id == id)
}

pub fn load_backup_fixture_db(fixture: &'static FixtureDb) -> Result<LoadedBackupFixture, Error> {
    let candidate = fixture.path_candidate();
    let fixture_path = match candidate.origin {
        FixturePathOrigin::CheckoutLocal => {
            if !candidate.path.exists() {
                return Err(Error::from(DbError::Cozo(format!(
                    "Backup fixture {} is missing at {}",
                    fixture.id,
                    candidate.path.display()
                ))));
            }
            candidate.path.clone()
        }
        FixturePathOrigin::SharedSnapshot | FixturePathOrigin::Registered => {
            backup_fixture_path_or_seed(fixture)?
        }
    };

    let db = import_backup_fixture_db(fixture, &fixture_path)?;
    validate_backup_fixture_contract(fixture, &db)?;
    let origin = if fixture_path == candidate.registered_path {
        FixturePathOrigin::Registered
    } else {
        candidate.origin
    };

    Ok(LoadedBackupFixture {
        path: CheckedFixturePath {
            path: fixture_path,
            registered_path: candidate.registered_path,
            origin,
        },
        db,
    })
}

pub fn fresh_backup_fixture_db(fixture: &'static FixtureDb) -> Result<Database, Error> {
    Ok(load_backup_fixture_db(fixture)?.into_db())
}

fn import_backup_fixture_db(
    fixture: &'static FixtureDb,
    fixture_path: &Path,
) -> Result<Database, Error> {
    let db = Database::init_with_schema()?;
    match fixture.import_mode {
        FixtureImportMode::PlainBackup => {
            let prior_rels = plain_backup_import_relations(fixture, &db)?;
            db.import_from_backup(&fixture_path, &prior_rels)
                .map_err(DbError::from)?;
        }
        FixtureImportMode::BackupWithEmbeddings => {
            import_backup_with_embeddings_for_fixture(fixture, &db, &fixture_path)?;
        }
    }

    db.ensure_compilation_unit_relations()?;
    Ok(db)
}

pub fn plain_backup_import_relations(
    fixture: &'static FixtureDb,
    db: &Database,
) -> Result<Vec<String>, Error> {
    match fixture.status {
        FixtureStatus::TypedTypeGraph => db
            .prior_rels_for_typed_type_graph_backup_import()
            .map_err(Error::from),
        _ => db.prior_rels_for_plain_backup_import().map_err(Error::from),
    }
}

pub fn import_backup_with_embeddings_for_fixture(
    fixture: &'static FixtureDb,
    db: &Database,
    fixture_path: &std::path::Path,
) -> Result<(), Error> {
    match fixture.status {
        FixtureStatus::TypedTypeGraph => db
            .import_backup_with_embeddings(fixture_path)
            .map_err(Error::from),
        _ => db
            .import_plain_fixture_backup_with_embeddings(fixture_path)
            .map_err(Error::from),
    }
}

pub fn backup_fixture_path_or_seed(fixture: &'static FixtureDb) -> Result<PathBuf, Error> {
    let fixture_path = fixture.path();
    let seed_path = fixture.repo_path();
    let explicit_fixture_dir = env::var_os(PLOKE_DB_SNAPSHOT_FIXTURE_DIR_ENV).is_some();
    let explicit_fixture_dir_uses_default_cache =
        explicit_fixture_dir && is_default_home_snapshot_fixture_path(fixture, &fixture_path);
    if fixture_path.exists() {
        if explicit_fixture_dir && !explicit_fixture_dir_uses_default_cache {
            return Ok(fixture_path);
        }
        return default_fixture_path_or_matching_seed(fixture_path, &seed_path);
    }

    if !explicit_fixture_dir {
        if let Some(path) = home_config_snapshot_fixture_path(fixture) {
            if path.exists() {
                return default_fixture_path_or_matching_seed(path, &seed_path);
            }
        }
    }

    if seed_path.exists() {
        return Ok(seed_path);
    }

    Err(Error::from(DbError::Cozo(format!(
        "Backup fixture {} is missing at {}. Stage shared fixtures with `cargo xtask fixtures ensure --snapshots` or set {}. Committed seed path: {}",
        fixture.id,
        fixture_path.display(),
        PLOKE_DB_SNAPSHOT_FIXTURE_DIR_ENV,
        seed_path.display()
    ))))
}

fn default_fixture_path_or_matching_seed(
    fixture_path: PathBuf,
    seed_path: &Path,
) -> Result<PathBuf, Error> {
    // The shared snapshot dir is global across local worktrees. If it contains
    // a same-named file from a different worktree, prefer this worktree's seed.
    if seed_path.exists() && !fixture_files_match(&fixture_path, seed_path)? {
        return Ok(seed_path.to_path_buf());
    }
    Ok(fixture_path)
}

fn fixture_files_match(left: &Path, right: &Path) -> Result<bool, Error> {
    if left == right {
        return Ok(true);
    }

    let left_metadata = fixture_file_metadata(left)?;
    let right_metadata = fixture_file_metadata(right)?;
    if left_metadata.len() != right_metadata.len() {
        return Ok(false);
    }

    let left_bytes = read_fixture_file(left)?;
    let right_bytes = read_fixture_file(right)?;
    Ok(left_bytes == right_bytes)
}

fn fixture_file_metadata(path: &Path) -> Result<fs::Metadata, Error> {
    fs::metadata(path).map_err(|err| {
        Error::from(DbError::Cozo(format!(
            "Could not inspect backup fixture file {}: {err}",
            path.display()
        )))
    })
}

fn read_fixture_file(path: &Path) -> Result<Vec<u8>, Error> {
    fs::read(path).map_err(|err| {
        Error::from(DbError::Cozo(format!(
            "Could not read backup fixture file {}: {err}",
            path.display()
        )))
    })
}

fn is_default_home_snapshot_fixture_path(fixture: &'static FixtureDb, path: &Path) -> bool {
    home_config_snapshot_fixture_path(fixture).is_some_and(|default_path| default_path == path)
}

fn home_config_snapshot_fixture_path(fixture: &'static FixtureDb) -> Option<PathBuf> {
    let home = env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join(".config")
            .join("ploke")
            .join("db_snapshot_fixtures")
            .join(fixture.filename()),
    )
}

pub fn shared_backup_fixture_db(fixture: &'static FixtureDb) -> Result<Arc<Database>, Error> {
    {
        let cache = SHARED_FIXTURE_DBS
            .lock()
            .expect("shared backup fixture cache mutex should not be poisoned");
        if let Some(db) = cache.get(fixture.id) {
            return Ok(Arc::clone(db));
        }
    }

    let db = Arc::new(fresh_backup_fixture_db(fixture)?);
    let mut cache = SHARED_FIXTURE_DBS
        .lock()
        .expect("shared backup fixture cache mutex should not be poisoned");
    if let Some(existing) = cache.get(fixture.id) {
        return Ok(Arc::clone(existing));
    }
    cache.insert(fixture.id, Arc::clone(&db));
    Ok(db)
}

pub fn validate_backup_fixture_contract(fixture: &FixtureDb, db: &Database) -> Result<(), Error> {
    if let Some(expected_embedding) = fixture.embedding {
        let expected_set = expected_embedding.embedding_set();
        if expected_embedding.active_set_expected {
            db.set_active_set(expected_set.clone())?;
        }
        if expected_embedding.vectors_present {
            let embedding_count = db.count_embeddings_for_set(&expected_set)?;
            if embedding_count == 0 {
                return Err(Error::from(DbError::Cozo(format!(
                    "Fixture {} at {} does not contain embeddings for {}",
                    fixture.id,
                    fixture.path().display(),
                    expected_set.rel_name
                ))));
            }
        }
    }

    if fixture.requires_primary_index {
        create_index_primary(db).map_err(Error::from)?;
    }

    validate_fixture_path_scope(fixture, db)?;
    Ok(())
}

fn validate_fixture_path_scope(fixture: &FixtureDb, db: &Database) -> Result<(), Error> {
    if fixture.path_scope != FixturePathScope::CheckoutLocal || fixture.parsed_targets.is_empty() {
        return Ok(());
    }

    let expected_roots = fixture
        .parsed_targets
        .iter()
        .map(|target| workspace_root().join(target))
        .collect::<Vec<_>>();
    let expected_roots_display = expected_roots
        .iter()
        .map(|root| root.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");

    let crate_rows = db
        .raw_query("?[root_path] := *crate_context { root_path }")
        .map_err(Error::from)?;
    for row in crate_rows.rows {
        let Some(root_path) = row.first().and_then(data_value_str) else {
            continue;
        };
        let root_path = Path::new(root_path);
        if !expected_roots
            .iter()
            .any(|expected| root_path.starts_with(expected))
        {
            return Err(Error::from(DbError::Cozo(format!(
                "Fixture {} is path-bound to another checkout: crate root '{}' does not start with any registered target root [{}]. Regenerate a checkout-local copy with `cargo xtask recreate-backup-db --fixture {}`.",
                fixture.id,
                root_path.display(),
                expected_roots_display,
                fixture.id
            ))));
        }
    }

    let workspace_rows = db
        .raw_query("?[root_path] := *workspace_metadata { root_path }")
        .unwrap_or_else(|_| {
            cozo::NamedRows {
                headers: vec![],
                rows: vec![],
                next: None,
            }
            .into()
        });
    for row in workspace_rows.rows {
        let Some(root_path) = row.first().and_then(data_value_str) else {
            continue;
        };
        let root_path = Path::new(root_path);
        if !expected_roots
            .iter()
            .any(|expected| root_path.starts_with(expected) || expected.starts_with(root_path))
        {
            return Err(Error::from(DbError::Cozo(format!(
                "Fixture {} is path-bound to another checkout: workspace root '{}' is not compatible with registered target root [{}]. Regenerate a checkout-local copy with `cargo xtask recreate-backup-db --fixture {}`.",
                fixture.id,
                root_path.display(),
                expected_roots_display,
                fixture.id
            ))));
        }
    }

    Ok(())
}

fn data_value_str(value: &cozo::DataValue) -> Option<&str> {
    match value {
        cozo::DataValue::Str(value) => Some(value.as_ref()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use cozo::{DataValue, UuidWrapper};
    use ploke_core::WorkspaceId;

    use super::*;

    #[test]
    fn expected_embedding_set_uses_registry_metadata() {
        let set = FIXTURE_NODES_LOCAL_EMBEDDINGS
            .expected_embedding_set()
            .expect("local embedding fixture should define an embedding set");

        assert_eq!(set.provider.as_ref(), "local");
        assert_eq!(set.model.as_ref(), "sentence-transformers/all-MiniLM-L6-v2");
        assert_eq!(set.dims(), 384);
        assert_eq!(set.shape.dtype, EmbeddingDType::F32);
    }

    #[test]
    fn backup_db_fixture_lookup_returns_registered_fixture() {
        let fixture = backup_db_fixture("fixture_nodes_canonical")
            .expect("canonical fixture should be registered");

        assert_eq!(
            fixture.filename(),
            "fixture_nodes_canonical_2026-05-17.sqlite"
        );
        assert_eq!(fixture.status, FixtureStatus::Active);
        assert_eq!(fixture.path_scope, FixturePathScope::CheckoutLocal);
    }

    #[test]
    fn backup_db_fixture_lookup_returns_registered_workspace_fixture() {
        let fixture = backup_db_fixture("ws_fixture_01_canonical")
            .expect("workspace fixture should be registered");

        assert_eq!(
            fixture.filename(),
            "ws_fixture_01_canonical_2026-05-17.sqlite"
        );
        assert_eq!(
            fixture.parsed_targets,
            &["tests/fixture_workspace/ws_fixture_01"]
        );
        assert_eq!(fixture.import_mode, FixtureImportMode::PlainBackup);
        assert_eq!(fixture.status, FixtureStatus::Active);
        assert_eq!(fixture.path_scope, FixturePathScope::CheckoutLocal);
    }

    #[test]
    fn checkout_local_path_is_root_scoped_and_ignored_by_default() {
        let scoped = WS_FIXTURE_01_CANONICAL
            .checkout_local_path()
            .expect("workspace fixture should have checkout-local path");

        assert!(scoped.starts_with(workspace_root().join("tests/backup_dbs/local")));
        assert!(
            scoped
                .file_name()
                .and_then(|name| name.to_str())
                .expect("scoped path should have filename")
                .starts_with("ws_fixture_01_canonical__root-")
        );
    }

    #[test]
    fn default_fixture_path_or_matching_seed_prefers_seed_when_candidate_differs() {
        let temp_dir = unique_fixture_test_dir("fixture-path-differs");
        let candidate_path = temp_dir.join("candidate.sqlite");
        let seed_path = temp_dir.join("seed.sqlite");
        std::fs::write(&candidate_path, b"not this worktree's fixture")
            .expect("write differing fixture candidate");
        std::fs::write(&seed_path, b"this worktree's fixture").expect("write fixture seed");

        let resolved = default_fixture_path_or_matching_seed(candidate_path, &seed_path)
            .expect("resolve fixture candidate");

        assert_eq!(resolved, seed_path);
        std::fs::remove_dir_all(temp_dir).expect("remove fixture temp dir");
    }

    #[test]
    fn default_fixture_path_or_matching_seed_keeps_candidate_when_seed_matches() {
        let temp_dir = unique_fixture_test_dir("fixture-path-matches");
        let candidate_path = temp_dir.join("candidate.sqlite");
        let seed_path = temp_dir.join("seed.sqlite");
        std::fs::write(&seed_path, b"this worktree's fixture").expect("write fixture seed");
        std::fs::copy(&seed_path, &candidate_path).expect("copy matching fixture candidate");

        let resolved = default_fixture_path_or_matching_seed(candidate_path.clone(), &seed_path)
            .expect("resolve fixture candidate");

        assert_eq!(resolved, candidate_path);
        std::fs::remove_dir_all(temp_dir).expect("remove fixture temp dir");
    }

    #[test]
    fn default_home_snapshot_fixture_path_is_detected_without_xdg() {
        let default_path = home_config_snapshot_fixture_path(&FIXTURE_NODES_CANONICAL)
            .expect("HOME should be available in tests");
        let custom_path = std::env::temp_dir().join(FIXTURE_NODES_CANONICAL.filename());

        assert!(is_default_home_snapshot_fixture_path(
            &FIXTURE_NODES_CANONICAL,
            &default_path
        ));
        assert!(!is_default_home_snapshot_fixture_path(
            &FIXTURE_NODES_CANONICAL,
            &custom_path
        ));
    }

    fn unique_fixture_test_dir(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ploke-test-utils-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create fixture temp dir");
        path
    }

    #[test]
    fn corpus_type_graph_fixtures_are_registered_as_typed_graph_source_pinned_targets() {
        let fixture = backup_db_fixture("corpus_semver_type_graph")
            .expect("semver corpus fixture should be registered");

        assert_eq!(fixture.status, FixtureStatus::TypedTypeGraph);
        assert_eq!(fixture.import_mode, FixtureImportMode::PlainBackup);
        assert_eq!(
            fixture.parsed_targets,
            &["github:dtolnay/semver@8591f2344b52b31d85b538de58b76a676fe9ff90"]
        );
        assert!(matches!(
            fixture.creation,
            FixtureCreationStrategy::Automated(FixtureAutomation::GithubCorpusCrate {
                normalized_repo: "dtolnay/semver",
                checkout_slug: "dtolnay__semver",
                ..
            })
        ));
    }

    #[test]
    fn searchable_corpus_fixture_uses_openrouter_embedding_contract() {
        let fixture = backup_db_fixture("corpus_memchr_openrouter_embeddings")
            .expect("memchr searchable corpus fixture should be registered");

        assert_eq!(fixture.status, FixtureStatus::TypedTypeGraph);
        assert_eq!(fixture.import_mode, FixtureImportMode::BackupWithEmbeddings);
        assert_eq!(
            fixture.parsed_targets,
            &["github:BurntSushi/memchr@24f5daa5257e00e87007c936761600e034827905"]
        );
        let embedding = fixture
            .embedding
            .expect("searchable corpus fixture should declare embedding expectations");
        assert_eq!(embedding.provider, "openrouter");
        assert_eq!(embedding.model, "mistralai/codestral-embed-2505");
        assert_eq!(embedding.dims, 1536);
        assert!(embedding.vectors_present);
        assert!(matches!(
            fixture.creation,
            FixtureCreationStrategy::Automated(
                FixtureAutomation::GithubCorpusCrateOpenRouterEmbeddings {
                    normalized_repo: "BurntSushi/memchr",
                    checkout_slug: "BurntSushi__memchr",
                    ..
                }
            )
        ));
    }

    #[test]
    fn workspace_backup_fixture_loads_via_registry_and_has_workspace_metadata() {
        let db = fresh_backup_fixture_db(&WS_FIXTURE_01_CANONICAL)
            .expect("workspace fixture should load through the registry-backed helper");

        let workspace_rows = db
            .raw_query(
                "?[id, root_path, members] := *workspace_metadata { id, root_path, members }",
            )
            .expect("workspace_metadata query should succeed");
        assert_eq!(workspace_rows.rows.len(), 1);

        let crate_rows = db
            .raw_query("?[name, root_path] := *crate_context { name, root_path }")
            .expect("crate_context query should succeed");
        assert_eq!(crate_rows.rows.len(), 2);
    }

    #[test]
    fn workspace_backup_fixture_roundtrips_coherent_membership_and_identity() {
        let db = fresh_backup_fixture_db(&WS_FIXTURE_01_CANONICAL)
            .expect("workspace fixture should load through the registry-backed helper");
        let fixture_workspace_root = workspace_root().join("tests/fixture_workspace/ws_fixture_01");
        let expected_workspace_id = WorkspaceId::from_root_path(&fixture_workspace_root).uuid();

        let workspace_rows = db
            .raw_query(
                "?[id, namespace, root_path, members] := \
                 *workspace_metadata { id, namespace, root_path, members }",
            )
            .expect("workspace_metadata query should succeed");
        assert_eq!(workspace_rows.rows.len(), 1);

        let workspace_row = &workspace_rows.rows[0];
        assert_eq!(
            workspace_row[0],
            DataValue::Uuid(UuidWrapper(expected_workspace_id))
        );
        assert_eq!(
            workspace_row[1],
            DataValue::Uuid(UuidWrapper(expected_workspace_id))
        );
        assert_eq!(
            workspace_row[2],
            DataValue::from(fixture_workspace_root.display().to_string())
        );

        let workspace_members = match &workspace_row[3] {
            DataValue::List(values) => values
                .iter()
                .map(|value| match value {
                    DataValue::Str(path) => path.to_string(),
                    other => panic!("expected workspace member path string, found {other:?}"),
                })
                .collect::<BTreeSet<_>>(),
            other => panic!("expected workspace members list, found {other:?}"),
        };

        let crate_rows = db
            .raw_query("?[root_path] := *crate_context { root_path }")
            .expect("crate_context query should succeed");
        assert_eq!(crate_rows.rows.len(), 2);
        let crate_member_paths = crate_rows
            .rows
            .iter()
            .map(|row| match &row[0] {
                DataValue::Str(path) => path.to_string(),
                other => panic!("expected crate_context.root_path string, found {other:?}"),
            })
            .collect::<BTreeSet<_>>();

        assert_eq!(workspace_members, crate_member_paths);
    }
}
