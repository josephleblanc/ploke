use std::path::PathBuf;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ploke_db::Database;
use ploke_transform::transform::{transform_parsed_graph, transform_parsed_workspace};
use syn_parser::{parse_workspace, try_run_phases_and_merge};

fn fixture_crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixture_crates/fixture_nodes")
        .canonicalize()
        .expect("canonicalize fixture_nodes path")
}

fn fixture_workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixture_workspace/ws_fixture_01")
        .canonicalize()
        .expect("canonicalize ws_fixture_01 path")
}

fn bench_transform_parsed_graph(c: &mut Criterion) {
    let root = fixture_crate_root();
    let mut out = try_run_phases_and_merge(&root).expect("parse fixture_nodes");
    let merged = out.extract_merged_graph().expect("merged graph");
    let tree = out.extract_module_tree().expect("module tree");
    c.bench_function("transform_parsed_graph", |b| {
        b.iter(|| {
            let db = Database::init_with_schema().expect("db");
            transform_parsed_graph(&db, merged.clone(), black_box(&tree)).expect("transform");
        })
    });
}

fn bench_transform_parsed_workspace(c: &mut Criterion) {
    let ws = fixture_workspace_root();
    c.bench_function("transform_parsed_workspace", |b| {
        b.iter(|| {
            let db = Database::init_with_schema().expect("db");
            let parsed = parse_workspace(black_box(&ws), None).expect("parse workspace");
            transform_parsed_workspace(&db, parsed).expect("transform workspace");
        })
    });
}

criterion_group!(
    benches,
    bench_transform_parsed_graph,
    bench_transform_parsed_workspace
);
criterion_main!(benches);
