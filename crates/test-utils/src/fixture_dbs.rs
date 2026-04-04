use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use ploke_common::workspace_root;
use ploke_core::embeddings::{
    EmbeddingDType, EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
};
use ploke_db::{Database, DbError, create_index_primary, multi_embedding::db_ext::EmbeddingExt};
use ploke_error::Error;

use once_cell::sync::Lazy;

static SHARED_FIXTURE_DBS: Lazy<Mutex<HashMap<&'static str, Arc<Database>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

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
    Legacy,
    Orphaned,
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
    pub default_access: FixtureAccess,
    pub import_mode: FixtureImportMode,
    pub requires_primary_index: bool,
    pub bm25_index_expected: bool,
    pub embedding: Option<FixtureEmbeddingExpectation>,
    pub last_updated: &'static str,
    pub notes: &'static str,
}

impl FixtureDb {
    pub fn path(&self) -> PathBuf {
        workspace_root().join(self.rel_path)
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
    rel_path: "tests/backup_dbs/fixture_nodes_canonical_2026-04-01.sqlite",
    parsed_targets: &["tests/fixture_crates/fixture_nodes"],
    status: FixtureStatus::Active,
    creation: FixtureCreationStrategy::Automated(FixtureAutomation::FixtureCrateMultiEmbedding {
        fixture_name: "fixture_nodes",
        output_stem: "fixture_nodes_canonical",
    }),
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::PlainBackup,
    requires_primary_index: true,
    bm25_index_expected: false,
    embedding: None,
    last_updated: "2026-04-01",
    notes: "Canonical current-schema fixture_nodes backup. It is imported as a plain backup, but regeneration intentionally uses setup_db_full_multi_embedding so the saved snapshot includes the current multi-embedding schema relations expected by downstream tests without seeding local vectors.",
};

pub const FIXTURE_NODES_LOCAL_EMBEDDINGS: FixtureDb = FixtureDb {
    id: "fixture_nodes_local_embeddings",
    rel_path: "tests/backup_dbs/fixture_nodes_local_embeddings_2026-04-01.sqlite",
    parsed_targets: &["tests/fixture_crates/fixture_nodes"],
    status: FixtureStatus::Active,
    creation: FixtureCreationStrategy::Automated(FixtureAutomation::FixtureCrateLocalEmbeddings {
        fixture_name: "fixture_nodes",
        output_stem: "fixture_nodes_local_embeddings",
    }),
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
    last_updated: "2026-03-20",
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
    rel_path: "tests/backup_dbs/ploke_db_primary_2026-03-22.sqlite",
    parsed_targets: &["crates/ploke-db"],
    status: FixtureStatus::Active,
    creation: FixtureCreationStrategy::Automated(FixtureAutomation::WorkspaceCrate {
        crate_name: "ploke-db",
        output_stem: "ploke_db_primary",
    }),
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::PlainBackup,
    requires_primary_index: true,
    bm25_index_expected: false,
    embedding: None,
    last_updated: "2026-03-22",
    notes: "Current-schema `crates/ploke-db` graph backup recreated from source via setup_db_full_crate(\"ploke-db\") and used by get_code_edges regression tests.",
};

pub const WS_FIXTURE_01_CANONICAL: FixtureDb = FixtureDb {
    id: "ws_fixture_01_canonical",
    rel_path: "tests/backup_dbs/ws_fixture_01_canonical_2026-03-21.sqlite",
    parsed_targets: &["tests/fixture_workspace/ws_fixture_01"],
    status: FixtureStatus::Active,
    creation: FixtureCreationStrategy::Automated(FixtureAutomation::WorkspaceFixture {
        fixture_name: "ws_fixture_01",
        output_stem: "ws_fixture_01_canonical",
    }),
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::PlainBackup,
    requires_primary_index: true,
    bm25_index_expected: false,
    embedding: None,
    last_updated: "2026-03-21",
    notes: "Canonical plain backup for the committed multi-member workspace fixture `tests/fixture_workspace/ws_fixture_01`. Regeneration parses the on-disk workspace fixture, transforms `workspace_metadata` plus crate graphs into a fresh DB, and writes a strict plain-backup snapshot without assuming any embedding model contract.",
};

/// Single-member workspace fixture for testing focused-crate scenarios.
/// This contains only `member_root` from ws_fixture_01, simulating a workspace
/// where only one crate has been indexed/loaded.
pub const WS_FIXTURE_01_MEMBER_SINGLE: FixtureDb = FixtureDb {
    id: "ws_fixture_01_member_single",
    rel_path: "tests/backup_dbs/ws_fixture_01_member_single_2026-04-03.sqlite",
    parsed_targets: &["tests/fixture_workspace/ws_fixture_01/member_root"],
    status: FixtureStatus::Active,
    creation: FixtureCreationStrategy::Automated(FixtureAutomation::FixtureWorkspaceMember {
        fixture_name: "ws_fixture_01",
        member_crate: "member_root",
        output_stem: "ws_fixture_01_member_single",
    }),
    default_access: FixtureAccess::ImmutableShared,
    import_mode: FixtureImportMode::PlainBackup,
    requires_primary_index: true,
    bm25_index_expected: false,
    embedding: None,
    last_updated: "2026-04-03",
    notes: "Single-member slice of ws_fixture_01 containing only `member_root`. Used for testing scenarios where a workspace is loaded but only one member crate is indexed/focused. Regeneration extracts only the member crate's graph plus workspace metadata (without other members).",
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

pub fn fresh_backup_fixture_db(fixture: &'static FixtureDb) -> Result<Database, Error> {
    let fixture_path = fixture.path();
    if !fixture_path.exists() {
        return Err(Error::from(DbError::Cozo(format!(
            "Backup fixture {} is missing at {}",
            fixture.id,
            fixture_path.display()
        ))));
    }

    let db = Database::init_with_schema()?;
    match fixture.import_mode {
        FixtureImportMode::PlainBackup => {
            let prior_rels = db.prior_rels_for_plain_backup_import()?;
            db.import_from_backup(&fixture_path, &prior_rels)
                .map_err(DbError::from)?;
        }
        FixtureImportMode::BackupWithEmbeddings => {
            db.import_backup_with_embeddings(&fixture_path)
                .map_err(Error::from)?;
        }
    }

    db.ensure_compilation_unit_relations()?;

    validate_backup_fixture_contract(fixture, &db)?;
    Ok(db)
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

    Ok(())
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
            "fixture_nodes_canonical_2026-03-20.sqlite"
        );
        assert_eq!(fixture.status, FixtureStatus::Active);
    }

    #[test]
    fn backup_db_fixture_lookup_returns_registered_workspace_fixture() {
        let fixture = backup_db_fixture("ws_fixture_01_canonical")
            .expect("workspace fixture should be registered");

        assert_eq!(
            fixture.filename(),
            "ws_fixture_01_canonical_2026-03-21.sqlite"
        );
        assert_eq!(
            fixture.parsed_targets,
            &["tests/fixture_workspace/ws_fixture_01"]
        );
        assert_eq!(fixture.import_mode, FixtureImportMode::PlainBackup);
        assert_eq!(fixture.status, FixtureStatus::Active);
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
