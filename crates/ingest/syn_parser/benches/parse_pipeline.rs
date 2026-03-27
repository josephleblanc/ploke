use std::path::PathBuf;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use syn_parser::{
    ParsedCodeGraph, discovery::run_discovery_phase, parser::analyze_files_parallel,
    try_run_phases_and_merge, try_run_phases_and_resolve,
};

fn fixture_crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixture_crates/fixture_nodes")
        .canonicalize()
        .expect("canonicalize fixture_nodes path")
}

fn bench_try_run_phases_and_merge(c: &mut Criterion) {
    let root = fixture_crate_root();
    c.bench_function("try_run_phases_and_merge", |b| {
        b.iter(|| {
            try_run_phases_and_merge(black_box(&root)).expect("parse merge");
        })
    });
}

fn bench_analyze_files_parallel(c: &mut Criterion) {
    let root = fixture_crate_root();
    let discovery = run_discovery_phase(None, &[root]).expect("discovery fixture_nodes");
    c.bench_function("analyze_files_parallel", |b| {
        b.iter(|| {
            analyze_files_parallel(black_box(&discovery), 0);
        })
    });
}

fn bench_merge_new(c: &mut Criterion) {
    let root = fixture_crate_root();
    let graphs = try_run_phases_and_resolve(&root).expect("resolve");
    c.bench_function("merge_new", |b| {
        b.iter(|| {
            let v: Vec<ParsedCodeGraph> = graphs.iter().cloned().collect();
            black_box(ParsedCodeGraph::merge_new(v).expect("merge"));
        })
    });
}

fn bench_build_tree_and_prune(c: &mut Criterion) {
    let root = fixture_crate_root();
    let mut out = try_run_phases_and_merge(&root).expect("merge out");
    let merged = out
        .extract_merged_graph()
        .expect("merged graph for build_tree bench");
    c.bench_function("build_tree_and_prune", |b| {
        b.iter(|| {
            let mut g = merged.clone();
            black_box(g.build_tree_and_prune().expect("tree"));
        })
    });
}

criterion_group!(
    benches,
    bench_try_run_phases_and_merge,
    bench_analyze_files_parallel,
    bench_merge_new,
    bench_build_tree_and_prune
);
criterion_main!(benches);
