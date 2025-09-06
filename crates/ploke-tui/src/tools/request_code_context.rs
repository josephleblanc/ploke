
use ploke_core::rag_types::{AssembledContext, AssembledMeta, ConciseContext, RequestCodeContextResult};

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
            },
            "required": ["search_term"]
        }
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_def_serializes_expected_shape() -> color_eyre::Result<()> {
        let def = <RequestCodeContextGat as Tool>::tool_def();
        let v = serde_json::to_value(&def).expect("serialize");
        eprintln!("{}", serde_json::to_string_pretty(&v)?);
        let func = v
            .get("function")
            .expect("function field name")
            .as_object()
            .expect("def obj");
        assert_eq!(
            func.get("name").and_then(|n| n.as_str()),
            Some("request_code_context")
        );
        let params = func
            .get("parameters")
            .and_then(|p| p.as_object())
            .expect("parameters obj");
        let req = params
            .get("required")
            .and_then(|r| r.as_array())
            .expect("req arr");
        assert!(req.iter().any(|s| s.as_str() == Some("search_term")));
        let props = params
            .get("properties")
            .and_then(|p| p.as_object())
            .expect("props obj");
        assert!(props.contains_key("search_term"));
        Ok(())
    }
}

// --- GAT implementation ---
use std::borrow::Cow;

#[derive(Debug, Clone, Deserialize)]
pub struct RequestCodeContextParams<'a> {
    pub token_budget: Option<u32>,
    #[serde(borrow)]
    pub search_term: Cow<'a, str>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequestCodeContextParamsOwned {
    pub token_budget: Option< u32 >,
    pub search_term: String,
}

/// Unit struct for GAT-based tool; uses Ctx to access RagService/state
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
            search_term: params.search_term.to_string(),
        }
    }

    // fn tool_def() -> ToolDefinition{
    //     ToolFunctionDef {
    //         name: ToolName::RequestCodeContext,
    //         description: ToolDescr::RequestCodeContext,
    //         parameters: Self::schema().clone(),
    //     }.into()
    // }

    async fn execute<'de>(
        params: Self::Params<'de>,
        ctx: Ctx,
    ) -> Result<ToolResult, ploke_error::Error> {
        use crate::rag::utils::calc_top_k_for_budget;
        use ploke_rag::{RetrievalStrategy, RrfConfig, TokenBudget};
        fn extract_llm_context(assembled: AssembledContext) {
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
        let search_term = params.search_term.as_ref();
        if search_term.trim().is_empty() {
            return Err(ploke_error::Error::Internal(
                ploke_error::InternalError::CompilerError(
                    "No query available (search_term missing or empty)".to_string(),
                ),
            ));
        }
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
        let AssembledContext {
            parts,
            stats,
        } = rag.get_context(search_term, top_k, &budget, &strategy).await?;
        let assembled_meta = AssembledMeta {
            search_term: params.search_term.to_string(),
            top_k,
            kind: ContextPartKind::Code
        };
        tracing::info!("Executed Tool: {:?} \nWith stats: {:#?}", Self::name(), stats);
        let result = RequestCodeContextResult::from_assembled(parts, assembled_meta);
        let serialized = serde_json::to_string(&result).expect("Invalid state: serialization");
        Ok(ToolResult {
            content: serialized,
        })
    }
}

#[cfg(test)]
mod gat_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn params_deserialize_and_into_owned() {
        let raw = r#"{"token_budget":512,"search_term":"foo bar"}"#;
        let params = RequestCodeContextGat::deserialize_params(raw).expect("parse");
        assert_eq!(params.token_budget, Some( 512 ));
        assert_eq!(params.search_term, "foo bar");
        let owned = RequestCodeContextGat::into_owned(&params);
        assert_eq!(owned.token_budget, Some( 512 ));
        assert_eq!(owned.search_term, "foo bar");
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

    #[tokio::test]
    async fn execute_returns_error_without_rag_success() {
        use crate::event_bus::EventBusCaps;
        let state = Arc::new(crate::test_utils::mock::create_mock_app_state());
        let event_bus = Arc::new(crate::EventBus::new(EventBusCaps::default()));
        let ctx = super::Ctx {
            state,
            event_bus,
            request_id: Uuid::new_v4(),
            parent_id: Uuid::new_v4(),
            call_id: ArcStr::from("rcctx-test"),
        };
        let params = RequestCodeContextParams {
            token_budget: Some( 256 ),
            search_term: Cow::Borrowed("fn"),
        };
        let out = RequestCodeContextGat::execute(params, ctx).await;
        assert!(
            out.is_err(),
            "expected execute to error with mock/unavailable RAG"
        );
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
                    },
                    "required": ["search_term"]
                }
            }
        });
        assert_eq!(expected, v);

        Ok(())
    }
}
