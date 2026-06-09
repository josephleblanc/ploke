use std::env;
use std::path::{Path, PathBuf};

use syn_parser::parser::nodes::{AssociatedItemNodeId, TypeDefNode};
use syn_parser::parser::relations::SyntacticRelation;
use syn_parser::{GraphAccess, try_run_phases_and_merge};

const RIPGREP_CRATE_ENV: &str = "PLOKE_RIPGREP_IGNORE_CRATE";

fn target_crate_root() -> Option<PathBuf> {
    let Ok(raw_path) = env::var(RIPGREP_CRATE_ENV) else {
        eprintln!(
            "skipping ripgrep ignore regression; set {RIPGREP_CRATE_ENV}=/path/to/ripgrep/crates/ignore"
        );
        return None;
    };

    let path = PathBuf::from(raw_path);
    let crate_root = if path.file_name().is_some_and(|name| name == "Cargo.toml") {
        path.parent()
            .expect("Cargo.toml path must have a parent")
            .to_path_buf()
    } else {
        path
    };

    assert_crate_shape(&crate_root);
    Some(crate_root)
}

fn assert_crate_shape(crate_root: &Path) {
    assert!(
        crate_root.join("Cargo.toml").is_file(),
        "{RIPGREP_CRATE_ENV} must point at the ripgrep ignore crate root or Cargo.toml"
    );
    assert!(
        crate_root.join("src").join("dir.rs").is_file(),
        "ripgrep ignore crate must contain src/dir.rs"
    );
}

#[test]
fn ripgrep_ignore_crate_parses_and_locates_ignore_matched_ignore() {
    let Some(crate_root) = target_crate_root() else {
        return;
    };

    let mut parser_output =
        try_run_phases_and_merge(&crate_root).expect("ripgrep ignore crate should parse and merge");
    let graph = parser_output
        .extract_merged_graph()
        .expect("parser output should contain a merged graph");

    let ignore_struct_count = graph
        .defined_types()
        .iter()
        .filter(
            |type_def| matches!(type_def, TypeDefNode::Struct(struct_node) if struct_node.name == "Ignore"),
        )
        .count();
    assert_eq!(
        ignore_struct_count, 1,
        "expected exactly one `Ignore` struct in the parsed ignore crate"
    );

    let method_matches: Vec<_> = graph
        .impls()
        .iter()
        .flat_map(|impl_node| {
            impl_node
                .methods()
                .iter()
                .filter(|method| method.name == "matched_ignore")
                .map(move |method| (impl_node, method))
        })
        .collect();
    assert_eq!(
        method_matches.len(),
        1,
        "expected exactly one `matched_ignore` method in the parsed ignore crate"
    );

    let (owner_impl, method) = method_matches[0];
    let target = AssociatedItemNodeId::Method(method.method_id());
    let relation_found = graph.relations().iter().any(|relation| {
        matches!(
            relation,
            SyntacticRelation::ImplAssociatedItem { source, target: associated_item }
                if *source == owner_impl.id() && *associated_item == target
        )
    });

    assert!(
        relation_found,
        "expected an ImplAssociatedItem relation from the owning impl to `matched_ignore`"
    );
}
