use ploke_core::rag_types::{
    AssembledContext, AssembledMeta, ConciseContext, RequestCodeContextResult,
};
use ploke_db::get_by_id::{GetNodeInfo, NodePaths};

use crate::TOKEN_LIMIT;

use super::*;

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
                    "description": "Search term guide which code guide hybrid semantic search and bm25."
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
        let token_budget = params.token_budget.unwrap_or(TOKEN_LIMIT);
        let top_k = calc_top_k_for_budget(token_budget);
        let budget = TokenBudget {
            max_total: token_budget as usize,
            ..Default::default()
        };
        let strategy = RetrievalStrategy::Hybrid {
            rrf: RrfConfig::default(),
            mmr: None,
        };
        let AssembledContext { parts, stats: _ } = rag
            .get_context(&search_term, top_k, &budget, &strategy)
            .await?;

        let assembled_meta = AssembledMeta {
            search_term,
            top_k,
            kind: ContextPartKind::Code,
        };
        let result = RequestCodeContextResult::from_assembled(parts, assembled_meta);
        let serialized = serde_json::to_string(&result).expect("serialization");
        Ok(ToolResult {
            content: serialized,
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
                "description": "Request additional code context from the repository up to a token budget.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "search_term": {
                            "type": "string",
                            "description": "Search term guide which code guide hybrid semantic search and bm25."
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

