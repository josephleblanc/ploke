/// Tests for parsing real-world crates from `tests/fixture_github_clones/`.
///
/// These tests exercise `try_run_phases_and_merge` against production-grade
/// Rust code to surface parse failures, merge conflicts, or module-tree
/// construction bugs that smaller fixtures do not expose.
use ploke_common::fixture_github_clones_dir;
use syn_parser::{
    parser::graph::ParsedCodeGraph,
    try_run_phases_and_merge,
};

// ---------------------------------------------------------------------------
// Tracing helper
// ---------------------------------------------------------------------------

/// Initialize `tracing-subscriber` for a single test run.
///
/// Captures the log targets that are instrumented in the pipeline:
/// - `debug_dup`      – per-step pruning counts inside `ParsedCodeGraph::prune`
/// - `mod_tree_build` – module-tree construction and pruning stages
/// - `buggy` / `buggy_c` – crate-context tracking during merge
///
/// The subscriber is silently ignored if another test in the same process
/// already initialized it (`try_init` returns `Err` instead of panicking).
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Default: TRACE on the targets we care about, WARN for everything else.
        EnvFilter::new(
            "debug_dup=trace,mod_tree_build=debug,buggy=trace,buggy_c=debug,warn",
        )
    });
    let _ = fmt::Subscriber::builder()
        .with_env_filter(filter)
        .with_test_writer()
        .try_init();
}

// ---------------------------------------------------------------------------
// Stage-isolation helpers shared by several tests
// ---------------------------------------------------------------------------

/// Run Phase 1 (discovery) and Phase 2 (file parsing) for the serde fixture,
/// returning the raw per-file `ParsedCodeGraph` results without merging.
///
/// Panics if discovery or all-file-parse fail (those are pre-conditions for
/// the later stages we want to test).
fn collect_serde_graphs() -> Vec<syn_parser::ParsedCodeGraph> {
    use syn_parser::{discovery::run_discovery_phase, parser::analyze_files_parallel};

    let serde_path = fixture_github_clones_dir().join("serde").join("serde");
    let discovery = run_discovery_phase(None, &[serde_path])
        .expect("Discovery must succeed for serde github clone");

    let results = analyze_files_parallel(&discovery, 0);

    let (oks, errs): (Vec<_>, Vec<_>) = results.into_iter().partition(Result::is_ok);

    if !errs.is_empty() {
        for e in &errs {
            eprintln!("Phase 2 file error: {:#?}", e);
        }
        panic!(
            "{} file(s) failed to parse during serde phase-2 collection",
            errs.len()
        );
    }

    oks.into_iter().map(Result::unwrap).collect()
}

// ===========================================================================
// Stage-isolation tests
// ===========================================================================

/// Parses the `serde` crate from the github-clones fixture directory using
/// `try_run_phases_and_merge` and asserts it succeeds end-to-end.
///
/// If this test fails it indicates a real parse/merge/tree bug that needs
/// investigation – the Err variant is printed in full to aid debugging.
#[test]
fn parse_serde_github_clone() {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None)
        .try_init();

    let serde_path = fixture_github_clones_dir().join("serde").join("serde");

    let result = try_run_phases_and_merge(&serde_path);

    if let Err(e) = &result {
        eprintln!("Serde parse FAILED:\n{e:#?}");
    }

    assert!(
        result.is_ok(),
        "try_run_phases_and_merge failed on serde github clone:\n{:#?}",
        result.err()
    );
}

/// Smoke-tests that discovery alone succeeds for the serde fixture,
/// narrowing the failure stage if `parse_serde_github_clone` fails.
#[test]
fn discovery_serde_github_clone() {
    use syn_parser::discovery::run_discovery_phase;

    let serde_path = fixture_github_clones_dir().join("serde").join("serde");

    let discovery = run_discovery_phase(None, &[serde_path]);

    assert!(
        discovery.is_ok(),
        "Discovery failed for serde github clone: {:#?}",
        discovery.err()
    );
}

/// Checks that Phase 2 (file parsing) produces no errors for serde,
/// so we can isolate whether the bug is in parse vs. merge vs. tree-build.
#[test]
fn phase2_serde_github_clone() {
    use syn_parser::{discovery::run_discovery_phase, parser::analyze_files_parallel};

    let serde_path = fixture_github_clones_dir().join("serde").join("serde");
    let discovery = run_discovery_phase(None, &[serde_path])
        .expect("Discovery should succeed for serde");

    let results = analyze_files_parallel(&discovery, 0);

    let errors: Vec<_> = results.iter().filter(|r| r.is_err()).collect();

    for err in &errors {
        eprintln!("Phase 2 error: {:#?}", err);
    }

    assert!(
        errors.is_empty(),
        "{} file(s) failed to parse in serde github clone",
        errors.len()
    );
}

/// Checks that Phase 3 (merge) succeeds in isolation for serde.
///
/// Runs discovery + file parsing, then calls `ParsedCodeGraph::merge_new`
/// without proceeding to module-tree construction.  A failure here points to
/// a merge-specific bug rather than a tree-building one.
#[test]
fn merge_serde_github_clone() {
    init_tracing();

    use syn_parser::parser::graph::ParsedCodeGraph;

    let graphs = collect_serde_graphs();
    let graph_count = graphs.len();

    let result = ParsedCodeGraph::merge_new(graphs);

    if let Err(ref e) = result {
        eprintln!("Merge FAILED after collecting {graph_count} file graphs:\n{e:#?}");
    }

    assert!(
        result.is_ok(),
        "ParsedCodeGraph::merge_new failed on serde github clone"
    );
}

/// Checks that `build_module_tree` (tree construction, *without* applying the
/// prune to the graph) succeeds for serde, and prints diagnostic counts for
/// the `PruningResult` that would later be passed to `prune`.
///
/// This test is expected to *pass* even when `parse_serde_github_clone` panics,
/// because the panic lives inside `prune`, not inside `build_module_tree`.
/// Use the emitted counts to understand what the pruner intends to remove
/// before the assertion fires.
#[test]
fn build_module_tree_serde_github_clone() {
    init_tracing();

    use syn_parser::parser::graph::ParsedCodeGraph;

    let graphs = collect_serde_graphs();
    let merged = ParsedCodeGraph::merge_new(graphs).expect("merge_new should succeed for serde");

    let result = merged.build_module_tree();

    match result {
        Err(ref e) => {
            eprintln!("build_module_tree FAILED:\n{e:#?}");
            panic!("build_module_tree failed on serde github clone");
        }
        Ok((_, ref pruning)) => {
            eprintln!(
                "\nbuild_module_tree OK.\nPruningResult summary:\n\
                 - pruned_module_ids : {}\n\
                 - pruned_item_ids   : {} (all, including secondary)\n\
                 - pruned_relations  : {}",
                pruning.pruned_module_ids.len(),
                pruning.pruned_item_ids.len(),
                pruning.pruned_relations.len(),
            );
        }
    }

    assert!(result.is_ok(), "build_module_tree failed on serde github clone");
}

// ===========================================================================
// Diagnostic tests: reproduce and diagnose the prune-count mismatch
// ===========================================================================

/// Diagnoses the mismatch between `total_count_diff` and `pruned_item_ids.len()`
/// that causes the panic in `ParsedCodeGraph::prune` on serde.
///
/// # Background
///
/// `prune` accumulates a `total_count_diff` by summing the number of elements
/// removed from every top-level graph collection (functions, defined_types,
/// consts, statics, macros, use_statements, impls, traits, non-file modules)
/// **plus** a delta for methods embedded inside any removed `ImplNode` or
/// `TraitNode`.
///
/// The `PruningResult` from `build_module_tree`, by contrast, collects IDs
/// through a BFS over the `Contains` relations in the `ModuleTree`.  Methods
/// stored in `ImplNode.methods` / `TraitNode.methods` are *not* tracked by
/// `Contains` relations and therefore their `MethodNodeId`s are absent from
/// `pruned_item_ids`.
///
/// Consequently `total_count_diff` > `pruned_item_ids.len()` whenever a
/// removed impl or trait block contains methods, and the final assertion in
/// `prune` fires.
///
/// # What this test checks
///
/// It reconstructs the same per-category counts that `prune` would compute,
/// isolates the "orphan method" contribution, and asserts that:
///
/// ```text
/// total_simulated_diff == pruned_item_ids_non_secondary + orphan_method_count
/// ```
///
/// where `orphan_method_count` is the number of methods inside removed
/// impls/traits whose IDs are absent from `pruned_item_ids`.
/// If this assertion passes, the discrepancy is entirely explained by
/// orphan methods and there is no other source of mismatch.
#[test]
fn diagnose_prune_counts_serde_github_clone() {
    init_tracing();

    use itertools::Itertools;
    use syn_parser::{
        parser::{
            graph::{GraphNode, ParsedCodeGraph},
            nodes::{AnyNodeId, AsAnyNodeId},
        },
        resolve::PruningResult,
        GraphAccess,
    };

    let graphs = collect_serde_graphs();
    let merged = ParsedCodeGraph::merge_new(graphs).expect("merge_new should succeed for serde");

    let (_, pruning): (_, PruningResult) = merged
        .build_module_tree()
        .expect("build_module_tree should succeed for serde");

    // --- Replicate the secondary-id filter from `prune` ---
    let pruned_item_initial = pruning.pruned_item_ids.len();
    let pruned_item_ids: Vec<AnyNodeId> = pruning
        .pruned_item_ids
        .iter()
        .copied()
        .filter(|id| {
            !matches!(
                id,
                AnyNodeId::Variant(_)
                    | AnyNodeId::Field(_)
                    | AnyNodeId::Param(_)
                    | AnyNodeId::GenericParam(_)
            )
        })
        .collect_vec();
    let removed_secondary = pruned_item_initial - pruned_item_ids.len();

    // --- Per-category counts (mirror the `retain` calls in `prune`) ---
    let funcs_to_remove = merged
        .functions()
        .iter()
        .filter(|n| pruned_item_ids.contains(&n.id.as_any()))
        .count();
    let defined_types_to_remove = merged
        .defined_types()
        .iter()
        .filter(|n| pruned_item_ids.contains(&n.any_id()))
        .count();
    let consts_to_remove = merged
        .consts()
        .iter()
        .filter(|n| pruned_item_ids.contains(&n.id.as_any()))
        .count();
    let statics_to_remove = merged
        .statics()
        .iter()
        .filter(|n| pruned_item_ids.contains(&n.id.as_any()))
        .count();
    let macros_to_remove = merged
        .macros()
        .iter()
        .filter(|n| pruned_item_ids.contains(&n.id.as_any()))
        .count();
    let use_stmts_to_remove = merged
        .use_statements()
        .iter()
        .filter(|n| pruned_item_ids.contains(&n.id.as_any()))
        .count();

    // Count methods before removing impls/traits (matching `prune`'s approach).
    let methods_before: usize = merged
        .impls()
        .iter()
        .flat_map(|imp| imp.methods.iter())
        .chain(merged.traits().iter().flat_map(|tr| tr.methods.iter()))
        .count();

    let impls_to_remove = merged
        .impls()
        .iter()
        .filter(|n| pruned_item_ids.contains(&n.id.as_any()))
        .count();
    let traits_to_remove = merged
        .traits()
        .iter()
        .filter(|n| pruned_item_ids.contains(&n.id.as_any()))
        .count();

    // Methods *after* simulated removal of impls/traits (matching `prune`'s approach).
    let methods_after: usize = merged
        .impls()
        .iter()
        .filter(|n| !pruned_item_ids.contains(&n.id.as_any()))
        .flat_map(|imp| imp.methods.iter())
        .chain(
            merged
                .traits()
                .iter()
                .filter(|n| !pruned_item_ids.contains(&n.id.as_any()))
                .flat_map(|tr| tr.methods.iter()),
        )
        .count();
    let methods_removed = methods_before - methods_after;

    // Count ALL non-root modules in pruned_item_ids regardless of file-based status.
    //
    // `prune()` uses a single `retain(|m| !pruned_item_ids.contains(&m.id.as_any()))`
    // after the file-module pass, which removes any module whose AnyNodeId appears in
    // pruned_item_ids — including file-based modules that leaked into pruned_item_ids via
    // BFS through ResolvesToDefinition relations (e.g. private/de.rs and private/ser.rs
    // in serde, which are children of the unlinked private/mod.rs module).
    let nonfile_mods_to_remove = merged
        .modules()
        .iter()
        .filter(|m| pruned_item_ids.contains(&m.id.as_any()))
        .count();

    // Simulated total_count_diff (matches the `prune` accumulator).
    let total_simulated_diff = funcs_to_remove
        + defined_types_to_remove
        + consts_to_remove
        + statics_to_remove
        + macros_to_remove
        + use_stmts_to_remove
        + impls_to_remove
        + traits_to_remove
        + methods_removed
        + nonfile_mods_to_remove;

    // Methods in removed impls/traits whose IDs are NOT in `pruned_item_ids`.
    // These are the "orphan" methods that cause `total_count_diff > pruned_item_ids.len()`.
    let orphan_method_count: usize = merged
        .impls()
        .iter()
        .filter(|n| pruned_item_ids.contains(&n.id.as_any()))
        .flat_map(|imp| imp.methods.iter())
        .chain(
            merged
                .traits()
                .iter()
                .filter(|n| pruned_item_ids.contains(&n.id.as_any()))
                .flat_map(|tr| tr.methods.iter()),
        )
        .filter(|m| !pruned_item_ids.contains(&m.id.as_any()))
        .count();

    eprintln!(
        "\n=== Prune-count diagnosis for serde ===\n\
         pruned_item_ids (all)                : {pruned_item_initial}\n\
         - secondary (Variant/Field/Param/GP) : {removed_secondary}\n\
         pruned_item_ids (non-secondary)       : {}\n\
         \n\
         Per-category items to remove:\n\
           functions (top-level)               : {funcs_to_remove}\n\
           defined_types                       : {defined_types_to_remove}\n\
           consts                              : {consts_to_remove}\n\
           statics                             : {statics_to_remove}\n\
           macros                              : {macros_to_remove}\n\
           use_statements                      : {use_stmts_to_remove}\n\
           impls                               : {impls_to_remove}\n\
           traits                              : {traits_to_remove}\n\
           methods (in removed impls/traits)   : {methods_removed}\n\
             of which NOT in pruned_item_ids   : {orphan_method_count}  ← orphans\n\
           modules (all kinds in pruned_ids)   : {nonfile_mods_to_remove}\n\
         \n\
         total_simulated_diff (= what `prune` computes as left side)  : {total_simulated_diff}\n\
         pruned_item_ids.len() (= what `prune` uses as right side)    : {}\n\
         difference (total_simulated_diff - pruned_item_ids.len())    : {}\n",
        pruned_item_ids.len(),
        pruned_item_ids.len(),
        total_simulated_diff as isize - pruned_item_ids.len() as isize,
    );

    // --- Extra: check whether any MODULE ids in pruned_item_ids are FILE-BASED ---
    // The category simulation only counts non-file-based modules. If file-based
    // module IDs leaked into pruned_item_ids (e.g., via BFS through
    // ResolvesToDefinition), the simulation undercounts by exactly that amount.
    let file_based_in_pruned: Vec<_> = merged
        .modules()
        .iter()
        .filter(|m| m.is_file_based())
        .filter(|m| pruned_item_ids.contains(&m.id.as_any()))
        .collect();

    eprintln!(
        "file-based modules IN pruned_item_ids : {} {:?}",
        file_based_in_pruned.len(),
        file_based_in_pruned
            .iter()
            .map(|m| format!("{}({})", m.name, m.id))
            .collect::<Vec<_>>(),
    );

    // The core invariant: if the only source of mismatch is orphan methods, then
    // total_simulated_diff == (non-secondary pruned items) + (orphan methods).
    // If this assertion fails, a different or additional source of mismatch exists.
    assert_eq!(
        total_simulated_diff,
        pruned_item_ids.len() + orphan_method_count,
        "total_simulated_diff should equal (non-secondary pruned items) + (orphan methods in \
         removed impls/traits).\n\
         If this fails, there is an additional source of mismatch beyond orphan methods."
    );
}

/// Checks which IDs in `pruned_item_ids` are not found in *any* top-level graph
/// collection, providing a complementary lens on the mismatch.
///
/// "Phantom" IDs are entries in `pruned_item_ids` that the BFS collected but
/// that are not present in any `graph.*` Vec.  A non-zero phantom count would
/// indicate the BFS is traversing into node types (e.g. `MethodNodeId` via
/// `Contains` relations emitted for associated items) that are not stored as
/// top-level graph items.  In that scenario `pruned_item_ids.len()` would
/// overcount relative to what `retain` can actually remove, which is the
/// *opposite* direction from the currently observed panic.
#[test]
fn diagnose_phantom_prune_ids_serde_github_clone() {
    init_tracing();

    use itertools::Itertools;
    use std::collections::HashMap;
    use syn_parser::{
        parser::{
            graph::{GraphNode, ParsedCodeGraph},
            nodes::{AnyNodeId, AsAnyNodeId},
        },
        resolve::PruningResult,
        GraphAccess,
    };

    let graphs = collect_serde_graphs();
    let merged = ParsedCodeGraph::merge_new(graphs).expect("merge_new should succeed for serde");

    let (_, pruning): (_, PruningResult) = merged
        .build_module_tree()
        .expect("build_module_tree should succeed for serde");

    // Replicate the secondary-id filter from `prune`.
    let pruned_item_ids: Vec<AnyNodeId> = pruning
        .pruned_item_ids
        .iter()
        .copied()
        .filter(|id| {
            !matches!(
                id,
                AnyNodeId::Variant(_)
                    | AnyNodeId::Field(_)
                    | AnyNodeId::Param(_)
                    | AnyNodeId::GenericParam(_)
            )
        })
        .collect_vec();

    // Build a set of all *top-level* node IDs present in the graph.
    // Note: methods embedded inside `ImplNode`/`TraitNode` are intentionally
    // excluded because they are not stored in a top-level Vec.
    let all_top_level_ids: std::collections::HashSet<AnyNodeId> = merged
        .functions()
        .iter()
        .map(|n| n.id.as_any())
        .chain(merged.defined_types().iter().map(|n| n.any_id()))
        .chain(merged.consts().iter().map(|n| n.id.as_any()))
        .chain(merged.statics().iter().map(|n| n.id.as_any()))
        .chain(merged.macros().iter().map(|n| n.id.as_any()))
        .chain(merged.use_statements().iter().map(|n| n.id.as_any()))
        .chain(merged.impls().iter().map(|n| n.id.as_any()))
        .chain(merged.traits().iter().map(|n| n.id.as_any()))
        .chain(merged.modules().iter().map(|n| n.id.as_any()))
        .collect();

    let phantom_ids: Vec<AnyNodeId> = pruned_item_ids
        .iter()
        .copied()
        .filter(|id| !all_top_level_ids.contains(id))
        .collect_vec();

    // Group phantom IDs by variant for a compact summary.
    let mut by_variant: HashMap<&'static str, usize> = HashMap::new();
    for id in &phantom_ids {
        let key = match id {
            AnyNodeId::Function(_) => "Function",
            AnyNodeId::Struct(_) => "Struct",
            AnyNodeId::Enum(_) => "Enum",
            AnyNodeId::Union(_) => "Union",
            AnyNodeId::TypeAlias(_) => "TypeAlias",
            AnyNodeId::Trait(_) => "Trait",
            AnyNodeId::Impl(_) => "Impl",
            AnyNodeId::Const(_) => "Const",
            AnyNodeId::Static(_) => "Static",
            AnyNodeId::Macro(_) => "Macro",
            AnyNodeId::Import(_) => "Import",
            AnyNodeId::Module(_) => "Module",
            AnyNodeId::Method(_) => "Method",
            AnyNodeId::Field(_) => "Field",
            AnyNodeId::Variant(_) => "Variant",
            AnyNodeId::Param(_) => "Param",
            AnyNodeId::GenericParam(_) => "GenericParam",
            AnyNodeId::Reexport(_) => "Reexport",
        };
        *by_variant.entry(key).or_default() += 1;
    }

    eprintln!(
        "\n=== Phantom-ID diagnosis for serde ===\n\
         pruned_item_ids (non-secondary) : {}\n\
         IDs not found in any top-level collection : {} (phantoms)\n\
         By variant: {by_variant:#?}\n",
        pruned_item_ids.len(),
        phantom_ids.len(),
    );

    // We expect zero phantom IDs.  If this assertion fires, `pruned_item_ids`
    // contains IDs that were never inserted into any top-level graph collection
    // – a separate bug worth investigating alongside the orphan-method issue.
    assert!(
        phantom_ids.is_empty(),
        "{} phantom IDs found in pruned_item_ids (not present in any top-level collection).\n\
         By variant: {by_variant:#?}",
        phantom_ids.len(),
    );
}

// ===========================================================================
// Deep-node diagnostics: identify the exact nodes responsible for the mismatch
// ===========================================================================

/// Prints detailed information about every method that appears in
/// `pruned_item_ids` but has no corresponding `AnyNodeId::Method` entry in
/// any top-level graph collection.
///
/// The goal is to understand:
/// 1. Which impl/trait block each orphan method belongs to.
/// 2. The name and span of that method.
/// 3. Whether the method also exists as a top-level `FunctionNode` (which
///    would indicate a duplicate-ID or mislabelling bug).
///
/// This test is intentionally non-asserting – it just emits data for
/// diagnosis.  Run it with `-- --nocapture` to see the output.
#[test]
fn inspect_method_ids_in_pruned_item_ids_serde() {
    init_tracing();

    use std::collections::HashMap;
    use syn_parser::{
        parser::{
            graph::{GraphNode, ParsedCodeGraph},
            nodes::{AnyNodeId, AsAnyNodeId},
        },
        resolve::PruningResult,
        GraphAccess,
    };

    let graphs = collect_serde_graphs();
    let merged = ParsedCodeGraph::merge_new(graphs).expect("merge_new should succeed for serde");

    let (_, pruning): (_, PruningResult) = merged
        .build_module_tree()
        .expect("build_module_tree should succeed for serde");

    // Collect all Method IDs from pruned_item_ids.
    let pruned_method_ids: Vec<AnyNodeId> = pruning
        .pruned_item_ids
        .iter()
        .copied()
        .filter(|id| matches!(id, AnyNodeId::Method(_)))
        .collect();

    // Build a map: method ID → (impl/trait name, method name, module path, file)
    // by scanning every impl and trait in the graph.
    let mut method_info: HashMap<AnyNodeId, (String, &str)> = HashMap::new();
    for imp in merged.impls() {
        for m in &imp.methods {
            method_info.insert(m.any_id(), (imp.name().to_string(), &m.name));
        }
    }
    for tr in merged.traits() {
        for m in &tr.methods {
            method_info.insert(m.any_id(), (tr.name.clone(), &m.name));
        }
    }

    // Separate methods whose impl/trait IS in pruned_item_ids (expected) from
    // those whose container is NOT pruned (surprising).
    let pruned_impl_ids: std::collections::HashSet<AnyNodeId> = pruning
        .pruned_item_ids
        .iter()
        .copied()
        .filter(|id| matches!(id, AnyNodeId::Impl(_) | AnyNodeId::Trait(_)))
        .collect();

    let mut expected_methods: Vec<(AnyNodeId, String)> = Vec::new();
    let mut surprising_methods: Vec<(AnyNodeId, String)> = Vec::new();

    for mid in &pruned_method_ids {
        // Find the impl/trait that owns this method.
        let owner_in_pruned = merged
            .impls()
            .iter()
            .find(|imp| imp.methods.iter().any(|m| m.any_id() == *mid))
            .map(|imp| imp.id.as_any())
            .or_else(|| {
                merged
                    .traits()
                    .iter()
                    .find(|tr| tr.methods.iter().any(|m| m.any_id() == *mid))
                    .map(|tr| tr.id.as_any())
            });

        let (container_name, method_name) = method_info
            .get(mid)
            .map(|(c, m)| (c.as_str(), *m))
            .unwrap_or(("<unknown>", "<unknown>"));
        let description = format!(
            "method `{}` in `{}` (owner_id: {:?})",
            method_name,
            container_name,
            owner_in_pruned,
        );

        match owner_in_pruned {
            Some(owner_id) if pruned_impl_ids.contains(&owner_id) => {
                expected_methods.push((*mid, description));
            }
            _ => {
                surprising_methods.push((*mid, description));
            }
        }
    }

    eprintln!(
        "\n=== Method IDs in pruned_item_ids for serde/serde ===\n\
         Total method IDs in pruned_item_ids : {}\n\
         Expected (owner impl/trait is also pruned) : {}\n\
         Surprising (owner NOT pruned / not found) : {}",
        pruned_method_ids.len(),
        expected_methods.len(),
        surprising_methods.len(),
    );

    if !surprising_methods.is_empty() {
        eprintln!("\nSurprising method IDs (first 20):");
        for (id, desc) in surprising_methods.iter().take(20) {
            eprintln!("  {:?}  {}", id, desc);
        }
    }

    // Show a sample of expected methods to confirm they look reasonable.
    eprintln!("\nSample expected method IDs (first 10):");
    for (id, desc) in expected_methods.iter().take(10) {
        eprintln!("  {:?}  {}", id, desc);
    }
}

/// Identifies exactly which IDs in `pruned_item_ids` (non-secondary,
/// non-method) are NOT matched by the `retain` calls inside `prune`, for the
/// `serde/serde` crate.
///
/// These are the 2 IDs responsible for the `total_simulated_diff (603) !=
/// pruned_item_ids.len() (605)` mismatch observed in
/// `diagnose_prune_counts_serde_github_clone`.
#[test]
fn inspect_unmatched_non_method_ids_serde() {
    init_tracing();

    use itertools::Itertools;
    use syn_parser::{
        parser::{
            graph::{GraphNode, ParsedCodeGraph},
            nodes::{AnyNodeId, AsAnyNodeId},
        },
        resolve::PruningResult,
        GraphAccess,
    };

    let graphs = collect_serde_graphs();
    let merged = ParsedCodeGraph::merge_new(graphs).expect("merge_new should succeed for serde");

    let (_, pruning): (_, PruningResult) = merged
        .build_module_tree()
        .expect("build_module_tree should succeed for serde");

    // Non-secondary, non-method IDs in pruned_item_ids.
    let interesting_ids: Vec<AnyNodeId> = pruning
        .pruned_item_ids
        .iter()
        .copied()
        .filter(|id| {
            !matches!(
                id,
                AnyNodeId::Variant(_)
                    | AnyNodeId::Field(_)
                    | AnyNodeId::Param(_)
                    | AnyNodeId::GenericParam(_)
                    | AnyNodeId::Method(_) // handled via method-delta in prune
            )
        })
        .collect_vec();

    // For each interesting ID, check if it is found in any top-level collection.
    let find_node_desc = |id: AnyNodeId| -> String {
        if let Some(n) = merged.functions().iter().find(|n| n.id.as_any() == id) {
            return format!("Function `{}`", n.name);
        }
        if let Some(n) = merged.defined_types().iter().find(|n| n.any_id() == id) {
            return format!("TypeDef `{}`", n.name());
        }
        if let Some(n) = merged.consts().iter().find(|n| n.id.as_any() == id) {
            return format!("Const `{}`", n.name);
        }
        if let Some(n) = merged.statics().iter().find(|n| n.id.as_any() == id) {
            return format!("Static `{}`", n.name);
        }
        if let Some(n) = merged.macros().iter().find(|n| n.id.as_any() == id) {
            return format!("Macro `{}`", n.name);
        }
        if let Some(n) = merged.use_statements().iter().find(|n| n.id.as_any() == id) {
            return format!("UseStatement `{}`", n.visible_name);
        }
        if let Some(n) = merged.impls().iter().find(|n| n.id.as_any() == id) {
            return format!("Impl `{}`", n.name());
        }
        if let Some(n) = merged.traits().iter().find(|n| n.id.as_any() == id) {
            return format!("Trait `{}`", n.name);
        }
        if let Some(n) = merged.modules().iter().find(|n| n.id.as_any() == id) {
            return format!(
                "Module `{}` (file_based: {}, file: {:?})",
                n.name,
                n.is_file_based(),
                n.file_path(),
            );
        }
        format!("<NOT FOUND IN ANY COLLECTION: {:?}>", id)
    };

    // Split into matched vs unmatched.
    let matched: Vec<_> = interesting_ids
        .iter()
        .map(|&id| (id, find_node_desc(id)))
        .filter(|(_, desc)| !desc.starts_with("<NOT FOUND"))
        .collect();
    let unmatched: Vec<_> = interesting_ids
        .iter()
        .map(|&id| (id, find_node_desc(id)))
        .filter(|(_, desc)| desc.starts_with("<NOT FOUND"))
        .collect();

    eprintln!(
        "\n=== Non-secondary, non-method IDs in pruned_item_ids (serde/serde) ===\n\
         Total : {}\n\
         Matched in a top-level collection : {}\n\
         UNMATCHED (no corresponding graph node) : {}",
        interesting_ids.len(),
        matched.len(),
        unmatched.len(),
    );

    if !unmatched.is_empty() {
        eprintln!("\nUnmatched IDs:");
        for (id, desc) in &unmatched {
            eprintln!("  variant={:?}  desc={}", id, desc);
        }
    }
}

// ===========================================================================
// Workspace-level tests: identify which serde workspace member causes the panic
// ===========================================================================

/// Run `try_run_phases_and_merge` on every member of the serde workspace and
/// report which ones fail (and with what error / panic details).
///
/// The xtask command `--target tests/fixture_github_clones/serde` targets the
/// whole workspace.  The `ParsedCodeGraph::prune` panic (`1001 != 988`) must
/// originate in one of the individual workspace members.  This test narrows it
/// down.
///
/// Each member is caught via `std::panic::catch_unwind` so the test can
/// report all failures rather than stopping at the first one.
#[test]
fn diagnose_all_serde_workspace_members() {
    init_tracing();

    let workspace_root = fixture_github_clones_dir().join("serde");
    // Members declared in tests/fixture_github_clones/serde/Cargo.toml.
    let members = ["serde", "serde_core", "serde_derive", "serde_derive_internals", "test_suite"];

    eprintln!("\n=== Serde workspace member parse results ===");

    for member in &members {
        let crate_path = workspace_root.join(member);
        if !crate_path.exists() {
            eprintln!("  {member:30} SKIP (path does not exist: {})", crate_path.display());
            continue;
        }

        // try_run_phases_and_merge internally calls `ParsedCodeGraph::prune`, which
        // currently panics on certain inputs.  We use catch_unwind so this test
        // can report ALL failing members rather than stopping at the first.
        let path_clone = crate_path.clone();
        let outcome = std::panic::catch_unwind(move || {
            try_run_phases_and_merge(&path_clone)
        });

        match outcome {
            Ok(Ok(_)) => {
                eprintln!("  {member:30} OK");
            }
            Ok(Err(e)) => {
                eprintln!("  {member:30} Err  → {e}");
            }
            Err(panic_val) => {
                let msg = panic_val
                    .downcast_ref::<String>()
                    .map(|s| s.as_str())
                    .or_else(|| panic_val.downcast_ref::<&str>().copied())
                    .unwrap_or("<non-string panic>");
                eprintln!("  {member:30} PANIC → {msg}");
            }
        }
    }
    eprintln!();
}

/// For whichever serde workspace member panics, run the detailed prune-count
/// diagnosis and report the per-category breakdown.
///
/// To avoid re-hardcoding member names, this iterates all members and runs
/// the count simulation for the ones that have data (i.e., where
/// `build_module_tree` succeeds even if `build_tree_and_prune` panics).
#[test]
fn diagnose_prune_counts_all_serde_members() {
    init_tracing();

    use itertools::Itertools;
    use syn_parser::{
        discovery::run_discovery_phase,
        parser::{
            analyze_files_parallel,
            graph::{GraphNode, ParsedCodeGraph},
            nodes::{AnyNodeId, AsAnyNodeId},
        },
        resolve::PruningResult,
        GraphAccess,
    };

    let workspace_root = fixture_github_clones_dir().join("serde");
    let members = ["serde", "serde_core", "serde_derive", "serde_derive_internals", "test_suite"];

    for member in &members {
        let crate_path = workspace_root.join(member);
        if !crate_path.exists() {
            continue;
        }

        // Phase 1 + 2: discovery and file parsing.
        let discovery = match run_discovery_phase(None, &[crate_path.clone()]) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("  {member}: discovery failed: {e}");
                continue;
            }
        };
        let results = analyze_files_parallel(&discovery, 0);
        let (oks, errs): (Vec<_>, Vec<_>) = results.into_iter().partition(Result::is_ok);
        if !errs.is_empty() {
            eprintln!("  {member}: {} file(s) failed to parse", errs.len());
        }
        let graphs: Vec<ParsedCodeGraph> = oks.into_iter().map(Result::unwrap).collect();

        // Phase 3: merge.
        let merged = match ParsedCodeGraph::merge_new(graphs) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("  {member}: merge failed: {e}");
                continue;
            }
        };

        // Phase 4: build tree (stops before pruning the graph).
        let (_, pruning): (_, PruningResult) = match merged.build_module_tree() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  {member}: build_module_tree failed: {e}");
                continue;
            }
        };

        // Replicate the secondary-id filter.
        let pruned_item_initial = pruning.pruned_item_ids.len();
        let pruned_item_ids: Vec<AnyNodeId> = pruning
            .pruned_item_ids
            .iter()
            .copied()
            .filter(|id| {
                !matches!(
                    id,
                    AnyNodeId::Variant(_)
                        | AnyNodeId::Field(_)
                        | AnyNodeId::Param(_)
                        | AnyNodeId::GenericParam(_)
                )
            })
            .collect_vec();

        // Per-category item counts.
        let funcs     = merged.functions()     .iter().filter(|n| pruned_item_ids.contains(&n.id.as_any())).count();
        let types     = merged.defined_types() .iter().filter(|n| pruned_item_ids.contains(&n.any_id())).count();
        let consts    = merged.consts()        .iter().filter(|n| pruned_item_ids.contains(&n.id.as_any())).count();
        let statics   = merged.statics()       .iter().filter(|n| pruned_item_ids.contains(&n.id.as_any())).count();
        let macros    = merged.macros()        .iter().filter(|n| pruned_item_ids.contains(&n.id.as_any())).count();
        let use_stmts = merged.use_statements().iter().filter(|n| pruned_item_ids.contains(&n.id.as_any())).count();
        let impls     = merged.impls()         .iter().filter(|n| pruned_item_ids.contains(&n.id.as_any())).count();
        let traits    = merged.traits()        .iter().filter(|n| pruned_item_ids.contains(&n.id.as_any())).count();
        let nonfile_mods = merged.modules().iter()
            .filter(|m| !m.is_file_based())
            .filter(|m| pruned_item_ids.contains(&m.id.as_any()))
            .count();

        let methods_before: usize = merged.impls().iter().flat_map(|imp| imp.methods.iter())
            .chain(merged.traits().iter().flat_map(|tr| tr.methods.iter())).count();
        let methods_after: usize = merged.impls().iter()
            .filter(|n| !pruned_item_ids.contains(&n.id.as_any()))
            .flat_map(|imp| imp.methods.iter())
            .chain(
                merged.traits().iter()
                    .filter(|n| !pruned_item_ids.contains(&n.id.as_any()))
                    .flat_map(|tr| tr.methods.iter()),
            )
            .count();
        let methods_removed = methods_before - methods_after;

        // Method IDs in pruned_item_ids (AnyNodeId::Method variants).
        let method_ids_in_pruned = pruning
            .pruned_item_ids.iter()
            .filter(|id| matches!(id, AnyNodeId::Method(_)))
            .count();

        // IDs in pruned_item_ids not matched by any retain call.
        let all_top_level: std::collections::HashSet<AnyNodeId> = merged
            .functions()     .iter().map(|n| n.id.as_any())
            .chain(merged.defined_types() .iter().map(|n| n.any_id()))
            .chain(merged.consts()        .iter().map(|n| n.id.as_any()))
            .chain(merged.statics()       .iter().map(|n| n.id.as_any()))
            .chain(merged.macros()        .iter().map(|n| n.id.as_any()))
            .chain(merged.use_statements().iter().map(|n| n.id.as_any()))
            .chain(merged.impls()         .iter().map(|n| n.id.as_any()))
            .chain(merged.traits()        .iter().map(|n| n.id.as_any()))
            .chain(merged.modules()       .iter().map(|n| n.id.as_any()))
            .collect();

        let truly_phantom: Vec<AnyNodeId> = pruned_item_ids
            .iter().copied()
            .filter(|id| !all_top_level.contains(id))
            .filter(|id| !matches!(id, AnyNodeId::Method(_)))
            .collect_vec();

        let total_simulated = funcs + types + consts + statics + macros
            + use_stmts + impls + traits + methods_removed + nonfile_mods;

        let secondary_count = pruned_item_initial - pruned_item_ids.len();

        eprintln!(
            "\n--- Member: {member} ---\n\
             pruned_item_ids total (all)     : {pruned_item_initial}\n\
             - secondary                     : {secondary_count}\n\
             non-secondary                   : {}\n\
               method IDs in pruned_item_ids : {method_ids_in_pruned}\n\
             \n\
             Items removed by retain calls:\n\
               functions                     : {funcs}\n\
               defined_types                 : {types}\n\
               consts                        : {consts}\n\
               statics                       : {statics}\n\
               macros                        : {macros}\n\
               use_statements                : {use_stmts}\n\
               impls                         : {impls}\n\
               traits                        : {traits}\n\
               methods delta (not retain)    : {methods_removed}\n\
               non-file modules              : {nonfile_mods}\n\
             \n\
             total_simulated               : {total_simulated}\n\
             pruned_item_ids (non-sec)     : {}\n\
             difference                    : {}\n\
             truly phantom (non-method IDs not in any collection): {}\n",
            pruned_item_ids.len(),
            pruned_item_ids.len(),
            total_simulated as isize - pruned_item_ids.len() as isize,
            truly_phantom.len(),
        );

        // Print the truly phantom IDs to identify what they are.
        if !truly_phantom.is_empty() {
            eprintln!("  Truly phantom IDs:");
            for id in &truly_phantom {
                eprintln!("    {:?}", id);
            }
        }
    }
}

// ===========================================================================
// Diagnostic: identify the crate-root module and confirm build.rs exclusion
// ===========================================================================

/// Confirms that `build.rs` is excluded from discovery, and prints full
/// details about the crate-root module (the one whose display ID matches
/// `S:f0e93454..4c81af2b` in the tracing output), including its name,
/// canonical path, file, and every outgoing relation.
///
/// Run with:
///   cargo test -p syn_parser --test mod \
///     'github_clones::diagnose_serde_crate_root_module' -- --nocapture
#[test]
fn diagnose_serde_crate_root_module() {
    init_tracing();

    use syn_parser::{
        discovery::run_discovery_phase,
        parser::{
            analyze_files_parallel,
            graph::ParsedCodeGraph,
            nodes::{AsAnyNodeId, ModuleNodeId},
            relations::SyntacticRelation,
        },
        GraphAccess,
    };

    let serde_path = ploke_common::fixture_github_clones_dir()
        .join("serde")
        .join("serde");

    // -----------------------------------------------------------------------
    // Phase 1: discovery – print the discovered file list
    // -----------------------------------------------------------------------
    let discovery = run_discovery_phase(None, &[serde_path.clone()])
        .expect("Discovery must succeed for serde github clone");

    let ctx = discovery
        .get_crate_context(&serde_path)
        .expect("CrateContext must be present for serde/serde");

    let has_build_rs = ctx
        .files
        .iter()
        .any(|p| p.file_name().map_or(false, |f| f == "build.rs"));

    eprintln!(
        "\n=== Serde discovery results ===\n\
         crate root : {}\n\
         files discovered : {}\n\
         build.rs included? : {}\n",
        serde_path.display(),
        ctx.files.len(),
        has_build_rs,
    );

    // List every file that is NOT under src/ (there should be none).
    let outside_src: Vec<_> = ctx
        .files
        .iter()
        .filter(|p| {
            let src_prefix = serde_path.join("src");
            !p.starts_with(&src_prefix)
        })
        .collect();
    if outside_src.is_empty() {
        eprintln!("  All {} files are under src/ ✓", ctx.files.len());
    } else {
        eprintln!("  Files outside src/ ({}):", outside_src.len());
        for f in &outside_src {
            eprintln!("    {}", f.display());
        }
    }

    assert!(
        !has_build_rs,
        "build.rs must NOT be in the discovered file list – \
         it is a build script, not a library source file"
    );

    // -----------------------------------------------------------------------
    // Phase 2 + 3: parse and merge
    // -----------------------------------------------------------------------
    let results = analyze_files_parallel(&discovery, 0);
    let (oks, errs): (Vec<_>, Vec<_>) = results.into_iter().partition(Result::is_ok);
    if !errs.is_empty() {
        for e in &errs {
            eprintln!("Phase 2 file error: {:#?}", e);
        }
        panic!("{} file(s) failed during phase-2", errs.len());
    }
    let graphs: Vec<ParsedCodeGraph> = oks.into_iter().map(Result::unwrap).collect();

    let merged = ParsedCodeGraph::merge_new(graphs)
        .expect("merge_new should succeed for serde");

    // -----------------------------------------------------------------------
    // Find the crate-root module (the one whose short ID starts with f0e93454)
    // and also dump every module whose displayed ID contains the string
    // "f0e93454" so we can confirm it is lib.rs.
    // -----------------------------------------------------------------------
    let target_short_prefix = "f0e93454";

    eprintln!("\n=== All modules (name / id / file) ===");
    let mut found_root: Option<ModuleNodeId> = None;
    for m in merged.modules() {
        let id_str = m.id.to_string();
        let file_str = m
            .file_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<inline/decl>".to_string());
        eprintln!("  {:50}  {}  file={}", id_str, m.name, file_str);
        if id_str.contains(target_short_prefix) {
            found_root = Some(m.id);
        }
    }

    // -----------------------------------------------------------------------
    // Dump relations out of the crate-root module
    // -----------------------------------------------------------------------
    if let Some(root_id) = found_root {
        let root_mod = merged
            .modules()
            .iter()
            .find(|m| m.id == root_id)
            .expect("root module must exist");

        eprintln!(
            "\n=== Crate-root module details ===\n\
             id         : {}\n\
             name       : {}\n\
             file       : {:?}\n\
             is_file_based: {}\n\
             items count: {}",
            root_mod.id,
            root_mod.name,
            root_mod.file_path(),
            root_mod.is_file_based(),
            root_mod.items().map_or(0, |i| i.len()),
        );

        eprintln!("\n  Outgoing Contains/ModuleImports relations:");
        let root_any = root_id.as_any();
        for rel in merged.relations() {
            match rel {
                SyntacticRelation::Contains { source, target }
                    if source.as_any() == root_any =>
                {
                    let name = merged
                        .find_node_unique(target.as_any())
                        .ok()
                        .map(|n| n.name().to_string())
                        .unwrap_or_else(|| "<err>".into());
                    eprintln!("    Contains  → {:?}  (name={})", target, name);
                }
                SyntacticRelation::ModuleImports { source, target }
                    if source.as_any() == root_any =>
                {
                    let name = merged
                        .find_node_unique((*target).as_any())
                        .ok()
                        .map(|n| n.name().to_string())
                        .unwrap_or_else(|| "<err>".into());
                    eprintln!("    ModuleImports → {:?}  (name={})", target, name);
                }
                _ => {}
            }
        }
    } else {
        eprintln!(
            "\n  NOTE: no module with short-ID prefix '{}' found in this run \
             (IDs are hash-derived and may differ across machines/versions).",
            target_short_prefix
        );
        eprintln!("  Look for the lib.rs file-based module above.");
    }

    // -----------------------------------------------------------------------
    // Print modules referenced by `serde_core_private` imports
    // -----------------------------------------------------------------------
    eprintln!("\n=== Use-statements referencing serde_core_private ===");
    let mut found_any = false;
    for imp in merged.use_statements() {
        let path_str = imp.source_path.join("::");
        if path_str.contains("serde_core_private") {
            found_any = true;
            eprintln!(
                "  id={} name={} path={} vis={:?}",
                imp.id, imp.visible_name, path_str, imp.kind
            );
        }
    }
    if !found_any {
        eprintln!("  (none found – serde_core_private is not in any use-statement path)");
    }
}

