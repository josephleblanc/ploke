use ploke_core::{
    io_types::EmbeddingData,
    rag_types::{
        AwaitedCallSiteInfo, CallBuildDomainInfo, CallContextInfo, CallEffectPolicyViolationInfo,
        CallImpactInfo, CallPathInfo, CallProofInvariantFindingInfo, CallReachEffectInfo,
        CallReachInfo, CallTestEntrypointInfo, CallTestSelectionInfo, CrateBoundaryEdgeInfo,
        ExternalSummaryNeedInfo, LocalBindingEdgeInfo, LocalBindingInfo, ModuleBoundaryEdgeInfo,
        ProofContextInfo, ReturnedCallBindingFlowInfo, ReturnedFutureExecutionFlowInfo,
        ReturnedFutureFlowInfo, RuntimeDispatchNeedInfo,
    },
    tool_types::ToolName,
};
use ploke_db::{
    CallPathOptions, Database, DbError,
    helpers::{
        graph_resolve_exact, graph_resolve_exact_call_body_owner,
        graph_resolve_exact_call_body_owner_for_parent, graph_resolve_exact_impl_method,
        graph_resolve_exact_trait_impl_method, graph_resolve_exact_trait_method,
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
Use only with node_kind=method. Use alone for trait method declarations, or combine with owner_type for trait impl methods.
For trait impl methods, include one generic root when needed to disambiguate overloads.
Examples: owner_trait="Handler" for Handler::call; owner_trait="Service<Request>" with owner_type="HandlerService" for impl Service<Request<B>> for HandlerService::call."#;

pub(super) const OWNER_TYPE_DESC: &str = r#"Optional self type name that owns an inherent method item.
Use only with node_kind=method. Use alone for inherent methods, or combine with owner_trait for trait impl methods.
Examples: owner_type="HandleError" for HandleError::new; owner_type="HandlerService" with owner_trait="Service<Request>" for impl Service<Request<B>> for HandlerService::call."#;

pub(super) const PARENT_NAME_DESC: &str = r#"Optional parent item name for executable body-owner nodes.
Use only with node_kind=closure, node_kind=async_block, or node_kind=local_item when file_path, module_path, item_name, and node_kind would otherwise match multiple nested executable owners.
Example: parent_name="test_from_extractor" for a function-local impl method such as local_impl_method:from_request_parts."#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum OwnerQualifier {
    Trait(String),
    Type(String),
    TraitImpl {
        trait_name: String,
        type_name: String,
        trait_arg: Option<String>,
    },
}

impl OwnerQualifier {
    pub(super) fn message(&self) -> String {
        match self {
            Self::Trait(owner) => format!(" and owner_trait {owner}"),
            Self::Type(owner) => format!(" and owner_type {owner}"),
            Self::TraitImpl {
                trait_name,
                type_name,
                trait_arg,
            } => {
                let trait_owner = trait_arg
                    .as_ref()
                    .map(|arg| format!("{trait_name}<{arg}>"))
                    .unwrap_or_else(|| trait_name.clone());
                format!(" and owner_trait {trait_owner} and owner_type {type_name}")
            }
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
        .map(parse_owner_trait_qualifier)
        .transpose()?;
    let owner_type = owner_type
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    if (owner_trait.is_some() || owner_type.is_some()) && !matches!(node_kind, NodeKind::Method) {
        return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
            message: "owner qualifiers can only be used when node_kind is `method`.".to_string(),
        }));
    }

    Ok(match (owner_trait, owner_type) {
        (Some((trait_name, trait_arg)), Some(type_name)) => Some(OwnerQualifier::TraitImpl {
            trait_name,
            type_name,
            trait_arg,
        }),
        (Some((owner, _)), None) => Some(OwnerQualifier::Trait(owner)),
        (None, Some(owner)) => Some(OwnerQualifier::Type(owner)),
        (None, None) => None,
    })
}

pub(super) fn normalize_parent_name(
    parent_name: Option<&str>,
    node_kind: NodeKind,
) -> Result<Option<String>, ploke_error::Error> {
    let parent = parent_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    if parent.is_some() && node_kind.call_body_owner_kind().is_none() {
        return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
            message: "parent_name can only be used with executable body-owner node kinds: `closure`, `async_block`, or `local_item`.".to_string(),
        }));
    }

    Ok(parent)
}

fn parse_owner_trait_qualifier(
    owner_trait: &str,
) -> Result<(String, Option<String>), ploke_error::Error> {
    let Some(open_idx) = owner_trait.find('<') else {
        return Ok((owner_trait.to_string(), None));
    };
    let Some(close_idx) = owner_trait.rfind('>') else {
        return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
            message: format!("owner_trait `{owner_trait}` has an unmatched `<`."),
        }));
    };
    if close_idx + 1 != owner_trait.len() {
        return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
            message: format!(
                "owner_trait `{owner_trait}` must use a single trailing generic root, e.g. Service<Request>."
            ),
        }));
    }
    let trait_name = owner_trait[..open_idx].trim();
    let trait_arg = owner_trait[open_idx + 1..close_idx].trim();
    if trait_name.is_empty() || trait_arg.is_empty() || trait_arg.contains(',') {
        return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
            message: format!(
                "owner_trait `{owner_trait}` must use one non-empty generic root, e.g. Service<Request>."
            ),
        }));
    }
    Ok((trait_name.to_string(), Some(trait_arg.to_string())))
}

pub(super) fn resolve_exact_item(
    db: &Database,
    node_kind: NodeKind,
    abs_path: &Path,
    mod_path: &[String],
    item_name: &str,
    owner: Option<&OwnerQualifier>,
    parent_name: Option<&str>,
) -> Result<Vec<EmbeddingData>, DbError> {
    match owner {
        Some(OwnerQualifier::Trait(owner)) => {
            graph_resolve_exact_trait_method(db, abs_path, mod_path, item_name, owner)
        }
        Some(OwnerQualifier::Type(owner)) => {
            graph_resolve_exact_impl_method(db, abs_path, mod_path, item_name, owner)
        }
        Some(OwnerQualifier::TraitImpl {
            trait_name,
            type_name,
            trait_arg,
        }) => graph_resolve_exact_trait_impl_method(
            db,
            abs_path,
            mod_path,
            item_name,
            trait_name,
            type_name,
            trait_arg.as_deref(),
        ),
        None if matches!(node_kind, NodeKind::Variant) => {
            graph_resolve_exact_variant(db, abs_path, mod_path, item_name)
        }
        None if let Some(owner_kind) = node_kind.call_body_owner_kind() => {
            if let Some(parent) = parent_name {
                graph_resolve_exact_call_body_owner_for_parent(
                    db, abs_path, mod_path, item_name, owner_kind, parent,
                )
            } else {
                graph_resolve_exact_call_body_owner(db, abs_path, mod_path, item_name, owner_kind)
            }
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
    pub(super) parent_name: Option<&'a str>,
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
    let parent = normalize_parent_name(request.parent_name, node_kind)?;
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
        parent.as_deref(),
    ) {
        Ok(items) if items.len() == 1 => items,
        Ok(items) if items.is_empty() => {
            let hint = node_kind
                .lookup_hint()
                .map(|hint| format!(" {hint}"))
                .unwrap_or_default();
            return Err(ploke_error::Error::Domain(DomainError::Ui {
                message: format!(
                    "No code item named `{}` found in {} with module_path {} and node_kind {}{}{}.{}",
                    request.item_name,
                    rel_path.display(),
                    request.module_path,
                    node_kind.as_str(),
                    owner_message(owner.as_ref()),
                    parent_message(parent.as_deref()),
                    hint
                ),
            }));
        }
        Ok(_) => {
            return Err(ploke_error::Error::Domain(DomainError::Ui {
                message: format!(
                    "Multiple items matched `{}` in {} with module_path {} and node_kind {}{}{}; expected a single match.",
                    request.item_name,
                    rel_path.display(),
                    request.module_path,
                    node_kind.as_str(),
                    owner_message(owner.as_ref()),
                    parent_message(parent.as_deref())
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

pub(super) fn parent_message(parent_name: Option<&str>) -> String {
    parent_name
        .map(|parent| format!(" and parent_name {parent}"))
        .unwrap_or_default()
}

pub(super) struct ContextCarriers {
    pub(super) call_context: Vec<CallContextInfo>,
    pub(super) proof_context: Vec<ProofContextInfo>,
}

pub(super) struct CallPathCarriers {
    pub(super) from_owner: Vec<CallPathInfo>,
    pub(super) to_target: Vec<CallPathInfo>,
    pub(super) cycles_from_owner: Vec<CallPathInfo>,
}

const TOOL_CALL_PATH_OPTIONS: CallPathOptions = CallPathOptions {
    max_depth: 2,
    max_paths: 64,
};

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

    let from_owner = match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => rag
            .exact_call_paths_from_owner(node_id, TOOL_CALL_PATH_OPTIONS)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect outgoing call paths for code item {node_id}: {err}"
                )))
            })?,
        _ => Vec::new(),
    };
    let to_target = match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => rag
            .exact_call_paths_to_target(node_id, TOOL_CALL_PATH_OPTIONS)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect incoming call paths for code item {node_id}: {err}"
                )))
            })?,
        _ => Vec::new(),
    };
    let cycles_from_owner = from_owner
        .iter()
        .filter(|path| path.end_id == node_id && !path.edges.is_empty())
        .cloned()
        .collect();

    Ok(CallPathCarriers {
        from_owner,
        to_target,
        cycles_from_owner,
    })
}

pub(super) fn call_impact_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Option<CallImpactInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => rag
            .exact_call_impact_for_target(node_id, TOOL_CALL_PATH_OPTIONS)
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

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => rag
            .exact_call_reach_for_owner(node_id, TOOL_CALL_PATH_OPTIONS)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect reach summary for code item {node_id}: {err}"
                )))
            }),
        _ => Ok(None),
    }
}

pub(super) fn call_reach_effects_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Vec<CallReachEffectInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => Ok(rag
            .exact_call_effects_reachable_from_owner(node_id, TOOL_CALL_PATH_OPTIONS)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect reachable effects for code item {node_id}: {err}"
                )))
            })?
            .unwrap_or_default()),
        _ => Ok(Vec::new()),
    }
}

pub(super) fn external_summary_needs_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Vec<ExternalSummaryNeedInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => Ok(rag
            .exact_external_summary_needs_for_owner(node_id, TOOL_CALL_PATH_OPTIONS)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect external summary needs for code item {node_id}: {err}"
                )))
            })?
            .unwrap_or_default()),
        _ => Ok(Vec::new()),
    }
}

pub(super) fn runtime_dispatch_needs_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Vec<RuntimeDispatchNeedInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => Ok(rag
            .exact_runtime_dispatch_needs_for_owner(node_id, TOOL_CALL_PATH_OPTIONS)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect runtime dispatch needs for code item {node_id}: {err}"
                )))
            })?
            .unwrap_or_default()),
        _ => Ok(Vec::new()),
    }
}

pub(super) fn local_bindings_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Vec<LocalBindingInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => Ok(rag
            .exact_local_bindings_for_owner(node_id)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect local bindings for code item {node_id}: {err}"
                )))
            })?
            .unwrap_or_default()),
        _ => Ok(Vec::new()),
    }
}

pub(super) fn local_binding_edges_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Vec<LocalBindingEdgeInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => Ok(rag
            .exact_local_binding_edges_for_owner(node_id)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect local binding edges for code item {node_id}: {err}"
                )))
            })?
            .unwrap_or_default()),
        _ => Ok(Vec::new()),
    }
}

pub(super) fn returned_call_binding_flows_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Vec<ReturnedCallBindingFlowInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => Ok(rag
            .exact_returned_call_binding_flows_for_owner(node_id)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect returned-call binding flows for code item {node_id}: {err}"
                )))
            })?
            .unwrap_or_default()),
        _ => Ok(Vec::new()),
    }
}

pub(super) fn returned_future_flows_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Vec<ReturnedFutureFlowInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => Ok(rag
            .exact_returned_future_flows_for_owner(node_id)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect returned future flows for code item {node_id}: {err}"
                )))
            })?
            .unwrap_or_default()),
        _ => Ok(Vec::new()),
    }
}

pub(super) fn returned_future_execution_flows_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Vec<ReturnedFutureExecutionFlowInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => Ok(rag
            .exact_returned_future_execution_flows_for_owner(node_id)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect returned future execution flows for code item {node_id}: {err}"
                )))
            })?
            .unwrap_or_default()),
        _ => Ok(Vec::new()),
    }
}

pub(super) fn awaited_call_sites_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Vec<AwaitedCallSiteInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => Ok(rag
            .exact_awaited_call_sites_for_owner(node_id)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect awaited call sites for code item {node_id}: {err}"
                )))
            })?
            .unwrap_or_default()),
        _ => Ok(Vec::new()),
    }
}

pub(super) fn module_boundary_edges_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Vec<ModuleBoundaryEdgeInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => Ok(rag
            .exact_module_boundary_edges_from_owner(node_id, TOOL_CALL_PATH_OPTIONS)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect module boundary edges for code item {node_id}: {err}"
                )))
            })?
            .unwrap_or_default()),
        _ => Ok(Vec::new()),
    }
}

pub(super) fn crate_boundary_edges_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Vec<CrateBoundaryEdgeInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => Ok(rag
            .exact_crate_boundary_edges_from_owner(node_id, TOOL_CALL_PATH_OPTIONS)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect crate boundary edges for code item {node_id}: {err}"
                )))
            })?
            .unwrap_or_default()),
        _ => Ok(Vec::new()),
    }
}

pub(super) fn call_build_domains_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Vec<CallBuildDomainInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => Ok(rag
            .exact_call_build_domains_for_node(node_id)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect build domains for code item {node_id}: {err}"
                )))
            })?
            .unwrap_or_default()),
        _ => Ok(Vec::new()),
    }
}

pub(super) fn call_test_entrypoints_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Vec<CallTestEntrypointInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => Ok(rag
            .exact_call_test_entrypoints_for_node(node_id)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect test entrypoints for code item {node_id}: {err}"
                )))
            })?
            .unwrap_or_default()),
        _ => Ok(Vec::new()),
    }
}

pub(super) fn call_test_selection_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Option<CallTestSelectionInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => rag
            .exact_call_test_selection_for_target(node_id, TOOL_CALL_PATH_OPTIONS)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect test selection for code item {node_id}: {err}"
                )))
            }),
        _ => Ok(None),
    }
}

pub(super) fn call_effect_policy_violations_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
    allowed_effects: &[String],
) -> Result<Vec<CallEffectPolicyViolationInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => {
            let result = if allowed_effects.is_empty() {
                rag.exact_call_effect_policy_violations_for_stored_owner_policy(
                    node_id,
                    TOOL_CALL_PATH_OPTIONS,
                )
            } else {
                rag.exact_call_effect_policy_violations_for_owner(
                    node_id,
                    TOOL_CALL_PATH_OPTIONS,
                    allowed_effects,
                )
            };

            match result {
                Ok(rows) => Ok(rows.unwrap_or_default()),
                Err(ploke_rag::RagError::Db(ploke_db::DbError::Cozo(message)))
                    if message.contains("no admitted effect_policy proof row") =>
                {
                    Ok(Vec::new())
                }
                Err(err) => Err(ploke_error::Error::Internal(InternalError::CompilerError(
                    format!(
                        "failed to collect effect policy violations for code item {node_id}: {err}"
                    ),
                ))),
            }
        }
        _ => Ok(Vec::new()),
    }
}

pub(super) fn call_proof_invariant_findings_for_node(
    ctx: &super::Ctx,
    node_id: Uuid,
) -> Result<Vec<CallProofInvariantFindingInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    match ctx.state.rag.as_ref() {
        Some(rag) if !rag.call_context_degraded() => Ok(rag
            .exact_call_proof_invariant_findings_for_owner(node_id, TOOL_CALL_PATH_OPTIONS)
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "failed to collect proof invariant findings for code item {node_id}: {err}"
                )))
            })?
            .unwrap_or_default()),
        _ => Ok(Vec::new()),
    }
}

pub(super) fn with_call_usage_fields(
    payload: super::ToolUiPayload,
    impact: Option<&CallImpactInfo>,
    reach: Option<&CallReachInfo>,
    reach_effects: &[CallReachEffectInfo],
    policy_violations: &[CallEffectPolicyViolationInfo],
    invariant_findings: &[CallProofInvariantFindingInfo],
    summary_needs: &[ExternalSummaryNeedInfo],
    runtime_needs: &[RuntimeDispatchNeedInfo],
    local_bindings: &[LocalBindingInfo],
    local_binding_edges: &[LocalBindingEdgeInfo],
    awaited_sites: &[AwaitedCallSiteInfo],
    returned_flows: &[ReturnedCallBindingFlowInfo],
    future_flows: &[ReturnedFutureFlowInfo],
    future_execution_flows: &[ReturnedFutureExecutionFlowInfo],
    module_boundary_edges: &[ModuleBoundaryEdgeInfo],
    crate_boundary_edges: &[CrateBoundaryEdgeInfo],
    build_domains: &[CallBuildDomainInfo],
    test_entrypoints: &[CallTestEntrypointInfo],
    test_selection: Option<&CallTestSelectionInfo>,
) -> super::ToolUiPayload {
    payload
        .with_field(
            "impact_callers",
            count(impact.map(|info| info.callers.len())),
        )
        .with_field(
            "impact_direct_callers",
            count(impact.map(|info| info.direct_callers.len())),
        )
        .with_field(
            "impact_direct_call_sites",
            count(impact.map(|info| info.direct_call_sites.len())),
        )
        .with_field(
            "impact_callsite_buckets",
            count(impact.map(|info| info.callsite_buckets.len())),
        )
        .with_field(
            "impact_public_callers",
            count(impact.map(|info| info.public_callers.len())),
        )
        .with_field(
            "impact_test_callers",
            count(impact.map(|info| info.test_callers.len())),
        )
        .with_field(
            "impact_non_test_callers",
            count(impact.map(|info| info.non_test_callers.len())),
        )
        .with_field(
            "impact_source_files",
            count(impact.map(|info| info.source_files.len())),
        )
        .with_field(
            "impact_source_crates",
            count(impact.map(|info| info.source_crates.len())),
        )
        .with_field(
            "impact_source_cfgs",
            count(impact.map(|info| info.source_cfgs.len())),
        )
        .with_field(
            "impact_source_modules",
            count(impact.map(|info| info.source_modules.len())),
        )
        .with_field("reach_callees", count(reach.map(|info| info.callees.len())))
        .with_field(
            "reach_direct_callees",
            count(reach.map(|info| info.direct_callees.len())),
        )
        .with_field(
            "reach_direct_call_sites",
            count(reach.map(|info| info.direct_call_sites.len())),
        )
        .with_field(
            "reach_boundary_call_sites",
            count(reach.map(|info| info.boundary_call_sites.len())),
        )
        .with_field(
            "reach_boundary_edges",
            count(reach.map(|info| info.boundary_edges.len())),
        )
        .with_field(
            "reach_public_callees",
            count(reach.map(|info| info.public_callees.len())),
        )
        .with_field(
            "reach_frontier_calls",
            count(reach.map(|info| info.frontier_calls.len())),
        )
        .with_field(
            "reach_external_frontier_calls",
            count(reach.map(|info| info.external_frontier_calls.len())),
        )
        .with_field(
            "reach_unsupported_frontier_calls",
            count(reach.map(|info| info.unsupported_frontier_calls.len())),
        )
        .with_field(
            "reach_unresolved_frontier_calls",
            count(reach.map(|info| info.unresolved_frontier_calls.len())),
        )
        .with_field(
            "reach_ambiguous_frontier_calls",
            count(reach.map(|info| info.ambiguous_frontier_calls.len())),
        )
        .with_field(
            "reach_source_files",
            count(reach.map(|info| info.source_files.len())),
        )
        .with_field(
            "reach_source_crates",
            count(reach.map(|info| info.source_crates.len())),
        )
        .with_field(
            "reach_source_cfgs",
            count(reach.map(|info| info.source_cfgs.len())),
        )
        .with_field(
            "reach_source_modules",
            count(reach.map(|info| info.source_modules.len())),
        )
        .with_field("reach_effects", reach_effects.len().to_string())
        .with_field(
            "effect_policy_violations",
            policy_violations.len().to_string(),
        )
        .with_field(
            "proof_invariant_findings",
            invariant_findings.len().to_string(),
        )
        .with_field("external_summary_needs", summary_needs.len().to_string())
        .with_field("runtime_dispatch_needs", runtime_needs.len().to_string())
        .with_field("local_bindings", local_bindings.len().to_string())
        .with_field("local_binding_edges", local_binding_edges.len().to_string())
        .with_field("awaited_call_sites", awaited_sites.len().to_string())
        .with_field(
            "returned_call_binding_flows",
            returned_flows.len().to_string(),
        )
        .with_field("returned_future_flows", future_flows.len().to_string())
        .with_field(
            "returned_future_execution_flows",
            future_execution_flows.len().to_string(),
        )
        .with_field(
            "module_boundary_edges",
            module_boundary_edges.len().to_string(),
        )
        .with_field(
            "crate_boundary_edges",
            crate_boundary_edges.len().to_string(),
        )
        .with_field("call_build_domains", build_domains.len().to_string())
        .with_field("call_test_entrypoints", test_entrypoints.len().to_string())
        .with_field(
            "call_test_selection_source_tests",
            count(test_selection.map(|info| info.source_test_callers.len())),
        )
        .with_field(
            "call_test_selection_source_paths",
            count(test_selection.map(|info| info.source_test_paths.len())),
        )
        .with_field(
            "call_test_selection_generated_entrypoints",
            count(test_selection.map(|info| info.generated_entrypoints.len())),
        )
        .with_field(
            "call_test_selection_build_domains",
            count(test_selection.map(|info| info.build_domains.len())),
        )
}

fn count(value: Option<usize>) -> String {
    value.unwrap_or_default().to_string()
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
    fn owner_qualifier_accepts_trait_impl_method_owner() {
        let owner =
            normalize_owner_qualifier(Some("Service"), Some("HandlerService"), NodeKind::Method)
                .expect("trait impl qualifier")
                .expect("owner qualifier");
        assert_eq!(
            owner,
            OwnerQualifier::TraitImpl {
                trait_name: "Service".to_string(),
                type_name: "HandlerService".to_string(),
                trait_arg: None,
            }
        );
        assert_eq!(
            owner.message(),
            " and owner_trait Service and owner_type HandlerService"
        );
    }

    #[test]
    fn owner_qualifier_accepts_trait_impl_generic_root() {
        let owner = normalize_owner_qualifier(
            Some("Service<Request>"),
            Some("HandlerService"),
            NodeKind::Method,
        )
        .expect("trait impl qualifier")
        .expect("owner qualifier");
        assert_eq!(
            owner,
            OwnerQualifier::TraitImpl {
                trait_name: "Service".to_string(),
                type_name: "HandlerService".to_string(),
                trait_arg: Some("Request".to_string()),
            }
        );
        assert_eq!(
            owner.message(),
            " and owner_trait Service<Request> and owner_type HandlerService"
        );
    }
}
