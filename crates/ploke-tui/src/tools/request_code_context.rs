use ploke_core::rag_types::{
    AssembledContext, AssembledMeta, ConciseContext, RequestCodeContextResult,
};
use ploke_db::get_by_id::{GetNodeInfo, NodePaths};

use super::*;
use ploke_core::RetrievalScope;

// Canonical schema: RequestCodeContextArgs { token_budget, hint }
pub struct RequestCodeContext {
    rag: Arc<RagService>,
}

#[derive(Clone, PartialOrd, PartialEq, Deserialize)]
pub struct RequestCodeContextInput {
    pub token_budget: u32,
    #[serde(default)]
    pub search_term: Option<String>,
}

lazy_static::lazy_static! {
    static ref REQUEST_CODE_CONTEXT_PARAMETERS: serde_json::Value = json!({
            "type": "object",
            "properties": {
                "search_term": {
                    "type": "string",
                    "description": "A likely identifier, module, file, error, or other narrowing term for broad code retrieval. Prefer exact symbols or nearby file/module names over long natural-language guesses."
                },
                "token_budget": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Optional maximum tokens of code context to return, sane defaults"
                }
            }
        }
    );
}

fn zero_result_note(search_term: &str) -> String {
    format!(
        "No indexed snippets matched `{search_term}`. This tool is broad retrieval, not exact symbol lookup."
    )
}

fn zero_result_next_steps() -> Vec<String> {
    vec![
        "Retry with an exact symbol, type, method, module, or error name if you know one."
            .to_string(),
        "If you know the area but not the symbol, use list_dir or read_file on likely directories/files."
            .to_string(),
        "If you know the item name, switch to code_item_lookup for exact-definition lookup."
            .to_string(),
    ]
}

// --- GAT-based tool impl ---
use std::borrow::Cow;

#[derive(Debug, Clone, Deserialize)]
pub struct RequestCodeContextParams<'a> {
    pub token_budget: Option<u32>,
    #[serde(borrow)]
    pub search_term: Option<Cow<'a, str>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequestCodeContextParamsOwned {
    pub token_budget: Option<u32>,
    pub search_term: Option<String>,
}

#[derive(Default)]
pub struct RequestCodeContextGat {
    pub budget: TokenBudget,
    pub strategy: RetrievalStrategy,
}

impl super::Tool for RequestCodeContextGat {
    type Output = ploke_core::rag_types::RequestCodeContextResult;
    type OwnedParams = RequestCodeContextParamsOwned;
    type Params<'de>
        = RequestCodeContextParams<'de>
    where
        Self: 'de;

    fn name() -> ToolName {
        ToolName::RequestCodeContext
    }
    fn description() -> ToolDescr {
        ToolDescr::RequestCodeContext
    }
    fn schema() -> &'static serde_json::Value {
        REQUEST_CODE_CONTEXT_PARAMETERS.deref()
    }

    fn build(_ctx: &super::Ctx) -> Self {
        RequestCodeContextGat::default()
    }

    fn into_owned<'a>(params: &Self::Params<'a>) -> Self::OwnedParams {
        RequestCodeContextParamsOwned {
            token_budget: params.token_budget,
            search_term: params.search_term.as_ref().map(|s| s.to_string()),
        }
    }

    async fn execute<'de>(
        params: Self::Params<'de>,
        ctx: Ctx,
    ) -> Result<ToolResult, ploke_error::Error> {
        use crate::rag::utils::calc_top_k_for_budget;
        use ploke_rag::{RetrievalStrategy, RrfConfig, TokenBudget};
        if let Some(parse_failure) = ctx
            .state
            .with_system_read(|sys| sys.last_parse_failure().cloned())
            .await
        {
            return Err(tool_ui_error(parse_failure.message.clone()));
        }
        let rag = match &ctx.state.rag {
            Some(r) => r.clone(),
            None => {
                return Err(ploke_error::Error::Internal(
                    ploke_error::InternalError::CompilerError(
                        "RAG service unavailable".to_string(),
                    ),
                ));
            }
        };
        let mut search_term_opt = params
            .search_term
            .as_ref()
            .map(|s| s.as_ref().trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        if search_term_opt.is_none() {
            use crate::chat_history::MessageKind;
            let history = ctx.state.chat.read().await;
            let mut last_user: Option<String> = None;
            for msg in history.iter_path() {
                if matches!(msg.kind, MessageKind::User) {
                    last_user = Some(msg.content.clone());
                }
            }
            search_term_opt = last_user
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        }
        let Some(search_term) = search_term_opt else {
            return Err(ploke_error::Error::Internal(
                ploke_error::InternalError::CompilerError(
                    "No query available (search_term missing or empty)".to_string(),
                ),
            ));
        };
        let cfg = ctx.state.config.read().await;
        let token_budget = params.token_budget.unwrap_or(cfg.token_limit);
        let top_k = calc_top_k_for_budget(token_budget).min(cfg.rag.top_k);
        let budget = TokenBudget {
            max_total: token_budget as usize,
            per_part_max: cfg.rag.per_part_max_tokens,
            ..Default::default()
        };
        let strategy = cfg.rag.strategy.to_runtime();
        drop(cfg);
        let AssembledContext { parts, stats } = rag
            .get_context(
                &search_term,
                top_k,
                &budget,
                &strategy,
                RetrievalScope::LoadedWorkspace,
            )
            .await?;

        let assembled_meta = AssembledMeta {
            search_term,
            top_k,
            kind: ContextPartKind::Code,
        };
        tracing::debug!(?parts, ?stats);
        let mut result = RequestCodeContextResult::from_assembled(parts, assembled_meta);
        let summary = if result.context.is_empty() {
            result.note = Some(zero_result_note(&result.search_term));
            result.next_steps = zero_result_next_steps();
            "No code context found (0 snippets)".to_string()
        } else {
            format!("Context assembled: {} snippets", result.context.len())
        };
        let mut ui_payload = super::ToolUiPayload::new(Self::name(), ctx.call_id.clone(), summary)
            .with_field("search_term", result.search_term.as_str())
            .with_field("top_k", result.top_k.to_string())
            .with_field("returned", result.context.len().to_string());
        if let Some(note) = result.note.as_ref() {
            let details = std::iter::once(note.as_str().to_string())
                .chain(
                    result
                        .next_steps
                        .iter()
                        .enumerate()
                        .map(|(idx, step)| format!("{}. {}", idx + 1, step)),
                )
                .collect::<Vec<_>>()
                .join("\n");
            ui_payload = ui_payload.with_details(details);
        }
        let serialized = serde_json::to_string(&result).expect("serialization");
        Ok(ToolResult {
            content: serialized,
            ui_payload: Some(ui_payload),
        })
    }
}
#[cfg(test)]
mod gat_tests {
    use super::*;

    #[test]
    fn params_deserialize_and_into_owned() {
        let raw = r#"{"token_budget":512,"search_term":"foo bar"}"#;
        let params = RequestCodeContextGat::deserialize_params(raw).expect("parse");
        assert_eq!(params.token_budget, Some(512));
        assert_eq!(params.search_term.as_deref(), Some("foo bar"));
        let owned = RequestCodeContextGat::into_owned(&params);
        assert_eq!(owned.token_budget, Some(512));
        assert_eq!(owned.search_term.as_deref(), Some("foo bar"));
    }

    #[test]
    fn params_missing_search_term_still_parses() {
        let raw = r#"{"token_budget":256}"#;
        let params = RequestCodeContextGat::deserialize_params(raw).expect("parse");
        assert_eq!(params.token_budget, Some(256));
        assert!(params.search_term.is_none());
    }

    #[test]
    fn name_desc_and_schema_present() {
        assert!(matches!(
            RequestCodeContextGat::name(),
            ToolName::RequestCodeContext
        ));
        assert!(matches!(
            RequestCodeContextGat::description(),
            ToolDescr::RequestCodeContext
        ));
        let schema = RequestCodeContextGat::schema();
        let obj = schema.as_object().expect("schema obj");
        assert!(obj.contains_key("properties"));
    }

    #[test]
    fn de_to_value() -> color_eyre::Result<()> {
        let def = <RequestCodeContextGat as Tool>::tool_def();
        let v = serde_json::to_value(&def).expect("serialize");
        eprintln!("{}", serde_json::to_string_pretty(&v)?);
        let expected = json!({
            "type": "function",
            "function": {
                "name": "request_code_context",
                "description": "Request broad code context from the indexed workspace up to a token budget. Best for exploratory retrieval when you have likely identifiers, module names, file names, or error/type names. If it returns 0 snippets or broad irrelevant snippets, narrow the query with exact symbols or switch to code_item_lookup for exact definitions, or use list_dir/read_file once you know the area.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "search_term": {
                            "type": "string",
                            "description": "A likely identifier, module, file, error, or other narrowing term for broad code retrieval. Prefer exact symbols or nearby file/module names over long natural-language guesses."
                        },
                        "token_budget": {
                            "type": "integer",
                            "minimum": 1,
                            "description": "Optional maximum tokens of code context to return, sane defaults"
                        }
                    }
                }
            }
        });
        assert_eq!(expected, v);
        Ok(())
    }
}
