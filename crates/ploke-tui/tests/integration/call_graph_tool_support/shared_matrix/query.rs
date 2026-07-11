use ploke_test_utils::{CallCorpusFixture, CallExpected, CallOwnerSelector, CallTargetSelector};

use super::*;

pub(super) struct MatrixQuery {
    pub(super) file_path: PathBuf,
    pub(super) module_path: Vec<String>,
    pub(super) item_name: &'static str,
    pub(super) node_kind: &'static str,
    pub(super) owner_trait: Option<&'static str>,
    pub(super) owner_type: Option<&'static str>,
    pub(super) direction: QueryDirection,
}

#[derive(Clone, Copy)]
pub(super) enum QueryDirection {
    Incoming,
    Outgoing,
}

pub(super) fn query_node_id(
    query: &MatrixQuery,
    owner: &TargetInfo,
    target: Option<&TargetInfo>,
) -> Uuid {
    match query.direction {
        QueryDirection::Incoming => target.expect("incoming query target").id,
        QueryDirection::Outgoing => owner.id,
    }
}

pub(super) fn query_for_target(target: &TargetInfo, expected: CallExpected) -> MatrixQuery {
    let (item_name, node_kind) = match expected {
        CallExpected::Resolved { target, .. } => match target {
            CallTargetSelector::FunctionInModule { name, .. } => (name, "function"),
            CallTargetSelector::Struct { name } => (name, "struct"),
            CallTargetSelector::Variant { variant_name, .. } => (variant_name, "variant"),
        },
        CallExpected::Targetless { .. } => unreachable!("target query requires resolved case"),
    };
    MatrixQuery {
        file_path: target.file_path.clone(),
        module_path: target.module_path.clone(),
        item_name,
        node_kind,
        owner_trait: None,
        owner_type: None,
        direction: QueryDirection::Incoming,
    }
}

pub(super) fn query_for_owner(owner: &TargetInfo, selector: CallOwnerSelector) -> MatrixQuery {
    match selector {
        CallOwnerSelector::FunctionInModule { name, .. } => MatrixQuery {
            file_path: owner.file_path.clone(),
            module_path: owner.module_path.clone(),
            item_name: name,
            node_kind: "function",
            owner_trait: None,
            owner_type: None,
            direction: QueryDirection::Outgoing,
        },
        CallOwnerSelector::MethodByBody {
            name,
            owner_type,
            owner_trait,
            ..
        } => MatrixQuery {
            file_path: owner.file_path.clone(),
            module_path: owner.module_path.clone(),
            item_name: name,
            node_kind: "method",
            owner_trait,
            owner_type,
            direction: QueryDirection::Outgoing,
        },
        CallOwnerSelector::MethodByBodyFile { name, .. } => MatrixQuery {
            file_path: owner.file_path.clone(),
            module_path: owner.module_path.clone(),
            item_name: name,
            node_kind: "method",
            owner_trait: None,
            owner_type: None,
            direction: QueryDirection::Outgoing,
        },
    }
}

pub(super) fn crate_root_from_file(file_path: &std::path::Path, label: &str) -> PathBuf {
    let src_dir = file_path
        .ancestors()
        .find(|path| path.file_name().and_then(|name| name.to_str()) == Some("src"))
        .unwrap_or_else(|| panic!("{label} file should live under a crate src directory"));
    src_dir
        .parent()
        .unwrap_or_else(|| panic!("{label} src directory should have a crate root"))
        .to_path_buf()
}

pub(super) fn build_domain(fixture: CallCorpusFixture) -> &'static str {
    match fixture {
        CallCorpusFixture::Axum => "bd:corpus-axum-call-graph",
        CallCorpusFixture::Chrono => "bd:corpus-chrono-call-graph",
        CallCorpusFixture::Memchr => "bd:corpus-memchr-call-graph",
        CallCorpusFixture::GenericArray => "bd:corpus-generic-array-call-graph",
    }
}
