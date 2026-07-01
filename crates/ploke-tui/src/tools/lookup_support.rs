use ploke_core::{
    io_types::EmbeddingData,
    rag_types::{CallContextInfo, CallImpactInfo, CallPathInfo, CallReachInfo, ProofContextInfo},
    tool_types::ToolName,
};
use ploke_db::{
    CallPathOptions, Database, DbError,
    helpers::{
        graph_resolve_exact, graph_resolve_exact_impl_method, graph_resolve_exact_trait_method,
        graph_resolve_exact_variant,
    },
};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::rag::utils::NodeKind;

use super::{
    ToolError, ToolErrorCode, ToolInvocationError, ToolRetryContext, ValidatesAbolutePath,
};

pub(super) const MODULE_PATH_DESC: &str = r#"crate-relative module path, e.g. "crate" or "crate::mod_one::nested_mod".
Do not use the Cargo package/crate name, and do not include the target item's identifier.
If the module path is unknown, use request_code_context or read_file before exact lookup."#;

pub(super) const MODULE_PATH_EXPECTED: &str =
    "crate or crate::module::submodule, without the item name";

pub(super) const LOOKUP_RETRY_HINT: &str = "Use a crate-relative module_path that begins with `crate`. If the exact module path is uncertain, call request_code_context with the item name/signature or read_file on the target file before retrying exact lookup.";

pub(super) const OWNER_TRAIT_DESC: &str = r#"Optional trait name that owns a method item.
Use only with node_kind=method when file_path, module_path, and item_name are ambiguous.
Example: owner_trait="Handler" for Handler::call."#;

pub(super) const OWNER_TYPE_DESC: &str = r#"Optional self type name that owns an inherent method item.
Use only with node_kind=method when file_path, module_path, and item_name are ambiguous.
Example: owner_type="HandleError" for HandleError::new."#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum OwnerQualifier {
    Trait(String),
    Type(String),
}

impl OwnerQualifier {
    pub(super) fn message(&self) -> String {
        match self {
            Self::Trait(owner) => format!(" and owner_trait {owner}"),
            Self::Type(owner) => format!(" and owner_type {owner}"),
        }
    }
}

pub(super) fn validate_module_path(
    tool: ToolName,
    module_path: &str,
) -> Result<(), ToolInvocationError> {
    let parts = module_path
        .split("::")
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>();
    if parts.first().copied() == Some("crate") {
        Ok(())
    } else {
        Err(ToolInvocationError::Validation(module_path_error(
            tool,
            module_path,
        )))
    }
}

pub(super) fn module_path_error(tool: ToolName, received: &str) -> ToolError {
    let received = display_received(received);
    ToolError::new(
        tool,
        ToolErrorCode::InvalidFormat,
        module_path_error_message(received),
    )
    .field("module_path")
    .expected(MODULE_PATH_EXPECTED)
    .received(received)
    .retry_hint(LOOKUP_RETRY_HINT)
    .retry_context(
        ToolRetryContext::new()
            .field("example_root_module_path", "crate")
            .field("example_nested_module_path", "crate::module::submodule")
            .field(
                "wrong_pattern",
                "Cargo package names such as ploke_db_derive are not module_path values",
            )
            .field(
                "discovery_tools",
                vec!["request_code_context", "read_file", "list_dir"],
            ),
    )
}

pub(super) fn module_path_error_message(received: &str) -> String {
    format!(
        "module_path must start with \"crate\" and must be the module path without the target item name; received {received}"
    )
}

pub(super) fn item_canon_path(module_path: &str, item_name: &str) -> String {
    let module_path = module_path.trim().trim_end_matches("::");
    let item_name = item_name.trim().trim_start_matches("::");
    if module_path.is_empty() {
        item_name.to_string()
    } else if item_name.is_empty() {
        module_path.to_string()
    } else {
        format!("{module_path}::{item_name}")
    }
}

pub(super) fn normalize_owner_qualifier(
    owner_trait: Option<&str>,
    owner_type: Option<&str>,
    node_kind: NodeKind,
) -> Result<Option<OwnerQualifier>, ploke_error::Error> {
    let owner_trait = owner_trait
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let owner_type = owner_type
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    if owner_trait.is_some() && owner_type.is_some() {
        return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
            message: "Provide only one owner qualifier: owner_trait or owner_type.".to_string(),
        }));
    }
    if (owner_trait.is_some() || owner_type.is_some()) && !matches!(node_kind, NodeKind::Method) {
        return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
            message: "owner qualifiers can only be used when node_kind is `method`.".to_string(),
        }));
    }

    Ok(owner_trait
        .map(OwnerQualifier::Trait)
        .or_else(|| owner_type.map(OwnerQualifier::Type)))
}

pub(super) fn resolve_exact_item(
    db: &Database,
    node_kind: NodeKind,
    abs_path: &Path,
    mod_path: &[String],
    item_name: &str,
    owner: Option<&OwnerQualifier>,
) -> Result<Vec<EmbeddingData>, DbError> {
    match owner {
        Some(OwnerQualifier::Trait(owner)) => {
            graph_resolve_exact_trait_method(db, abs_path, mod_path, item_name, owner)
        }
        Some(OwnerQualifier::Type(owner)) => {
            graph_resolve_exact_impl_method(db, abs_path, mod_path, item_name, owner)
        }
        None if matches!(node_kind, NodeKind::Variant) => {
            graph_resolve_exact_variant(db, abs_path, mod_path, item_name)
        }
        None => graph_resolve_exact(db, node_kind.as_relation(), abs_path, mod_path, item_name),
    }
}

pub(super) struct ResolvedToolItem {
    pub(super) id: Uuid,
    pub(super) rel_path: PathBuf,
}

pub(super) struct ExactItemRequest<'a> {
    pub(super) item_name: &'a str,
    pub(super) file_path: &'a str,
    pub(super) node_kind: &'a str,
    pub(super) module_path: &'a str,
    pub(super) owner_trait: Option<&'a str>,
    pub(super) owner_type: Option<&'a str>,
}

impl<'a> ValidatesAbolutePath for ExactItemRequest<'a> {
    fn get_file_path(&self) -> impl AsRef<std::path::Path> {
        Path::new(self.file_path)
    }
}

pub(super) fn resolve_exact_tool_item(
    db: &Database,
    primary_root: &Path,
    policy: &ploke_io::path_policy::PathPolicy,
    request: ExactItemRequest<'_>,
) -> Result<ResolvedToolItem, ploke_error::Error> {
    use ploke_error::{DomainError, InternalError};

    let node_kind = request.node_kind.parse::<NodeKind>().map_err(|_| {
        ploke_error::Error::Domain(DomainError::Ui {
            message: format!(
                "Invalid node_kind `{}`. Allowed: {}",
                request.node_kind,
                NodeKind::allowed_values().join(", ")
            ),
        })
    })?;
    let owner = normalize_owner_qualifier(request.owner_trait, request.owner_type, node_kind)?;
    let abs_path = request
        .validate_to_abs_path(primary_root, policy)
        .map_err(|err| {
            ploke_error::Error::Domain(DomainError::Ui {
                message: format!(
                    r#"The target file could not be found at the resolved absolute path.
Original error message: {err} This indicates an incorrect file path.
Tip: consider using `request_code_context` with the item name, signature, or anticipated contents
for a more fuzzy search."#
                )
                .to_string(),
            })
        })?;
    let rel_path = abs_path
        .strip_prefix(primary_root)
        .map_err(|_| ploke_error::Error::Internal(InternalError::InvalidState("Error stripping relative path from absolute path. This indicates an error with the ploke application itself. Please consider filing an issue at the ploke github.")))?;
    let mod_path = request
        .module_path
        .split("::")
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if mod_path.is_empty() || mod_path.first().map(|part| part.as_str()) != Some("crate") {
        return Err(ploke_error::Error::Domain(DomainError::Ui {
            message: module_path_error_message(request.module_path),
        }));
    }

    let resolved = match resolve_exact_item(
        db,
        node_kind,
        &abs_path,
        &mod_path,
        request.item_name,
        owner.as_ref(),
    ) {
        Ok(items) if items.len() == 1 => items,
        Ok(items) if items.is_empty() => {
            let hint = node_kind
                .lookup_hint()
                .map(|hint| format!(" {hint}"))
                .unwrap_or_default();
            return Err(ploke_error::Error::Domain(DomainError::Ui {
                message: format!(
                    "No code item named `{}` found in {} with module_path {} and node_kind {}{}.{}",
                    request.item_name,
                    rel_path.display(),
                    request.module_path,
                    node_kind.as_str(),
                    owner_message(owner.as_ref()),
                    hint
                ),
            }));
        }
        Ok(_) => {
            return Err(ploke_error::Error::Domain(DomainError::Ui {
                message: format!(
                    "Multiple items matched `{}` in {} with module_path {} and node_kind {}{}; expected a single match.",
                    request.item_name,
                    rel_path.display(),
                    request.module_path,
                    node_kind.as_str(),
                    owner_message(owner.as_ref())
                ),
            }));
        }
        Err(err) => {
            return Err(ploke_error::Error::Internal(InternalError::CompilerError(
                format!(
                    "Database lookup failed: {err}. This indicates an issue with the ploke application itself, not an error in the search input. Please consider filing an issue at the ploke github."
                ),
            )));
        }
    };

    Ok(ResolvedToolItem {
        id: resolved[0].id,
        rel_path: rel_path.to_path_buf(),
    })
}

pub(super) fn owner_message(owner: Option<&OwnerQualifier>) -> String {
    owner.map(OwnerQualifier::message).unwrap_or_default()
}

pub(super) struct ContextCarriers {
    pub(super) call_context: Vec<CallContextInfo>,
    pub(super) proof_context: Vec<ProofContextInfo>,
}

pub(super) struct CallPathCarriers {
    pub(super) from_owner: Vec<CallPathInfo>,
    pub(super) to_target: Vec<CallPathInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CallContextCounts {
    pub(super) total: usize,
    pub(super) outgoing: usize,
    pub(super) incoming: usize,
}

pub(super) fn call_context_counts(node_id: Uuid, calls: &[CallContextInfo]) -> CallContextCounts {
    let mut outgoing = 0usize;
    let mut incoming = 0usize;
    for call in calls {
        if call.owner_id == node_id {
            outgoing += 1;
        }
        if call
            .targets
            .iter()
            .any(|target| target.target_id == node_id)
        {
            incoming += 1;
        }
    }

    CallContextCounts {
        total: calls.len(),
        outgoing,
        incoming,
    }
}

pub(super) fn context_carriers_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<ContextCarriers, ploke_error::Error> {
    use ploke_error::InternalError;

    let call_context = match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => {
            rag.exact_call_context(node_id).map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect call context for code item {node_id}: {err}"
                )))
            })?
        }
        _ => Vec::new(),
    };
    let proof_context = match ctx.state.rag.as_ref() {
        Some(rag) if !rag.proof_context_degraded() => {
            rag.exact_proof_context(node_id).map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect proof context for code item {node_id}: {err}"
                )))
            })?
        }
        _ => Vec::new(),
    };
    Ok(ContextCarriers {
        call_context,
        proof_context,
    })
}

pub(super) fn call_path_carriers_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<CallPathCarriers, ploke_error::Error> {
    use ploke_error::InternalError;

    let path_options = CallPathOptions {
        max_depth: 3,
        max_paths: 64,
    };
    let from_owner = match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => rag
            .exact_call_paths_from_owner(node_id, path_options)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect outgoing call paths for code item {node_id}: {err}"
                )))
            })?,
        _ => Vec::new(),
    };
    let to_target = match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => rag
            .exact_call_paths_to_target(node_id, path_options)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect incoming call paths for code item {node_id}: {err}"
                )))
            })?,
        _ => Vec::new(),
    };

    Ok(CallPathCarriers {
        from_owner,
        to_target,
    })
}

pub(super) fn call_impact_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Option<CallImpactInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    let path_options = CallPathOptions {
        max_depth: 3,
        max_paths: 64,
    };
    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => rag
            .exact_call_impact_for_target(node_id, path_options)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect impact summary for code item {node_id}: {err}"
                )))
            }),
        _ => Ok(None),
    }
}

pub(super) fn call_reach_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Option<CallReachInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    let path_options = CallPathOptions {
        max_depth: 3,
        max_paths: 64,
    };
    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => rag
            .exact_call_reach_for_owner(node_id, path_options)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect reach summary for code item {node_id}: {err}"
                )))
            }),
        _ => Ok(None),
    }
}

fn display_received(received: &str) -> &str {
    if received.trim().is_empty() {
        "<empty>"
    } else {
        received
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_module_path_is_structured_validation_error() {
        let err = validate_module_path(ToolName::CodeItemLookup, "ploke_db_derive")
            .expect_err("package-name module_path should fail preflight");
        let ToolInvocationError::Validation(err) = err else {
            panic!("expected validation error");
        };

        assert_eq!(err.code, ToolErrorCode::InvalidFormat);
        assert_eq!(err.field, Some("module_path"));
        assert_eq!(err.expected.as_deref(), Some(MODULE_PATH_EXPECTED));
        assert_eq!(err.received.as_deref(), Some("ploke_db_derive"));
        assert!(
            err.retry_hint
                .as_deref()
                .expect("retry hint")
                .contains("request_code_context")
        );
        assert!(err.retry_context.is_some());
    }

    #[test]
    fn item_canon_path_appends_item_to_module_path() {
        assert_eq!(item_canon_path("crate", "FieldSpec"), "crate::FieldSpec");
        assert_eq!(
            item_canon_path("crate::module::", "Thing"),
            "crate::module::Thing"
        );
    }

    #[test]
    fn owner_qualifier_accepts_one_method_owner() {
        let owner = normalize_owner_qualifier(None, Some("HandleError"), NodeKind::Method)
            .expect("owner type qualifier")
            .expect("owner type");
        assert_eq!(owner, OwnerQualifier::Type("HandleError".to_string()));
        assert_eq!(owner.message(), " and owner_type HandleError");
    }

    #[test]
    fn owner_qualifier_rejects_conflicts() {
        let err = normalize_owner_qualifier(Some("Handler"), Some("HandleError"), NodeKind::Method)
            .expect_err("conflicting owner qualifiers");
        assert!(err.to_string().contains("owner_trait or owner_type"));
    }
}
