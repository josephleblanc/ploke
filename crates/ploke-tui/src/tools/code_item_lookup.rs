use std::{ops::Deref, path::Path};

use ploke_core::{
    rag_types::{CanonPath, ConciseContext, NodeFilepath},
    tool_types::{ToolDescr, ToolName},
};
use ploke_db::helpers::graph_resolve_exact;
use ploke_error::DomainError;
use serde::{Deserialize, Serialize};

use crate::tools::{Tool, ValidatesAbolutePath};

const FILE_DESC: &str = "Absolute or workspace-relative file path.";
const MODULE_PATH: &str =
    r#"canonical module path segments, including leading "crate", e.g. ["crate","mod","submod"]"#;
const NODE_KIND: &str = r#"The kind of code item this is. Must be one of:
- function
- const
- enum
- impl
- import (e.g. use statement)
- macro
- module
- static
- struct
- trait
- type_alias
- union"#;
const ITEM_NAME: &str = r#"The name of the item being search for, e.g. 
if looking for

```rust
fn example_func() {}`, 
```

This would be the string: example_func
"#;

lazy_static::lazy_static! {
    static ref CODE_ITEM_LOOKUP_PARAMETERS: serde_json::Value = serde_json::json!({
        "type": "object",
        "properties": {
            "item_name": { "type": "string", "description": ITEM_NAME },
            "file_path": { "type": "string", "description": FILE_DESC },
            "node_kind": { "type": "string", "description": NODE_KIND },
            "module_path": {
                "type": "array",
                "items": {
                    "type": "string",
                    "diff": { "type": "string", "description": MODULE_PATH },
                }
            }
        },
        "required": ["item_name", "file_path", "node_kind", "module_path"],
        "additionalProperties": false
    });
}

#[derive(Debug, Clone, Deserialize)]
pub struct LookupParams<'a> {
    #[serde(borrow)]
    pub item_name: std::borrow::Cow<'a, str>,
    #[serde(borrow)]
    pub file_path: std::borrow::Cow<'a, str>,
    #[serde(default)]
    pub node_kind: std::borrow::Cow<'a, str>, // "error" | "overwrite"
    #[serde(default)]
    pub module_path: Vec<std::borrow::Cow<'a, str>>,
}

impl<'a> ValidatesAbolutePath for LookupParams<'a> {
    fn get_file_path(&self) -> impl AsRef<std::path::Path> {
        Path::new(self.file_path.as_ref())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LookupParamsOwned {
    pub item_name: String,
    pub file_path: String,
    pub node_kind: String,
    pub module_path: Vec<String>,
}

pub struct CodeItemLookup;

impl Tool for CodeItemLookup {
    type Output = ConciseContext;

    type OwnedParams = LookupParamsOwned;

    type Params<'de>
        = LookupParams<'de>
    where
        Self: 'de;

    fn name() -> ploke_core::tool_types::ToolName {
        ToolName::CodeItemLookup
    }

    fn description() -> ploke_core::tool_types::ToolDescr {
        ToolDescr::CodeItemLookup
    }

    fn schema() -> &'static serde_json::Value {
        CODE_ITEM_LOOKUP_PARAMETERS.deref()
    }

    fn build(_ctx: &super::Ctx) -> Self
    where
        Self: Sized,
    {
        Self
    }

    fn into_owned<'de>(params: &Self::Params<'de>) -> Self::OwnedParams {
        let module_path: Vec<String> = params
            .module_path
            .iter()
            .map(|s| s.to_owned().to_string())
            .collect();
        Self::OwnedParams {
            file_path: params.file_path.clone().into_owned(),
            item_name: params.item_name.clone().into_owned(),
            node_kind: params.node_kind.clone().into_owned(),
            module_path,
        }
    }

    async fn execute<'de>(
        params: Self::Params<'de>,
        ctx: super::Ctx,
    ) -> Result<super::ToolResult, ploke_error::Error> {
        use ploke_error::{DomainError, InternalError};

        // validate inputs and produce helpful error messages to help llm recover.
        check_empty(
            params.file_path.clone(),
            params.item_name.clone(),
            params.node_kind.clone(),
            &params.module_path,
        )?;

        if !params.file_path.ends_with(".rs") {
            return Err(ploke_error::Error::Domain(DomainError::Ui {
                message: r#"File path does not have Rust file extension `.rs`, please provide file path to a `.rs` file."#.to_string(),
            }));
        };

        let allowed_kinds = [
            "function",
            "const",
            "enum",
            "impl",
            "import",
            "macro",
            "module",
            "static",
            "struct",
            "trait",
            "type_alias",
            "union",
        ];
        if !allowed_kinds.contains(&params.node_kind.as_ref()) {
            return Err(ploke_error::Error::Domain(DomainError::Ui {
                message: format!(
                    "Invalid node_kind `{}`. Allowed: {}",
                    params.node_kind,
                    allowed_kinds.join(", ")
                ),
            }));
        }

        let crate_root = ctx
            .state
            .system
            .read()
            .await
            .crate_focus
            .clone()
            .ok_or_else(|| {
                ploke_error::Error::Domain(DomainError::Ui {
                    message:
                        "No crate is currently focused; load a workspace before using code_item_lookup."
                            .to_string(),
                })
            })?;

        let abs_path = params.validate_to_abs_path(crate_root)?;

        let mod_path: Vec<String> = params.module_path.iter().map(|s| s.to_string()).collect();

        if mod_path.is_empty() || mod_path.first().map(|s| s.as_str()) != Some("crate") {
            return Err(ploke_error::Error::Domain(DomainError::Ui {
                message: r#"module_path must start with "crate", e.g. crate::module::submodule"#
                    .to_string(),
            }));
        }

        let resolved_item = match graph_resolve_exact(
            &ctx.state.db,
            params.node_kind.as_ref(),
            &abs_path,
            &mod_path,
            params.item_name.as_ref(),
        ) {
            Ok(t) if t.len() == 1 => t,
            Ok(t) if t.is_empty() => {
                return Err(ploke_error::Error::Domain(DomainError::Ui {
                    message: format!(
                        "No code item named `{}` found in {} with module_path {} and node_kind {}",
                        params.item_name,
                        abs_path.display(),
                        params.module_path.join("::"),
                        params.node_kind
                    ),
                }));
            }
            Ok(_) => {
                return Err(ploke_error::Error::Domain(DomainError::Ui {
                    message: format!(
                        "Multiple items matched `{}` in {} with module_path {} and node_kind {}; expected a single match.",
                        params.item_name,
                        abs_path.display(),
                        params.module_path.join("::"),
                        params.node_kind
                    ),
                }));
            }
            Err(e) => {
                return Err(ploke_error::Error::Internal(InternalError::CompilerError(
                    format!("Database lookup failed: {e}"),
                )));
            }
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
        let concise_context = ConciseContext {
            file_path: NodeFilepath::new(abs_path.display().to_string()),
            canon_path: CanonPath::new(params.module_path.join("::")),
            snippet,
        };

        let content = serde_json::to_string(&concise_context).map_err(|err| {
            ploke_error::Error::Internal(InternalError::CompilerError(format!(
                "failed to serialize ConciseContext: {err}"
            )))
        })?;

        Ok(super::ToolResult { content })
    }
}

fn check_empty(
    file_path: std::borrow::Cow<'_, str>,
    item_name: std::borrow::Cow<'_, str>,
    node_kind: std::borrow::Cow<'_, str>,
    module_path: &Vec<std::borrow::Cow<'_, str>>,
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
    } else if module_path.iter().any(|s| s.trim().is_empty()) {
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
