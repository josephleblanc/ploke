use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ploke_db::{helpers::graph_resolve_exact, Database};
use std::path::{Path, PathBuf};

fn bench_resolve_strict(c: &mut Criterion) {
    // Load backup DB if present; otherwise skip running the bench work
    let db = Database::init_with_schema().expect("init db");
    let mut backup = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    backup.pop(); // crates/ploke-db -> crates
    backup.pop(); // crates -> workspace root
    backup.push("tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92");
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
            let v = graph_resolve_exact(
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
