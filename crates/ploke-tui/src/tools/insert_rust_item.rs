use std::{borrow::Cow, ops::Deref as _, path::Path, path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::json;
use syn::{Item, ItemImpl, Type, spanned::Spanned};

use crate::{
    EventBus,
    rag::{
        tools::stage_semantic_edit_proposal,
        utils::{ApplyCodeEditRequest, NodeKind, ToolCallParams},
    },
    tools::{
        ToolError, ToolErrorCode, ToolInvocationError, ToolName, ToolResult, tool_ui_error,
        validators::validate_file_path_basic,
    },
    utils::path_scoping,
};
use ploke_core::{
    PROJECT_NAMESPACE_UUID, TrackingHash, WriteSnippetData, rag_types::ApplyCodeEditResult,
};

use super::{Ctx, Tool};

pub struct InsertRustItem;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InsertRustContainerKind {
    File,
    Module,
    Trait,
    Impl,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InsertRustItemParams<'a> {
    #[serde(borrow)]
    pub file: Cow<'a, str>,
    pub container_kind: InsertRustContainerKind,
    #[serde(borrow, default)]
    pub container_canon: Option<Cow<'a, str>>,
    pub item_kind: NodeKind,
    #[serde(borrow)]
    pub code: Cow<'a, str>,
    #[serde(default)]
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InsertRustItemParamsOwned {
    pub file: String,
    pub container_kind: InsertRustContainerKind,
    pub container_canon: Option<String>,
    pub item_kind: NodeKind,
    pub code: String,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
struct ContainerSpan {
    start: usize,
    end: usize,
}

impl Tool for InsertRustItem {
    type Output = ApplyCodeEditResult;
    type OwnedParams = InsertRustItemParamsOwned;
    type Params<'de> = InsertRustItemParams<'de>;

    fn name() -> ToolName {
        ToolName::InsertRustItem
    }

    fn description() -> super::ToolDescr {
        super::ToolDescr::InsertRustItem
    }

    fn schema() -> &'static serde_json::Value {
        INSERT_RUST_ITEM_PARAMETERS.deref()
    }

    fn adapt_error(err: ToolInvocationError) -> ToolError {
        let hint = "Use a workspace-root-relative Rust path and, for module/trait/impl insertion, provide the canonical container path.";
        match err {
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Io { message },
            )) => ToolError::new(ToolName::InsertRustItem, ToolErrorCode::Io, message)
                .retry_hint(hint),
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Ui { message },
            )) => ToolError::new(
                ToolName::InsertRustItem,
                ToolErrorCode::InvalidFormat,
                message,
            )
            .retry_hint(hint),
            other => other.into_tool_error(ToolName::InsertRustItem),
        }
    }

    fn build(_ctx: &Ctx) -> Self {
        Self
    }

    fn deserialize_params<'a>(json: &'a str) -> Result<Self::Params<'a>, ToolInvocationError> {
        let params: InsertRustItemParams<'a> =
            serde_json::from_str(json).map_err(|source| ToolInvocationError::Deserialize {
                source,
                raw: Some(json.to_string()),
            })?;
        validate_params(&params)?;
        Ok(params)
    }

    fn into_owned<'a>(params: &Self::Params<'a>) -> Self::OwnedParams {
        InsertRustItemParamsOwned {
            file: params.file.clone().into_owned(),
            container_kind: params.container_kind,
            container_canon: params
                .container_canon
                .as_ref()
                .map(|canon| canon.clone().into_owned()),
            item_kind: params.item_kind,
            code: params.code.clone().into_owned(),
            confidence: params.confidence,
        }
    }

    async fn execute<'a>(
        params: Self::Params<'a>,
        ctx: Ctx,
    ) -> Result<ToolResult, ploke_error::Error> {
        let owned = Self::into_owned(&params);
        let (primary_root, policy) = ctx
            .state
            .with_system_read(|sys| {
                sys.tool_path_context()
                    .map(|(root, policy)| (root.clone(), policy.clone()))
            })
            .await
            .ok_or_else(|| {
                ploke_error::Error::Domain(ploke_error::DomainError::Ui {
                    message:
                        "No workspace is loaded; load a workspace before using insert_rust_item."
                            .to_string(),
                })
            })?;

        let requested_path = PathBuf::from(&owned.file);
        let abs_path =
            path_scoping::resolve_tool_path(requested_path.as_path(), &primary_root, &policy)
                .map_err(|err| {
                    ploke_error::Error::Domain(ploke_error::DomainError::Io {
                        message: format!("invalid path: {err}"),
                    })
                })?;

        if abs_path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
                message: "insert_rust_item only supports Rust source files (*.rs).".to_string(),
            }));
        }

        let planned_edit = plan_insert_edit(&abs_path, &owned).await?;
        let stage_ctx = ToolCallParams {
            state: Arc::clone(&ctx.state),
            event_bus: Arc::clone(&ctx.event_bus),
            request_id: ctx.request_id,
            parent_id: ctx.parent_id,
            name: Self::name(),
            typed_req: ApplyCodeEditRequest {
                edits: Vec::new(),
                confidence: owned.confidence,
            },
            call_id: ctx.call_id.clone(),
        };
        let proposal_id = stage_semantic_edit_proposal(stage_ctx, vec![planned_edit]).await;
        crate::tools::code_edit::print_code_edit_results(
            &ctx,
            proposal_id,
            ctx.request_id,
            Self::name(),
        )
        .await
    }
}

async fn plan_insert_edit(
    abs_path: &Path,
    params: &InsertRustItemParamsOwned,
) -> Result<WriteSnippetData, ploke_error::Error> {
    let file_data = ploke_io::read::read_and_compute_filehash(abs_path, PROJECT_NAMESPACE_UUID)
        .await
        .map_err(map_insert_planning_error)?;
    let content = file_data.contents;
    let expected_file_hash = file_data.hash;

    let (start_byte, replacement) = match params.container_kind {
        InsertRustContainerKind::File => plan_file_scope_insert(&content, &params.code),
        InsertRustContainerKind::Module => {
            let canon = required_container_canon(params)?;
            let segments = normalize_canon_segments(canon)?;
            let file = syn::parse_file(&content).map_err(|err| {
                ploke_error::Error::Domain(ploke_error::DomainError::Ui {
                    message: format!(
                        "failed to parse Rust file {}: {err}. If the file does not parse yet, use non_semantic_patch.",
                        abs_path.display()
                    ),
                })
            })?;
            let target_modules = segments.iter().skip(1).cloned().collect::<Vec<_>>();
            let module =
                find_inline_module_span(&file.items, &target_modules).ok_or_else(|| {
                    ploke_error::Error::Domain(ploke_error::DomainError::Ui {
                        message: format!(
                            "No inline module container found for {} in {}",
                            canon,
                            abs_path.display()
                        ),
                    })
                })?;
            plan_braced_container_insert(&content, module, &params.code)?
        }
        InsertRustContainerKind::Trait => {
            let canon = required_container_canon(params)?;
            let segments = normalize_canon_segments(canon)?;
            let file = syn::parse_file(&content).map_err(|err| {
                ploke_error::Error::Domain(ploke_error::DomainError::Ui {
                    message: format!(
                        "failed to parse Rust file {}: {err}. If the file does not parse yet, use non_semantic_patch.",
                        abs_path.display()
                    ),
                })
            })?;
            let (module_path, trait_name) = split_primary_canon_segments(&segments)?;
            let matches = collect_trait_spans(&file.items, &module_path, trait_name);
            let span = single_container_match("trait", canon, abs_path, matches)?;
            plan_braced_container_insert(&content, span, &params.code)?
        }
        InsertRustContainerKind::Impl => {
            let canon = required_container_canon(params)?;
            let segments = normalize_canon_segments(canon)?;
            let file = syn::parse_file(&content).map_err(|err| {
                ploke_error::Error::Domain(ploke_error::DomainError::Ui {
                    message: format!(
                        "failed to parse Rust file {}: {err}. If the file does not parse yet, use non_semantic_patch.",
                        abs_path.display()
                    ),
                })
            })?;
            let (module_path, owner_name) = split_primary_canon_segments(&segments)?;
            let matches = collect_inherent_impl_spans(&file.items, &module_path, owner_name);
            let span = single_container_match("inherent impl", canon, abs_path, matches)?;
            plan_braced_container_insert(&content, span, &params.code)?
        }
    };

    Ok(WriteSnippetData {
        id: uuid::Uuid::new_v4(),
        name: format!("insert_{}", params.item_kind.as_str()),
        file_path: abs_path.to_path_buf(),
        expected_file_hash,
        start_byte,
        end_byte: start_byte,
        replacement,
        namespace: PROJECT_NAMESPACE_UUID,
    })
}

fn validate_params(params: &InsertRustItemParams<'_>) -> Result<(), ToolInvocationError> {
    validate_file_path_basic(
        ToolName::InsertRustItem,
        "file",
        params.file.as_ref(),
        false,
    )
    .map_err(|err| ToolInvocationError::Exec(tool_ui_error(err.message.clone())))?;

    if params.code.trim().is_empty() {
        return Err(ToolInvocationError::Exec(tool_ui_error(
            "insert_rust_item requires non-empty `code`.".to_string(),
        )));
    }

    if !matches!(params.item_kind, NodeKind::Import | NodeKind::Module)
        && params.code.contains('\0')
    {
        return Err(ToolInvocationError::Exec(tool_ui_error(
            "insert_rust_item code must be valid UTF-8 text.".to_string(),
        )));
    }

    if !matches!(params.container_kind, InsertRustContainerKind::File)
        && params
            .container_canon
            .as_deref()
            .map(|canon| canon.trim().is_empty())
            .unwrap_or(true)
    {
        return Err(ToolInvocationError::Exec(tool_ui_error(
            "container_canon is required when container_kind is module, trait, or impl."
                .to_string(),
        )));
    }

    Ok(())
}

fn required_container_canon(
    params: &InsertRustItemParamsOwned,
) -> Result<&str, ploke_error::Error> {
    params
        .container_canon
        .as_deref()
        .filter(|canon| !canon.trim().is_empty())
        .ok_or_else(|| {
            ploke_error::Error::Domain(ploke_error::DomainError::Ui {
                message:
                    "container_canon is required when container_kind is module, trait, or impl."
                        .to_string(),
            })
        })
}

fn normalize_canon_segments(canon: &str) -> Result<Vec<String>, ploke_error::Error> {
    let canon = canon.trim();
    if canon.is_empty() {
        return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
            message: "container_canon cannot be empty.".to_string(),
        }));
    }
    let mut segments = canon
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if segments.first().map(String::as_str) != Some("crate") {
        segments.insert(0, "crate".to_string());
    }
    Ok(segments)
}

fn split_primary_canon_segments(
    segments: &[String],
) -> Result<(&[String], &str), ploke_error::Error> {
    if segments.len() < 2 {
        return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
            message: "container_canon must include an item name.".to_string(),
        }));
    }
    let owner_name = segments.last().expect("validated non-empty");
    Ok((&segments[1..segments.len() - 1], owner_name.as_str()))
}

fn find_inline_module_span(items: &[Item], target_modules: &[String]) -> Option<ContainerSpan> {
    if target_modules.is_empty() {
        return None;
    }

    for item in items {
        if let Item::Mod(item_mod) = item {
            if item_mod.ident != target_modules[0] {
                continue;
            }
            if target_modules.len() == 1 {
                return item_mod
                    .content
                    .as_ref()
                    .map(|_| span_from_spanned(item_mod));
            }
            if let Some((_, nested_items)) = &item_mod.content {
                if let Some(span) = find_inline_module_span(nested_items, &target_modules[1..]) {
                    return Some(span);
                }
            }
        }
    }

    None
}

fn collect_trait_spans(
    items: &[Item],
    target_modules: &[String],
    target_trait: &str,
) -> Vec<ContainerSpan> {
    let mut matches = Vec::new();
    let mut current_modules = Vec::new();
    collect_trait_spans_inner(
        items,
        &mut current_modules,
        target_modules,
        target_trait,
        &mut matches,
    );
    matches
}

fn collect_trait_spans_inner(
    items: &[Item],
    current_modules: &mut Vec<String>,
    target_modules: &[String],
    target_trait: &str,
    matches: &mut Vec<ContainerSpan>,
) {
    for item in items {
        match item {
            Item::Trait(item_trait)
                if current_modules.as_slice() == target_modules
                    && item_trait.ident == target_trait =>
            {
                matches.push(span_from_spanned(item_trait));
            }
            Item::Mod(item_mod) => {
                if let Some((_, nested_items)) = &item_mod.content {
                    current_modules.push(item_mod.ident.to_string());
                    collect_trait_spans_inner(
                        nested_items,
                        current_modules,
                        target_modules,
                        target_trait,
                        matches,
                    );
                    current_modules.pop();
                }
            }
            _ => {}
        }
    }
}

fn collect_inherent_impl_spans(
    items: &[Item],
    target_modules: &[String],
    owner_name: &str,
) -> Vec<ContainerSpan> {
    let mut matches = Vec::new();
    let mut current_modules = Vec::new();
    collect_inherent_impl_spans_inner(
        items,
        &mut current_modules,
        target_modules,
        owner_name,
        &mut matches,
    );
    matches
}

fn collect_inherent_impl_spans_inner(
    items: &[Item],
    current_modules: &mut Vec<String>,
    target_modules: &[String],
    owner_name: &str,
    matches: &mut Vec<ContainerSpan>,
) {
    for item in items {
        match item {
            Item::Impl(item_impl)
                if current_modules.as_slice() == target_modules
                    && item_impl.trait_.is_none()
                    && impl_owner_name(item_impl).as_deref() == Some(owner_name) =>
            {
                matches.push(span_from_spanned(item_impl));
            }
            Item::Mod(item_mod) => {
                if let Some((_, nested_items)) = &item_mod.content {
                    current_modules.push(item_mod.ident.to_string());
                    collect_inherent_impl_spans_inner(
                        nested_items,
                        current_modules,
                        target_modules,
                        owner_name,
                        matches,
                    );
                    current_modules.pop();
                }
            }
            _ => {}
        }
    }
}

fn impl_owner_name(item_impl: &ItemImpl) -> Option<String> {
    match item_impl.self_ty.as_ref() {
        Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        _ => None,
    }
}

fn single_container_match(
    container_label: &str,
    canon: &str,
    abs_path: &Path,
    matches: Vec<ContainerSpan>,
) -> Result<ContainerSpan, ploke_error::Error> {
    match matches.as_slice() {
        [span] => Ok(*span),
        [] => Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
            message: format!(
                "No {container_label} container found for {} in {}",
                canon,
                abs_path.display()
            ),
        })),
        spans => Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
            message: format!(
                "Ambiguous {container_label} container for {} in {}: {} candidates matched.",
                canon,
                abs_path.display(),
                spans.len()
            ),
        })),
    }
}

fn span_from_spanned<T: Spanned>(value: &T) -> ContainerSpan {
    let byte_range = value.span().byte_range();
    ContainerSpan {
        start: byte_range.start,
        end: byte_range.end,
    }
}

fn plan_braced_container_insert(
    content: &str,
    container: ContainerSpan,
    code: &str,
) -> Result<(usize, String), ploke_error::Error> {
    let container_text = content.get(container.start..container.end).ok_or_else(|| {
        ploke_error::Error::Domain(ploke_error::DomainError::Ui {
            message: "container span was out of bounds for the current file.".to_string(),
        })
    })?;
    let open_rel = container_text.find('{').ok_or_else(|| {
        ploke_error::Error::Domain(ploke_error::DomainError::Ui {
            message: "target container does not have an inline body to insert into.".to_string(),
        })
    })?;
    let close_rel = container_text.rfind('}').ok_or_else(|| {
        ploke_error::Error::Domain(ploke_error::DomainError::Ui {
            message: "target container does not end with a closing brace.".to_string(),
        })
    })?;

    let insert_at = container.start + close_rel;
    let body = &container_text[open_rel + 1..close_rel];
    let closing_indent = line_indent_at(content, insert_at);
    let body_indent = format!("{closing_indent}    ");
    let normalized_code = indent_inserted_code(code, &body_indent);
    let prefix = if body.trim().is_empty() { "\n" } else { "\n\n" };
    let replacement = format!("{prefix}{normalized_code}\n{closing_indent}");
    Ok((insert_at, replacement))
}

fn plan_file_scope_insert(content: &str, code: &str) -> (usize, String) {
    let normalized_code = indent_inserted_code(code, "");
    let prefix = if content.trim().is_empty() {
        ""
    } else if content.ends_with("\n\n") {
        ""
    } else if content.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    let mut replacement = format!("{prefix}{normalized_code}");
    if !replacement.ends_with('\n') {
        replacement.push('\n');
    }
    (content.len(), replacement)
}

fn line_indent_at(content: &str, byte_index: usize) -> String {
    let line_start = content[..byte_index]
        .rfind('\n')
        .map(|idx| idx + 1)
        .unwrap_or(0);
    content[line_start..byte_index]
        .chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .collect()
}

fn indent_inserted_code(code: &str, indent: &str) -> String {
    let trimmed = code.trim_matches('\n');
    let lines = trimmed.lines().collect::<Vec<_>>();
    let min_indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.chars()
                .take_while(|ch| *ch == ' ' || *ch == '\t')
                .count()
        })
        .min()
        .unwrap_or(0);

    lines
        .into_iter()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                let dedented = line.chars().skip(min_indent).collect::<String>();
                format!("{indent}{dedented}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn map_insert_planning_error(err: ploke_io::IoError) -> ploke_error::Error {
    match err {
        ploke_io::IoError::ParseError { path, message } => {
            ploke_error::Error::Domain(ploke_error::DomainError::Ui {
                message: format!(
                    "failed to parse Rust file {}: {}. If the file does not parse yet, use non_semantic_patch.",
                    path.display(),
                    message
                ),
            })
        }
        other => ploke_error::Error::from(other),
    }
}

lazy_static::lazy_static! {
    static ref INSERT_RUST_ITEM_PARAMETERS: serde_json::Value = json!({
        "type": "object",
        "properties": {
            "file": {
                "type": "string",
                "description": "Absolute or workspace-root-relative Rust file path."
            },
            "container_kind": {
                "type": "string",
                "enum": ["file", "module", "trait", "impl"],
                "description": "Where to insert the new item. Use `file` to append at file scope; `module`, `trait`, and `impl` insert before the container's closing brace."
            },
            "container_canon": {
                "type": "string",
                "description": "Canonical container path. Required for `module`, `trait`, and `impl`. For `impl`, provide the owner type path, e.g. `crate::module::TypeName`."
            },
            "item_kind": NodeKind::schema_property(),
            "code": {
                "type": "string",
                "description": "Full Rust source for the new item to insert."
            },
            "confidence": {
                "type": "number",
                "minimum": 0.0,
                "maximum": 1.0,
                "description": "Optional confidence indicator for the insertion proposal."
            }
        },
        "required": ["file", "container_kind", "item_kind", "code"],
        "additionalProperties": false
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventPriority, test_utils::new_test_harness::AppHarness};
    use uuid::Uuid;

    #[test]
    fn file_scope_insert_keeps_blank_line_before_new_item() {
        let source = "pub struct Foo;\n";
        let (_offset, replacement) = plan_file_scope_insert(source, "fn helper() {}\n");
        assert_eq!(replacement, "\nfn helper() {}\n");
    }

    #[test]
    fn braced_insert_indents_to_container_body() {
        let source = "impl Foo {\n    fn old() {}\n}\n";
        let container = ContainerSpan {
            start: 0,
            end: source.len() - 1,
        };
        let (offset, replacement) =
            plan_braced_container_insert(source, container, "fn helper() {}\n").expect("plan");
        assert_eq!(offset, source.len() - 2);
        assert_eq!(replacement, "\n\n    fn helper() {}\n");
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "test_harness")]
    async fn insert_rust_item_stages_file_scope_edit() {
        let harness = AppHarness::spawn().await.expect("spawn harness");
        let request_id = Uuid::new_v4();
        let call_id = ploke_core::ArcStr::from("test_insert_rust_item");
        let proposal_id = crate::app_state::core::derive_edit_proposal_id(request_id, &call_id);
        let ctx = Ctx {
            state: Arc::clone(&harness.state),
            event_bus: Arc::clone(&harness.event_bus),
            request_id,
            parent_id: request_id,
            call_id,
        };

        let params = InsertRustItemParams {
            file: Cow::Borrowed("src/structs.rs"),
            container_kind: InsertRustContainerKind::File,
            container_canon: None,
            item_kind: NodeKind::Function,
            code: Cow::Borrowed("fn inserted_helper() {}\n"),
            confidence: Some(0.9),
        };

        let result = InsertRustItem::execute(params, ctx).await.expect("execute");
        let parsed: ApplyCodeEditResult =
            serde_json::from_str(&result.content).expect("parse result");
        assert_eq!(parsed.staged, 1);
        assert!(parsed.files.iter().any(|file| file == "src/structs.rs"));

        let proposals = harness.state.proposals.read().await;
        let proposal = proposals.get(&proposal_id).expect("proposal");
        assert_eq!(proposal.edits.len(), 1);
        assert_eq!(
            proposal
                .edits
                .first()
                .expect("staged edit")
                .file_path
                .file_name()
                .and_then(|name| name.to_str()),
            Some("structs.rs")
        );
        drop(proposals);

        let mut realtime = harness.event_bus.subscribe(EventPriority::Realtime);
        while realtime.try_recv().is_ok() {}
    }
}
