use std::collections::BTreeMap;

use ploke_db::DbError;

use super::common::*;

#[test]
fn axum_real_target_workspace_dependency_candidates_link_selected_members() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let contexts = db.list_crate_context_rows()?;
    let axum = contexts
        .iter()
        .find(|context| context.name == "axum")
        .expect("axum fixture should include the axum crate");
    let axum_core = contexts
        .iter()
        .find(|context| context.name == "axum-core")
        .expect("axum fixture should include the axum-core crate");
    let axum_macros = contexts
        .iter()
        .find(|context| context.name == "axum-macros")
        .expect("axum fixture should include the axum-macros crate");

    let dependencies = db.crate_dependencies_for_namespace(axum.namespace)?;
    assert!(
        dependencies.iter().any(|dep| dep.crate_name == "axum"
            && dep.dep_name == "axum-core"
            && dep.dep_kind == "normal"
            && dep.path.is_some()),
        "axum should project its path dependency on axum-core: {dependencies:#?}"
    );

    let candidates = db.workspace_dependency_candidates(axum.namespace)?;
    let by_dep = candidates
        .iter()
        .map(|candidate| (candidate.dependency.dep_name.as_str(), candidate))
        .collect::<BTreeMap<_, _>>();

    // Matrix connection:
    //   axum/src/extract/state.rs imports `axum_core::extract::FromRef`.
    //   The workspace-aware call resolver consumes this carrier for the
    //   top-level State extractor `FromRef::from_ref` row and the nested local
    //   impl method row in middleware/from_extractor.rs.
    let core_candidate = by_dep
        .get("axum-core")
        .expect("axum-core path dependency should resolve to a parsed workspace crate");
    assert_eq!(core_candidate.dependency.crate_name, "axum");
    assert_eq!(core_candidate.dependency.dep_kind, "normal");
    assert_eq!(core_candidate.target.name, "axum-core");
    assert_eq!(core_candidate.target.namespace, axum_core.namespace);
    assert_eq!(core_candidate.target.root_path, axum_core.root_path);

    let macros_candidate = by_dep
        .get("axum-macros")
        .expect("axum-macros path dependency should resolve to a parsed workspace crate");
    assert_eq!(macros_candidate.dependency.crate_name, "axum");
    assert_eq!(macros_candidate.target.name, "axum-macros");
    assert_eq!(macros_candidate.target.namespace, axum_macros.namespace);
    assert_eq!(macros_candidate.target.root_path, axum_macros.root_path);

    Ok(())
}
