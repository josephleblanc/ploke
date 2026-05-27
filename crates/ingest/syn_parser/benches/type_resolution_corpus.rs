use std::{
    env, fs,
    panic::{self, AssertUnwindSafe},
    path::{Path, PathBuf},
};

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use syn_parser::{
    GraphAccess, ModuleTree, ParsedCodeGraph, error::SynParserError, parser::nodes::TypeDefNode,
    try_run_phases_and_merge,
};

#[cfg(not(feature = "typed_type_graph"))]
const RESOLVER_NAME: &str = "legacy_type_use_resolution";
#[cfg(feature = "typed_type_graph")]
const RESOLVER_NAME: &str = "typed_type_graph_v2";

fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixture_github_clones/corpus")
        .canonicalize()
        .expect("canonicalize fixture_github_clones/corpus path")
}

fn corpus_targets() -> Vec<(String, PathBuf)> {
    let root = corpus_root();
    let filter = env::var("PLOKE_CORPUS_FILTER").ok();
    let limit = env::var("PLOKE_CORPUS_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|limit| *limit > 0);

    let mut targets: Vec<_> = fs::read_dir(&root)
        .unwrap_or_else(|err| panic!("read corpus root {}: {err}", root.display()))
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }

            let name = entry.file_name().to_string_lossy().into_owned();
            if filter.as_ref().is_some_and(|needle| !name.contains(needle)) {
                return None;
            }

            Some((name, path))
        })
        .collect();

    targets.sort_by(|left, right| left.0.cmp(&right.0));
    if let Some(limit) = limit {
        targets.truncate(limit);
    }

    targets
}

fn parsed_target(
    target: &str,
    root: &Path,
) -> Result<(ParsedCodeGraph, ModuleTree), SynParserError> {
    let mut output = try_run_phases_and_merge(root)?;
    let graph = output.extract_merged_graph().ok_or_else(|| {
        SynParserError::InternalState(format!("merged graph missing for {target}"))
    })?;
    let tree = output.extract_module_tree().ok_or_else(|| {
        SynParserError::InternalState(format!("module tree missing for {target}"))
    })?;

    Ok((graph, tree))
}

#[cfg(not(feature = "typed_type_graph"))]
fn resolved_count_result(
    graph: &ParsedCodeGraph,
    tree: &ModuleTree,
) -> Result<usize, SynParserError> {
    syn_parser::resolve::type_resolution::resolve_type_uses_after_tree(
        black_box(graph),
        black_box(tree),
    )
    .map(|report| black_box(report.resolutions.len()))
}

#[cfg(feature = "typed_type_graph")]
fn resolved_count_result(
    graph: &ParsedCodeGraph,
    tree: &ModuleTree,
) -> Result<usize, SynParserError> {
    syn_parser::resolve::type_resolution_v2::resolve_type_relations_after_tree(
        black_box(graph),
        black_box(tree),
    )
    .map(|report| black_box(report.relations.len()))
}

fn panic_message(panic_value: Box<dyn std::any::Any + Send>) -> String {
    panic_value
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic_value.downcast_ref::<&str>().copied())
        .unwrap_or("<non-string panic>")
        .to_string()
}

fn print_graph_counts(target: &str, graph: &ParsedCodeGraph) {
    let type_aliases = graph
        .defined_types()
        .iter()
        .filter(|defined| matches!(defined, TypeDefNode::TypeAlias(_)))
        .count();

    eprintln!(
        "type_resolution_corpus graph: {target}: functions={} defined_types={} type_aliases={} impls={} traits={} consts={} statics={} type_graph={} modules={} relations={}",
        graph.functions().len(),
        graph.defined_types().len(),
        type_aliases,
        graph.impls().len(),
        graph.traits().len(),
        graph.consts().len(),
        graph.statics().len(),
        graph.type_graph().len(),
        graph.modules().len(),
        graph.relations().len()
    );
}

fn bench_type_resolution_corpus(c: &mut Criterion) {
    let targets = corpus_targets();
    let verbose = env::var_os("PLOKE_CORPUS_VERBOSE").is_some();
    let mut setup_failures = 0usize;
    let mut resolve_failures = 0usize;
    let mut benchable = 0usize;

    eprintln!(
        "type_resolution_corpus: resolver={RESOLVER_NAME}; selected_targets={}",
        targets.len()
    );

    let mut group = c.benchmark_group("type_resolution_corpus");

    for (target, path) in targets {
        let setup = panic::catch_unwind(AssertUnwindSafe(|| parsed_target(&target, &path)));
        let (graph, tree) = match setup {
            Ok(Ok(parsed)) => parsed,
            Ok(Err(err)) => {
                setup_failures += 1;
                eprintln!("type_resolution_corpus setup error: {target}: {err}");
                continue;
            }
            Err(panic_value) => {
                setup_failures += 1;
                eprintln!(
                    "type_resolution_corpus setup panic: {target}: {}",
                    panic_message(panic_value)
                );
                continue;
            }
        };

        if verbose {
            print_graph_counts(&target, &graph);
        }

        let preflight =
            panic::catch_unwind(AssertUnwindSafe(|| resolved_count_result(&graph, &tree)));
        match preflight {
            Ok(Ok(count)) => {
                benchable += 1;
                eprintln!("type_resolution_corpus benching: {target}: resolved={count}");
            }
            Ok(Err(err)) => {
                resolve_failures += 1;
                eprintln!("type_resolution_corpus resolve error: {target}: {err}");
                continue;
            }
            Err(panic_value) => {
                resolve_failures += 1;
                eprintln!(
                    "type_resolution_corpus resolve panic: {target}: {}",
                    panic_message(panic_value)
                );
                continue;
            }
        }

        group.bench_function(BenchmarkId::new(RESOLVER_NAME, target), |b| {
            b.iter(|| resolved_count_result(&graph, &tree).expect("corpus target should preflight"))
        });
    }

    group.finish();

    eprintln!(
        "type_resolution_corpus summary: benchable={benchable}; setup_failures={setup_failures}; resolve_failures={resolve_failures}"
    );
}

criterion_group!(benches, bench_type_resolution_corpus);
criterion_main!(benches);
