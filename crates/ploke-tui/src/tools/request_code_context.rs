use super::*;

pub struct RequestCodeContext {
    rag: Arc<RagService>,
    top_k: u16,
    budget: TokenBudget,
    strategy: RetrievalStrategy,
}

#[derive(Clone, PartialOrd, PartialEq, Deserialize)]
pub struct RequestCodeContextInput {
    search_term: String,
    top_k: Option<u16>,
}

#[derive(Clone, PartialOrd, PartialEq, Serialize)]
pub struct RequestCodeContextOutput {
    code: Vec<CodeSnippet>,
    meta: Vec<SnippetMeta>,
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Serialize, Ord, Eq)]
pub struct CodeSnippet {
    file_path: String,
    snippet: String,
    // canonical_path: NodePath
}

#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Serialize)]
pub struct SnippetMeta {
    id: Uuid,
    kind: ContextPartKind,
    score: f32,
    modality: Modality,
}

impl SnippetMeta {
    fn extract_meta(cp: &ContextPart) -> Self {
        SnippetMeta {
            id: cp.id,
            kind: cp.kind,
            score: cp.score,
            modality: cp.modality,
        }
    }
}

lazy_static::lazy_static! {
    static ref REQUEST_CODE_CONTEXT_PARAMETERS: serde_json::Value = json!({
        "type": "object",
        "properties": {
            "search_term": {
                "type": "string",
                "description": "The text used to perform a dense vector similarity and bm25 hybrid search of the code base."
            },
            "top_k": {
                "type": "integer",
                "minimum": 1,
                "description": "Optional suggestion for number of results to return."
            }
        },
        "required": ["search_term"],
        "additionalProperties": false
    });
}
impl Tool for RequestCodeContext {
    const NAME: &'static str = "request_code_context";
    const DESCRIPTION: &'static str =
        "Perform a dense vector similarity and bm25 hybrid search of the code base.";

    type Params = RequestCodeContextInput;
    type Output = RequestCodeContextOutput;

    async fn run(self, p: Self::Params) -> Result<Self::Output, ploke_error::Error> {
        let query = &p.search_term;
        let top_k = p.top_k.unwrap_or(self.top_k);
        let budget = self.budget;
        let strategy = self.strategy;
        let assembed_context = self
            .rag
            .get_context(query, top_k as usize, &budget, &strategy)
            .await?;

        let stats = assembed_context.stats;
        let parts = assembed_context.parts;
        let (snippets, metadata): (Vec<CodeSnippet>, Vec<SnippetMeta>) = parts
            .into_iter()
            .map(|cp| {
                let meta = SnippetMeta {
                    id: cp.id,
                    kind: cp.kind,
                    score: cp.score,
                    modality: cp.modality,
                };
                let snippet = CodeSnippet {
                    file_path: cp.file_path,
                    snippet: cp.text, // , canonical_path
                };
                (snippet, meta)
            })
            .unzip();
        let all_snippets = RequestCodeContextOutput {
            code: snippets,
            meta: metadata,
        };
        Ok(all_snippets)
    }

    fn schema() -> &'static serde_json::Value {
        REQUEST_CODE_CONTEXT_PARAMETERS.deref()
        // match REQUEST_CODE_CONTEXT_PARAMETERS.as_object() {
        //     Some(map) => map,
        //     None => panic!("Tool schema must be well-formed json object")
        // }
    }

    fn tool_def() -> ToolFunctionDef {
        ToolFunctionDef {
            name: ToolName::RequestCodeContext,
            description: ToolDescr::RequestCodeContext,
            // TODO: See if it is possible to get rid of the clone, somehow, perhaps by
            // implementing Deserialize on `&'static Value` or something? Not sure how serde works
            // here, or if it is possible to create a zero-alloc version since we have a &'static
            // underlying type.
            parameters: Self::schema().clone(),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn tool_def_serializes_expected_shape() {
        let def = <RequestCodeContext as Tool>::tool_def();
        let v = serde_json::to_value(&def).expect("serialize");
        let func = v.as_object().expect("def obj");
        assert_eq!(func.get("name").and_then(|n| n.as_str()), Some("request_code_context"));
        let params = func.get("parameters").and_then(|p| p.as_object()).expect("params obj");
        let props = params.get("properties").and_then(|p| p.as_object()).expect("props obj");
        assert!(props.contains_key("search_term"));
        assert!(props.contains_key("top_k"));
    }
}

// --- GAT implementation ---
use std::borrow::Cow;

#[derive(Debug, Clone, Deserialize)]
pub struct RequestCodeContextParams<'a> {
    #[serde(borrow)]
    pub search_term: Cow<'a, str>,
    #[serde(default)]
    pub top_k: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestCodeContextParamsOwned {
    pub search_term: String,
    pub top_k: Option<u16>,
}

/// Unit struct for GAT-based tool; uses Ctx to access RagService/state
pub struct RequestCodeContextGat;

impl super::GatTool for RequestCodeContextGat {
    type Output = RequestCodeContextOutput;
    type OwnedParams = RequestCodeContextParamsOwned;
    type Params<'de> = RequestCodeContextParams<'de>;

    fn name() -> ToolName { ToolName::RequestCodeContext }
    fn description() -> ToolDescr { ToolDescr::RequestCodeContext }
    fn schema() -> &'static serde_json::Value { REQUEST_CODE_CONTEXT_PARAMETERS.deref() }

    fn build(_ctx: &super::Ctx) -> Self { RequestCodeContextGat }

    fn into_owned<'a>(params: &Self::Params<'a>) -> Self::OwnedParams {
        RequestCodeContextParamsOwned {
            search_term: params.search_term.clone().into_owned(),
            top_k: params.top_k,
        }
    }

    async fn run<'a>(self, params: &Self::Params<'a>, ctx: super::Ctx) -> Result<Self::Output, ploke_error::Error> {
        use ploke_rag::{TokenBudget, RetrievalStrategy, RrfConfig};
        let rag = match &ctx.state.rag {
            Some(r) => r.clone(),
            None => {
                return Err(ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(
                    "RAG service unavailable".to_string(),
                )))
            }
        };
        let top_k = params.top_k.unwrap_or(16) as usize;
        let budget = TokenBudget::default();
        let strategy = RetrievalStrategy::Hybrid { rrf: RrfConfig::default(), mmr: None };
        let assembled = rag
            .get_context(params.search_term.as_ref(), top_k, &budget, &strategy)
            .await?;
        let (code, meta): (Vec<CodeSnippet>, Vec<SnippetMeta>) = assembled.parts.into_iter().map(|cp| {
            let meta = SnippetMeta::extract_meta(&cp);
            let snippet = CodeSnippet { file_path: cp.file_path, snippet: cp.text };
            (snippet, meta)
        }).unzip();
        Ok(RequestCodeContextOutput { code, meta })
    }
}
