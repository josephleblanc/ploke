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
        "required": ["search_term"],
        "additionalProperties": false
    });
}
impl Tool for RequestCodeContext {
    const NAME: &'static str = "request_code_context";
    const DESCRIPTION: &'static str =
        "Request additional code context from the repository up to a token budget.";

    type Params = RequestCodeContextInput;
    type Output = ploke_core::rag_types::RequestCodeContextResult;

    async fn run(self, p: Self::Params) -> Result<Self::Output, ploke_error::Error> {
        use crate::rag::utils::calc_top_k_for_budget;
        use ploke_rag::{RetrievalStrategy, RrfConfig, TokenBudget};

        let budget = TokenBudget { max_total: p.token_budget as usize, ..Default::default() };
        let query = p.search_term.unwrap_or_default();
        if query.trim().is_empty() {
            return Err(ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(
                "No query available (hint missing or empty)".to_string(),
            )));
        }
        let top_k = calc_top_k_for_budget(p.token_budget);
        let assembled = self
            .rag
            .get_context(
                &query,
                top_k,
                &budget,
                &RetrievalStrategy::Hybrid { rrf: RrfConfig::default(), mmr: None },
            )
            .await?;
        Ok(ploke_core::rag_types::RequestCodeContextResult { ok: true, query, top_k, context: assembled })
    }

    fn schema() -> &'static serde_json::Value {
        REQUEST_CODE_CONTEXT_PARAMETERS.deref()
    }

    fn tool_def() -> ToolFunctionDef {
        ToolFunctionDef {
            name: ToolName::RequestCodeContext,
            description: ToolDescr::RequestCodeContext,
            parameters: Self::schema().clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_def_serializes_expected_shape() {
        let def = <RequestCodeContext as Tool>::tool_def();
        let v = serde_json::to_value(&def).expect("serialize");
        let func = v.as_object().expect("def obj");
        assert_eq!(func.get("name").and_then(|n| n.as_str()), Some("request_code_context"));
        let params = func.get("parameters").and_then(|p| p.as_object()).expect("params obj");
        let req = params.get("required").and_then(|r| r.as_array()).expect("req arr");
        assert!(req.iter().any(|s| s.as_str() == Some("token_budget")));
        let props = params.get("properties").and_then(|p| p.as_object()).expect("props obj");
        assert!(props.contains_key("hint"));
    }
}

// --- GAT implementation ---
use std::borrow::Cow;

#[derive(Debug, Clone, Deserialize)]
pub struct RequestCodeContextParams<'a> {
    pub token_budget: u32,
    #[serde(borrow, default)]
    pub hint: Option<Cow<'a, str>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestCodeContextParamsOwned {
    pub token_budget: u32,
    pub hint: Option<String>,
}

/// Unit struct for GAT-based tool; uses Ctx to access RagService/state
#[derive(Default)]
pub struct RequestCodeContextGat {
    pub budget: TokenBudget,
    pub strategy: RetrievalStrategy,
}

impl super::GatTool for RequestCodeContextGat {
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
        RequestCodeContextParamsOwned { token_budget: params.token_budget, hint: params.hint.clone().map(|h| h.into_owned()) }
    }

    // fn tool_def() -> ToolDefinition{
    //     ToolFunctionDef {
    //         name: ToolName::RequestCodeContext,
    //         description: ToolDescr::RequestCodeContext,
    //         parameters: Self::schema().clone(),
    //     }.into()
    // }

    async fn execute<'de>(params: Self::Params<'de>, ctx: Ctx) -> Result<ToolResult, ploke_error::Error> {
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
        let hint = params.hint.as_ref().map(|h| h.as_ref()).unwrap_or("");
        if hint.trim().is_empty() {
            return Err(ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(
                "No query available (hint missing or empty)".to_string(),
            )));
        }
        let top_k = calc_top_k_for_budget(params.token_budget);
        let budget = TokenBudget { max_total: params.token_budget as usize, ..Default::default() };
        let strategy = RetrievalStrategy::Hybrid { rrf: RrfConfig::default(), mmr: None };
        let assembled = rag.get_context(hint, top_k, &budget, &strategy).await?;
        let result = ploke_core::rag_types::RequestCodeContextResult {
            ok: true,
            query: hint.to_string(),
            top_k,
            context: assembled,
        };
        let serialized = serde_json::to_string(&result).expect("Invalid state: serialization");
        Ok(ToolResult { content: serialized })
    }
}

#[cfg(test)]
mod gat_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn params_deserialize_and_into_owned() {
        let raw = r#"{"token_budget":512,"hint":"foo bar"}"#;
        let params = RequestCodeContextGat::deserialize_params(raw).expect("parse");
        assert_eq!(params.token_budget, 512);
        assert_eq!(params.hint.as_deref(), Some("foo bar"));
        let owned = RequestCodeContextGat::into_owned(&params);
        assert_eq!(owned.token_budget, 512);
        assert_eq!(owned.hint.as_deref(), Some("foo bar"));
    }

    #[test]
    fn name_desc_and_schema_present() {
        assert!(matches!(RequestCodeContextGat::name(), ToolName::RequestCodeContext));
        assert!(matches!(RequestCodeContextGat::description(), ToolDescr::RequestCodeContext));
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
            call_id: Arc::<str>::from("rcctx-test"),
        };
        let params = RequestCodeContextParams { token_budget: 256, hint: Some(Cow::Borrowed("fn")) };
        let out = RequestCodeContextGat::execute(params, ctx).await;
        assert!(out.is_err(), "expected execute to error with mock/unavailable RAG");
    }
}
