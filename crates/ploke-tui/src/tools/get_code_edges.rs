use std::{collections::BTreeMap, ops::Deref, path::Path};

use itertools::Itertools;
use ploke_core::{
    rag_types::{CallPathInfo, CallPathNodeInfo, CanonPath, ConciseContext, NodeFilepath},
    tool_descriptions::ToolDescription,
    tool_types::ToolName,
};
use ploke_db::{
    helpers::{graph_resolve_edges_for_call_body_owner_id, graph_resolve_edges_for_id},
    typed_rows::ResolvedEdgeData,
};
use ploke_error::DomainError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::rag::utils::NodeKind;
use crate::tools::{
    Tool, ToolError, ToolErrorCode, ToolInvocationError, ValidatesAbolutePath, lookup_support,
};

const FILE_DESC: &str = "Absolute or workspace-relative file path.";
const ITEM_NAME: &str = r#"The name of the item being search for, e.g.
if looking for

```rust
fn example_func() {}`,
```

This would be the string: example_func
"#;

lazy_static::lazy_static! {
    static ref ITEM_EDGES_LOOKUP_PARAMETERS: serde_json::Value = serde_json::json!({
        "type": "object",
        "properties": {
            "item_name": { "type": "string", "description": ITEM_NAME },
            "file_path": { "type": "string", "description": FILE_DESC },
            "node_kind": NodeKind::schema_property(),
            "module_path": { "type": "string", "description": lookup_support::MODULE_PATH_DESC },
            "owner_trait": {
                "type": "string",
                "description": lookup_support::OWNER_TRAIT_DESC
            },
            "owner_type": {
                "type": "string",
                "description": lookup_support::OWNER_TYPE_DESC
            },
            "parent_name": {
                "type": "string",
                "description": lookup_support::PARENT_NAME_DESC
            },
            "body_contains": {
                "type": "string",
                "description": lookup_support::BODY_CONTAINS_DESC
            },
            "allowed_effects": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Optional exact effect allowlist. When supplied, node_info includes call_effect_policy_violations for reachable effect_seed rows whose effect_class is not in this list."
            },
        },
        "required": ["item_name", "file_path", "node_kind", "module_path"],
        "additionalProperties": false
    });
}

#[derive(Debug, Clone, Deserialize)]
pub struct EdgesParams<'a> {
    #[serde(borrow)]
    pub item_name: std::borrow::Cow<'a, str>,
    #[serde(borrow)]
    pub file_path: std::borrow::Cow<'a, str>,
    #[serde(default)]
    pub node_kind: std::borrow::Cow<'a, str>, // "error" | "overwrite"
    #[serde(default)]
    pub module_path: std::borrow::Cow<'a, str>,
    #[serde(default, borrow)]
    pub owner_trait: Option<std::borrow::Cow<'a, str>>,
    #[serde(default, borrow)]
    pub owner_type: Option<std::borrow::Cow<'a, str>>,
    #[serde(default, borrow)]
    pub parent_name: Option<std::borrow::Cow<'a, str>>,
    #[serde(default, borrow)]
    pub body_contains: Option<std::borrow::Cow<'a, str>>,
    #[serde(default)]
    pub allowed_effects: Vec<std::borrow::Cow<'a, str>>,
}

impl<'a> ValidatesAbolutePath for EdgesParams<'a> {
    fn get_file_path(&self) -> impl AsRef<std::path::Path> {
        Path::new(self.file_path.as_ref())
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tool_contracts", derive(Deserialize))]
pub struct EdgesParamsOwned {
    pub item_name: String,
    pub file_path: String,
    pub node_kind: String,
    pub module_path: String,
    pub owner_trait: Option<String>,
    pub owner_type: Option<String>,
    pub parent_name: Option<String>,
    pub body_contains: Option<String>,
    pub allowed_effects: Vec<String>,
}

pub struct CodeItemEdges;

impl Tool for CodeItemEdges {
    type Output = NodeEdgeInfo;

    type OwnedParams = EdgesParamsOwned;

    type Params<'de>
        = EdgesParams<'de>
    where
        Self: 'de;

    fn name() -> ploke_core::tool_types::ToolName {
        ToolName::CodeItemEdges
    }

    fn description() -> ToolDescription {
        Self::name().description()
    }

    fn schema() -> &'static serde_json::Value {
        ITEM_EDGES_LOOKUP_PARAMETERS.deref()
    }

    fn adapt_error(err: ToolInvocationError) -> ToolError {
        match err {
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Ui { message },
            )) => ToolError::new(
                ToolName::CodeItemEdges,
                ToolErrorCode::InvalidFormat,
                message,
            )
            .retry_hint(lookup_support::LOOKUP_RETRY_HINT),
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Io { message },
            )) => ToolError::new(ToolName::CodeItemEdges, ToolErrorCode::Io, message)
                .retry_hint(lookup_support::LOOKUP_RETRY_HINT),
            other => other.into_tool_error(ToolName::CodeItemEdges),
        }
    }

    fn build(_ctx: &super::Ctx) -> Self
    where
        Self: Sized,
    {
        Self
    }

    fn into_owned<'de>(params: &Self::Params<'de>) -> Self::OwnedParams {
        Self::OwnedParams {
            file_path: params.file_path.clone().into_owned(),
            item_name: params.item_name.clone().into_owned(),
            node_kind: params.node_kind.clone().into_owned(),
            module_path: params.module_path.clone().into_owned(),
            owner_trait: params.owner_trait.as_ref().map(|value| value.to_string()),
            owner_type: params.owner_type.as_ref().map(|value| value.to_string()),
            parent_name: params.parent_name.as_ref().map(|value| value.to_string()),
            body_contains: params.body_contains.as_ref().map(|value| value.to_string()),
            allowed_effects: params
                .allowed_effects
                .iter()
                .map(|effect| effect.to_string())
                .collect(),
        }
    }

    fn deserialize_params<'a>(json: &'a str) -> Result<Self::Params<'a>, ToolInvocationError> {
        let params: EdgesParams<'a> =
            serde_json::from_str(json).map_err(|source| ToolInvocationError::Deserialize {
                source,
                raw: Some(json.to_string()),
            })?;
        lookup_support::validate_module_path(Self::name(), params.module_path.as_ref())?;
        Ok(params)
    }

    async fn execute<'de>(
        params: Self::Params<'de>,
        ctx: super::Ctx,
    ) -> Result<super::ToolResult, ploke_error::Error> {
        use ploke_error::{DomainError, InternalError};

        ctx.state.is_stale_err().await?;

        // validate inputs and produce helpful error messages to help llm recover.
        check_empty(
            &params.file_path,
            &params.item_name,
            &params.node_kind,
            &params.module_path,
        )?;

        if !params.file_path.ends_with(".rs") {
            return Err(ploke_error::Error::Domain(DomainError::Ui {
                message: r#"File path does not have Rust file extension `.rs`, please provide file path to a `.rs` file."#.to_string(),
            }));
        };

        let node_kind = params.node_kind.as_ref().parse::<NodeKind>().map_err(|_| {
            ploke_error::Error::Domain(DomainError::Ui {
                message: format!(
                    "Invalid node_kind `{}`. Allowed: {}",
                    params.node_kind,
                    NodeKind::allowed_values().join(", ")
                ),
            })
        })?;
        let owner = lookup_support::normalize_owner_qualifier(
            params.owner_trait.as_deref(),
            params.owner_type.as_deref(),
            node_kind,
        )?;
        let parent =
            lookup_support::normalize_parent_name(params.parent_name.as_deref(), node_kind)?;
        let marker =
            lookup_support::normalize_body_contains(params.body_contains.as_deref(), node_kind)?;

        let (primary_root, policy) = ctx
            .state
            .with_system_read(|sys| sys.tool_path_context())
            .await
            .ok_or_else(|| {
                ploke_error::Error::Domain(DomainError::Ui {
                    message:
                        "No workspace is loaded; load a workspace before using get_code_edges."
                            .to_string(),
                })
            })?;

        let abs_path = params
            .validate_to_abs_path(&primary_root, &policy)
            .map_err(|e| {
                ploke_error::Error::Domain(DomainError::Ui {
                    message: format!(
                        r#"The target file could not be found at the resolved absolute path.
Original error message: {e} This indicates an incorrect file path.
Tip: consider using `request_code_context` with the item name, signature, or anticipated contents
for a more fuzzy search."#
                    )
                    .to_string(),
                })
            })?;
        let rel_path = abs_path
            .strip_prefix(&primary_root)
            .map_err(|e| ploke_error::Error::Internal(InternalError::InvalidState("Error stripping relative path from absolute path. This indicates and error with the ploke application itself. Please consider filing an issue at the ploke github.")))?;

        let mod_path: Vec<String> = params
            .module_path
            .split("::")
            .filter(|s| !s.is_empty())
            .map(|s| s.to_owned())
            .collect();

        if mod_path.is_empty() || mod_path.first().map(|s| s.as_str()) != Some("crate") {
            return Err(ploke_error::Error::Domain(DomainError::Ui {
                message: lookup_support::module_path_error_message(params.module_path.as_ref()),
            }));
        }

        let resolved_item = match lookup_support::resolve_exact_item(
            &ctx.state.db,
            node_kind,
            &abs_path,
            &mod_path,
            params.item_name.as_ref(),
            owner.as_ref(),
            parent.as_deref(),
            marker.as_deref(),
        ) {
            Ok(t) if t.len() == 1 => t,
            Ok(t) if t.is_empty() => {
                let hint = node_kind
                    .lookup_hint()
                    .map(|s| format!(" {s}"))
                    .unwrap_or_default();
                return Err(ploke_error::Error::Domain(DomainError::Ui {
                    message: format!(
                        "No code item named `{}` found in {} with module_path {} and node_kind {}{}{}{}.{}",
                        params.item_name,
                        rel_path.display(),
                        params.module_path,
                        node_kind.as_str(),
                        lookup_support::owner_message(owner.as_ref()),
                        lookup_support::parent_message(parent.as_deref()),
                        lookup_support::body_message(marker.as_deref()),
                        hint
                    ),
                }));
            }
            Ok(_) => {
                let err = ploke_error::Error::Internal(InternalError::InvalidState(
                    "Multiple items matched search query. This violates an invariant that all items are unique when searched for using file path, module path, and item name. Please consider filing an issue at the ploke github.",
                ));
                return Err(ploke_error::Error::Domain(DomainError::Ui {
                    message: format!(
                        "Multiple items matched `{}` in {} with module_path {} and node_kind {}{}{}{}; expected a single match. This is an internal error: {}",
                        params.item_name,
                        rel_path.display(),
                        params.module_path,
                        node_kind.as_str(),
                        lookup_support::owner_message(owner.as_ref()),
                        lookup_support::parent_message(parent.as_deref()),
                        lookup_support::body_message(marker.as_deref()),
                        err
                    ),
                }));
            }
            Err(e) => {
                return Err(ploke_error::Error::Internal(InternalError::CompilerError(
                    format!(
                        "Database lookup failed: {e}. This indicates an issue with the ploke application iteself, not an error in the search input. Please consider filing an issue at the ploke github."
                    ),
                )));
            }
        };
        let resolved_item_id = resolved_item[0].id;
        let carriers = lookup_support::context_carriers_for_node(&ctx, resolved_item_id)?;
        let call_paths = lookup_support::call_path_carriers_for_node(&ctx, resolved_item_id)?;
        let call_impact = lookup_support::call_impact_for_node(&ctx, resolved_item_id)?;
        let call_reach = lookup_support::call_reach_for_node(&ctx, resolved_item_id)?;
        let call_reach_effects =
            lookup_support::call_reach_effects_for_node(&ctx, resolved_item_id)?;
        let unsafe_block_calls =
            lookup_support::unsafe_block_calls_for_node(&ctx, resolved_item_id)?;
        let allowed_effects = params
            .allowed_effects
            .iter()
            .map(|effect| effect.to_string())
            .collect::<Vec<_>>();
        let call_effect_policy_violations = lookup_support::call_effect_policy_violations_for_node(
            &ctx,
            resolved_item_id,
            &allowed_effects,
        )?;
        let call_proof_invariant_findings =
            lookup_support::call_proof_invariant_findings_for_node(&ctx, resolved_item_id)?;
        let external_summary_needs =
            lookup_support::external_summary_needs_for_node(&ctx, resolved_item_id)?;
        let runtime_dispatch_needs =
            lookup_support::runtime_dispatch_needs_for_node(&ctx, resolved_item_id)?;
        let local_bindings = lookup_support::local_bindings_for_node(&ctx, resolved_item_id)?;
        let local_binding_edges =
            lookup_support::local_binding_edges_for_node(&ctx, resolved_item_id)?;
        let self_field_parameter_flows =
            lookup_support::self_field_parameter_flows_for_node(&ctx, resolved_item_id)?;
        let awaited_call_sites =
            lookup_support::awaited_call_sites_for_node(&ctx, resolved_item_id)?;
        let returned_call_binding_flows =
            lookup_support::returned_call_binding_flows_for_node(&ctx, resolved_item_id)?;
        let returned_future_flows =
            lookup_support::returned_future_flows_for_node(&ctx, resolved_item_id)?;
        let returned_future_execution_flows =
            lookup_support::returned_future_execution_flows_for_node(&ctx, resolved_item_id)?;
        let module_boundary_edges =
            lookup_support::module_boundary_edges_for_node(&ctx, resolved_item_id)?;
        let crate_boundary_edges =
            lookup_support::crate_boundary_edges_for_node(&ctx, resolved_item_id)?;
        let call_build_domains =
            lookup_support::call_build_domains_for_node(&ctx, resolved_item_id)?;
        let call_test_entrypoints =
            lookup_support::call_test_entrypoints_for_node(&ctx, resolved_item_id)?;
        let call_test_selection =
            lookup_support::call_test_selection_for_node(&ctx, resolved_item_id)?;
        let call_path_nodes =
            call_path_nodes_for_paths(&call_paths.from_owner, &call_paths.to_target);

        let resolved_edges = if node_kind.call_body_owner_kind().is_some() {
            graph_resolve_edges_for_call_body_owner_id(&ctx.state.db, resolved_item_id)?
        } else {
            graph_resolve_edges_for_id(&ctx.state.db, node_kind.as_relation(), resolved_item_id)?
        };
        let tool_results = ctx
            .state
            .io_handle
            .get_snippets_batch(resolved_item)
            .await
            .map_err(|e| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "io channel error: {e}"
                )))
            })?;
        let mut tool_results_iter = tool_results.into_iter();
        let snippet_result = tool_results_iter.next().ok_or_else(|| {
            ploke_error::Error::Internal(InternalError::CompilerError(
                "get_snippets_batch returned no results".to_string(),
            ))
        })?;
        let snippet = snippet_result.map_err(|e| {
            ploke_error::Error::Internal(InternalError::CompilerError(format!(
                "failed to read snippet: {e}"
            )))
        })?;
        let call_graph_summary = call_graph_summary(
            resolved_item_id,
            &call_paths.from_owner,
            &call_paths.to_target,
            &carriers.call_context,
        );
        let concise_context = ConciseContext {
            id: resolved_item_id,
            file_path: NodeFilepath::new(rel_path.display().to_string()),
            canon_path: CanonPath::new(lookup_support::item_canon_path(
                params.module_path.as_ref(),
                params.item_name.as_ref(),
            )),
            snippet,
            type_context: None,
            call_expansion: None,
            call_context: carriers.call_context,
            call_paths_from_owner: Vec::new(),
            call_paths_to_target: Vec::new(),
            call_cycles_from_owner: Vec::new(),
            call_impact,
            call_reach,
            call_reach_effects,
            unsafe_block_calls,
            call_effect_policy_violations,
            call_proof_invariant_findings,
            external_summary_needs,
            runtime_dispatch_needs,
            local_bindings,
            local_binding_edges,
            self_field_parameter_flows,
            awaited_call_sites,
            returned_call_binding_flows,
            returned_future_flows,
            returned_future_execution_flows,
            module_boundary_edges,
            crate_boundary_edges,
            call_build_domains,
            call_test_entrypoints,
            call_test_selection,
            proof_context: carriers.proof_context,
        };

        let node_edge_info = NodeEdgeInfo {
            node_info: concise_context,
            edge_info: resolved_edges,
            call_paths_from_owner: call_paths.from_owner,
            call_paths_to_target: call_paths.to_target,
            call_cycles_from_owner: call_paths.cycles_from_owner,
            call_path_nodes,
            call_graph_summary,
        };
        let call_counts = lookup_support::call_context_counts(
            resolved_item_id,
            &node_edge_info.node_info.call_context,
        );

        let summary = format!("Resolved {} edges", node_edge_info.edge_info.len());
        let ui_payload = super::ToolUiPayload::new(Self::name(), ctx.call_id.clone(), summary)
            .with_field("file_path", node_edge_info.node_info.file_path.as_ref())
            .with_field("canon_path", node_edge_info.node_info.canon_path.as_ref())
            .with_field("edges", node_edge_info.edge_info.len().to_string())
            .with_field("call_context", call_counts.total.to_string())
            .with_field("call_context_outgoing", call_counts.outgoing.to_string())
            .with_field("call_context_incoming", call_counts.incoming.to_string())
            .with_field(
                "callers",
                node_edge_info.call_graph_summary.callers.to_string(),
            )
            .with_field(
                "callees",
                node_edge_info.call_graph_summary.callees.to_string(),
            )
            .with_field(
                "blocked_calls",
                node_edge_info.call_graph_summary.blocked.to_string(),
            )
            .with_field(
                "call_paths_from_owner",
                node_edge_info.call_paths_from_owner.len().to_string(),
            )
            .with_field(
                "call_paths_to_target",
                node_edge_info.call_paths_to_target.len().to_string(),
            )
            .with_field(
                "call_cycles_from_owner",
                node_edge_info.call_cycles_from_owner.len().to_string(),
            );
        let ui_payload = lookup_support::with_call_usage_fields(
            ui_payload,
            node_edge_info.node_info.call_impact.as_ref(),
            node_edge_info.node_info.call_reach.as_ref(),
            &node_edge_info.node_info.call_reach_effects,
            &node_edge_info.node_info.unsafe_block_calls,
            &node_edge_info.node_info.call_effect_policy_violations,
            &node_edge_info.node_info.call_proof_invariant_findings,
            &node_edge_info.node_info.external_summary_needs,
            &node_edge_info.node_info.runtime_dispatch_needs,
            &node_edge_info.node_info.local_bindings,
            &node_edge_info.node_info.local_binding_edges,
            &node_edge_info.node_info.self_field_parameter_flows,
            &node_edge_info.node_info.awaited_call_sites,
            &node_edge_info.node_info.returned_call_binding_flows,
            &node_edge_info.node_info.returned_future_flows,
            &node_edge_info.node_info.returned_future_execution_flows,
            &node_edge_info.node_info.module_boundary_edges,
            &node_edge_info.node_info.crate_boundary_edges,
            &node_edge_info.node_info.call_build_domains,
            &node_edge_info.node_info.call_test_entrypoints,
            node_edge_info.node_info.call_test_selection.as_ref(),
        )
        .with_field(
            "proof_context",
            node_edge_info.node_info.proof_context.len().to_string(),
        );
        let content = serde_json::to_string(&node_edge_info).map_err(|err| {
            ploke_error::Error::Internal(InternalError::CompilerError(format!(
                "failed to serialize NodeEdgeInfo: {err}. This indicates an error in the ploke application itself, not due to incorrect search terms. Please consider filing an issue on the ploke github."
            )))
        })?;

        Ok(super::ToolResult {
            content,
            ui_payload: Some(ui_payload),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeEdgeInfo {
    node_info: ConciseContext,
    edge_info: Vec<ResolvedEdgeData>,
    call_paths_from_owner: Vec<CallPathInfo>,
    call_paths_to_target: Vec<CallPathInfo>,
    call_cycles_from_owner: Vec<CallPathInfo>,
    call_path_nodes: Vec<CallPathNodeInfo>,
    call_graph_summary: CallGraphSummary,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CallGraphSummary {
    calls: usize,
    callers: usize,
    callees: usize,
    blocked: usize,
    outgoing_paths: usize,
    incoming_paths: usize,
    outgoing_depth: u32,
    incoming_depth: u32,
    path_nodes: usize,
}

fn call_graph_summary(
    node_id: Uuid,
    from_owner: &[CallPathInfo],
    to_target: &[CallPathInfo],
    call_context: &[ploke_core::rag_types::CallContextInfo],
) -> CallGraphSummary {
    let counts = lookup_support::call_context_counts(node_id, call_context);
    let calls = counts.outgoing;
    let callers = counts.incoming;
    let callees = call_context
        .iter()
        .filter(|call| call.owner_id == node_id)
        .map(|call| call.targets.len())
        .sum();
    let blocked = call_context
        .iter()
        .filter(|call| call.owner_id == node_id && call.targets.is_empty())
        .count();
    let outgoing_depth = from_owner.iter().map(|path| path.depth).max().unwrap_or(0);
    let incoming_depth = to_target.iter().map(|path| path.depth).max().unwrap_or(0);
    let path_nodes = call_path_nodes_for_paths(from_owner, to_target).len();

    CallGraphSummary {
        calls,
        callers,
        callees,
        blocked,
        outgoing_paths: from_owner.len(),
        incoming_paths: to_target.len(),
        outgoing_depth,
        incoming_depth,
        path_nodes,
    }
}

fn call_path_nodes_for_paths(
    from_owner: &[CallPathInfo],
    to_target: &[CallPathInfo],
) -> Vec<CallPathNodeInfo> {
    let mut nodes = BTreeMap::new();
    for path in from_owner.iter().chain(to_target) {
        for node in &path.nodes {
            nodes.entry(node.id).or_insert_with(|| node.clone());
        }
    }
    nodes.into_values().collect()
}

fn check_empty(
    file_path: &str,
    item_name: &str,
    node_kind: &str,
    module_path: &str,
) -> Result<(), ploke_error::Error> {
    struct MissingFieldInfo {
        missing_field: &'static str,
        help_msg: &'static str,
    }

    let info: Option<MissingFieldInfo> = if file_path.trim().is_empty() {
        Some(MissingFieldInfo {
            missing_field: "file_path",
            help_msg: "Tip: if the file_path is unknown, try using `request_code_context` to search for the item",
        })
    } else if item_name.trim().is_empty() {
        Some(MissingFieldInfo {
            missing_field: "item_name",
            help_msg: "Tip: if the item_name is unknown, try using `request_code_context` to search for the item",
        })
    } else if node_kind.trim().is_empty() {
        Some(MissingFieldInfo {
            missing_field: "node_kind",
            help_msg: "Tip: if the node_kind is unknown, try using `request_code_context` to search for the item",
        })
    } else if module_path.trim().is_empty() {
        Some(MissingFieldInfo {
            missing_field: "module_path",
            help_msg: "Tip: if the module_path is unknown, try using the `show_module_tree` tool",
        })
    } else {
        None
    };
    if let Some(missing) = info {
        let missing_field_msg = format!(
            "No field `{}` provided, must provide required field. {}",
            missing.missing_field, missing.help_msg
        );
        return Err(ploke_error::Error::Domain(DomainError::Ui {
            message: missing_field_msg,
        }));
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;

    #[test]
    fn schema_includes_method_node_kind() {
        let schema = <CodeItemEdges as Tool>::schema();
        let node_kind = schema
            .get("properties")
            .and_then(|p| p.get("node_kind"))
            .and_then(|nk| nk.as_object())
            .expect("node_kind schema");
        let enum_vals = node_kind
            .get("enum")
            .and_then(|v| v.as_array())
            .expect("enum values");
        let enum_vals: Vec<&str> = enum_vals
            .iter()
            .map(|v| v.as_str().expect("string enum value"))
            .collect();
        assert!(enum_vals.contains(&"method"));
        assert_eq!(enum_vals.first().copied(), Some("function"));
    }

    #[test]
    fn invalid_module_path_fails_during_preflight_validation() {
        let args = r#"{"file_path":"proc_macros/ploke-db-derive/src/lib.rs","item_name":"FieldSpec","module_path":"ploke_db_derive","node_kind":"struct"}"#;
        let err = <CodeItemEdges as Tool>::deserialize_params(args)
            .expect_err("package-name module_path should fail preflight");
        let ToolInvocationError::Validation(err) = err else {
            panic!("expected validation error");
        };

        assert_eq!(err.code, ToolErrorCode::InvalidFormat);
        assert_eq!(err.field, Some("module_path"));
        assert_eq!(
            err.expected.as_deref(),
            Some(lookup_support::MODULE_PATH_EXPECTED)
        );
        assert_eq!(err.received.as_deref(), Some("ploke_db_derive"));
        assert!(
            err.retry_hint
                .as_deref()
                .expect("retry hint")
                .contains("request_code_context")
        );
    }
}
