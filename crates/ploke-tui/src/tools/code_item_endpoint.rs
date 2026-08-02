use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::tools::lookup_support;

#[derive(Debug, Clone, Deserialize)]
pub struct CodeItemEndpoint<'a> {
    #[serde(borrow)]
    pub item_name: Cow<'a, str>,
    #[serde(borrow)]
    pub file_path: Cow<'a, str>,
    #[serde(borrow)]
    pub node_kind: Cow<'a, str>,
    #[serde(borrow)]
    pub module_path: Cow<'a, str>,
    #[serde(default, borrow)]
    pub owner_trait: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub owner_type: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub parent_name: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub body_contains: Option<Cow<'a, str>>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tool_contracts", derive(Deserialize))]
pub struct CodeItemEndpointOwned {
    pub item_name: String,
    pub file_path: String,
    pub node_kind: String,
    pub module_path: String,
    pub owner_trait: Option<String>,
    pub owner_type: Option<String>,
    pub parent_name: Option<String>,
    pub body_contains: Option<String>,
}

pub(crate) fn schema_property() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "item_name": { "type": "string" },
            "file_path": { "type": "string" },
            "node_kind": crate::rag::utils::NodeKind::schema_property(),
            "module_path": {
                "type": "string",
                "description": lookup_support::MODULE_PATH_DESC
            },
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
            }
        },
        "required": ["item_name", "file_path", "node_kind", "module_path"],
        "additionalProperties": false
    })
}

pub(crate) fn endpoint_to_owned(endpoint: &CodeItemEndpoint<'_>) -> CodeItemEndpointOwned {
    CodeItemEndpointOwned {
        item_name: endpoint.item_name.clone().into_owned(),
        file_path: endpoint.file_path.clone().into_owned(),
        node_kind: endpoint.node_kind.clone().into_owned(),
        module_path: endpoint.module_path.clone().into_owned(),
        owner_trait: endpoint.owner_trait.as_ref().map(ToString::to_string),
        owner_type: endpoint.owner_type.as_ref().map(ToString::to_string),
        parent_name: endpoint.parent_name.as_ref().map(ToString::to_string),
        body_contains: endpoint.body_contains.as_ref().map(ToString::to_string),
    }
}

pub(crate) fn validate_endpoint(
    label: &str,
    endpoint: &CodeItemEndpoint<'_>,
) -> Result<(), ploke_error::Error> {
    for (field, value) in [
        ("item_name", endpoint.item_name.as_ref()),
        ("file_path", endpoint.file_path.as_ref()),
        ("node_kind", endpoint.node_kind.as_ref()),
        ("module_path", endpoint.module_path.as_ref()),
    ] {
        if value.trim().is_empty() {
            return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
                message: format!("{label}.{field} must not be empty."),
            }));
        }
    }
    Ok(())
}

pub(crate) fn resolve_endpoint(
    ctx: &super::Ctx,
    primary_root: &std::path::Path,
    policy: &ploke_io::path_policy::PathPolicy,
    endpoint: &CodeItemEndpoint<'_>,
) -> Result<lookup_support::ResolvedToolItem, ploke_error::Error> {
    lookup_support::resolve_exact_tool_item(
        &ctx.state.db,
        primary_root,
        policy,
        lookup_support::ExactItemRequest {
            item_name: endpoint.item_name.as_ref(),
            file_path: endpoint.file_path.as_ref(),
            node_kind: endpoint.node_kind.as_ref(),
            module_path: endpoint.module_path.as_ref(),
            owner_trait: endpoint.owner_trait.as_deref(),
            owner_type: endpoint.owner_type.as_deref(),
            parent_name: endpoint.parent_name.as_deref(),
            body_contains: endpoint.body_contains.as_deref(),
        },
    )
}
