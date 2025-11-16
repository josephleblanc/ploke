use criterion::{Criterion, black_box, criterion_group, criterion_main};
use ploke_db::{Database, helpers::resolve_nodes_by_canon_in_file};
use ploke_test_utils::{
    LEGACY_FIXTURE_BACKUP_REL_PATH, MULTI_EMBED_FIXTURE_BACKUP_REL_PATH, workspace_root,
};
use std::path::{Path, PathBuf};

fn bench_resolve_strict(c: &mut Criterion) {
    // Load backup DB if present; otherwise skip running the bench work
    let db = Database::init_with_schema().expect("init db");
    let mut backup = workspace_root();
    if cfg!(feature = "multi_embedding_experiment") {
        backup.push(MULTI_EMBED_FIXTURE_BACKUP_REL_PATH);
    } else {
        backup.push(LEGACY_FIXTURE_BACKUP_REL_PATH);
    }
    if backup.exists() {
        let prior = db.relations_vec().expect("relations_vec");
        db.import_from_backup(&backup, &prior)
            .expect("import_from_backup");
    } else {
        // No data; avoid benchmarking empty DB
        return;
    }

    // Small sample: canonical function from fixture
    let abs_file = PathBuf::from("tests/fixture_crates/fixture_nodes/src/imports.rs");
    let abs_file = std::env::current_dir().unwrap().join(abs_file);
    let module_path = vec!["crate".to_string(), "imports".to_string()];
    let item_name = "use_imported_items";

    c.bench_function("resolve_strict_imports_fn", |b| {
        b.iter(|| {
            let v = resolve_nodes_by_canon_in_file(
                &db,
                "function",
                Path::new(black_box(&abs_file)),
                black_box(&module_path),
                black_box(item_name),
            )
            .expect("query");
            black_box(v.len());
        })
    });
}

criterion_group!(benches, bench_resolve_strict);
criterion_main!(benches);
