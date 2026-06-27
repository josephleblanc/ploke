use ploke_core::{
    rag_types::{CallContextInfo, ProofContextInfo},
    tool_types::ToolName,
};
use uuid::Uuid;

use super::{ToolError, ToolErrorCode, ToolInvocationError, ToolRetryContext};

pub(super) const MODULE_PATH_DESC: &str = r#"crate-relative module path, e.g. "crate" or "crate::mod_one::nested_mod".
Do not use the Cargo package/crate name, and do not include the target item's identifier.
If the module path is unknown, use request_code_context or read_file before exact lookup."#;

pub(super) const MODULE_PATH_EXPECTED: &str =
    "crate or crate::module::submodule, without the item name";

pub(super) const LOOKUP_RETRY_HINT: &str = "Use a crate-relative module_path that begins with `crate`. If the exact module path is uncertain, call request_code_context with the item name/signature or read_file on the target file before retrying exact lookup.";

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

pub(super) struct ContextCarriers {
    pub(super) call_context: Vec<CallContextInfo>,
    pub(super) proof_context: Vec<ProofContextInfo>,
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
}
