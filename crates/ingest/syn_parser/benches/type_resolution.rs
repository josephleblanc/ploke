use std::path::PathBuf;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use syn_parser::{ModuleTree, ParsedCodeGraph, try_run_phases_and_merge};

const RESOLVER_NAME: &str = "typed_type_graph_v2";

const FIXTURES: &[&str] = &[
    "fixture_nodes",
    "fixture_types",
    "fixture_conflation",
    "fixture_type_resolution_v2",
];

fn fixture_crate_root(fixture: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixture_crates")
        .join(fixture)
        .canonicalize()
        .unwrap_or_else(|err| panic!("canonicalize fixture path for {fixture}: {err}"))
}

fn parsed_fixture(fixture: &str) -> (ParsedCodeGraph, ModuleTree) {
    let root = fixture_crate_root(fixture);
    let mut output = try_run_phases_and_merge(&root)
        .unwrap_or_else(|err| panic!("parse and build module tree for {fixture}: {err}"));
    let graph = output
        .extract_merged_graph()
        .unwrap_or_else(|| panic!("merged graph missing for {fixture}"));
    let tree = output
        .extract_module_tree()
        .unwrap_or_else(|| panic!("module tree missing for {fixture}"));

    (graph, tree)
}

fn resolved_count(graph: &ParsedCodeGraph, tree: &ModuleTree) -> usize {
    let report = syn_parser::resolve::type_resolution_v2::resolve_type_relations_after_tree(
        black_box(graph),
        black_box(tree),
    )
    .expect("v2 type-relation resolution");
    black_box(report.relations.len())
}

fn bench_type_resolution(c: &mut Criterion) {
    let fixtures: Vec<_> = FIXTURES
        .iter()
        .map(|fixture| (*fixture, parsed_fixture(fixture)))
        .collect();
    let mut group = c.benchmark_group("type_resolution");

    for (fixture, (graph, tree)) in &fixtures {
        group.bench_function(format!("{RESOLVER_NAME}/{fixture}"), |b| {
            b.iter(|| resolved_count(graph, tree))
        });
    }

    group.finish();
}

criterion_group!(benches, bench_type_resolution);
criterion_main!(benches);
