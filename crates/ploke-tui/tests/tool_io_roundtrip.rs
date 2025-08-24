use ploke_core::rag_types::{
    ApplyCodeEditResult, AssembledContext, ContextPart, ContextPartKind, ContextStats,
    GetFileMetadataResult, Modality, RequestCodeContextArgs, RequestCodeContextResult,
};
use serde_json;
use uuid::Uuid;

#[test]
fn serde_roundtrip_request_code_context() {
    let args = RequestCodeContextArgs {
        token_budget: 512,
        hint: Some("SimpleStruct".to_string()),
    };
    let args_json = serde_json::to_string(&args).expect("serialize args");
    let args_back: RequestCodeContextArgs =
        serde_json::from_str(&args_json).expect("deserialize args");
    assert_eq!(args_back.token_budget, 512);
    assert_eq!(args_back.hint.as_deref(), Some("SimpleStruct"));

    let part = ContextPart {
        id: Uuid::new_v4(),
        file_path: "id://dummy".to_string(),
        ranges: vec![(0, 10)],
        kind: ContextPartKind::Code,
        text: "fn foo() {}".to_string(),
        score: 0.99,
        modality: Modality::HybridFused,
    };
    let result = RequestCodeContextResult {
        ok: true,
        query: "foo".to_string(),
        top_k: 3,
        context: AssembledContext {
            parts: vec![part],
            stats: ContextStats {
                total_tokens: 42,
                files: 1,
                parts: 1,
                truncated_parts: 0,
                dedup_removed: 0,
            },
        },
    };
    let res_json = serde_json::to_string(&result).expect("serialize result");
    let res_back: RequestCodeContextResult =
        serde_json::from_str(&res_json).expect("deserialize result");
    assert!(res_back.ok);
    assert_eq!(res_back.query, "foo");
    assert_eq!(res_back.top_k, 3);
    assert_eq!(res_back.context.parts.len(), 1);
    assert_eq!(res_back.context.stats.total_tokens, 42);
}

#[test]
fn serde_roundtrip_get_file_metadata_result() {
    let res = GetFileMetadataResult {
        ok: true,
        file_path: "/tmp/file.rs".to_string(),
        exists: true,
        byte_len: 1234,
        modified_ms: Some(1_700_000_000_000),
        file_hash: Uuid::nil().to_string(),
        tracking_hash: Uuid::new_v4().to_string(),
    };
    let json = serde_json::to_string(&res).expect("serialize");
    let back: GetFileMetadataResult = serde_json::from_str(&json).expect("deserialize");
    assert!(back.ok);
    assert_eq!(back.file_path, "/tmp/file.rs");
    assert!(back.exists);
    assert_eq!(back.byte_len, 1234);
    assert!(back.modified_ms.is_some());
    assert_eq!(back.file_hash.len(), 36);
    assert_eq!(back.tracking_hash.len(), 36);
}

#[test]
fn serde_roundtrip_apply_code_edit_result() {
    let res = ApplyCodeEditResult {
        ok: true,
        staged: 2,
        applied: 0,
        files: vec!["src/lib.rs".to_string(), "src/main.rs".to_string()],
        preview_mode: "diff".to_string(),
        auto_confirmed: false,
    };
    let json = serde_json::to_string(&res).expect("serialize");
    let back: ApplyCodeEditResult = serde_json::from_str(&json).expect("deserialize");
    assert!(back.ok);
    assert_eq!(back.staged, 2);
    assert_eq!(back.applied, 0);
    assert_eq!(back.files.len(), 2);
    assert_eq!(back.preview_mode, "diff");
    assert!(!back.auto_confirmed);
}
