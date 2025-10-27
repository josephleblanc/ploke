use ploke_core::rag_types::{
    ApplyCodeEditResult, CanonPath, ConciseContext, ContextPartKind, GetFileMetadataResult,
    NodeFilepath, RequestCodeContextArgs, RequestCodeContextResult,
};
use uuid::Uuid;

#[test]
fn serde_roundtrip_request_code_context() {
    let args = RequestCodeContextArgs {
        token_budget: Some( 512 ),
        search_term: "SimpleStruct".to_string(),
    };
    let args_json = serde_json::to_string(&args).expect("serialize args");
    let args_back: RequestCodeContextArgs =
        serde_json::from_str(&args_json).expect("deserialize args");
    assert_eq!(args_back.token_budget, Some( 512 ));
    assert_eq!(args_back.search_term, "SimpleStruct");

    let part = ConciseContext {
        file_path: NodeFilepath("id://dummy".to_string()),
        canon_path: CanonPath("some::module::dummy".to_string()),
        snippet: "fn foo() {}".to_string(),
    };
    let result = RequestCodeContextResult {
        ok: true,
        search_term: "foo".to_string(),
        top_k: 3,
        context: vec![part.clone()],
        kind: ContextPartKind::Code,
    };
    let res_json = serde_json::to_string(&result).expect("serialize result");
    let res_back: RequestCodeContextResult =
        serde_json::from_str(&res_json).expect("deserialize result");
    assert!(res_back.ok);
    assert_eq!(res_back.search_term, "foo");
    assert_eq!(res_back.top_k, 3);
    assert_eq!(res_back.context, vec![part]);
    assert_eq!(res_back.kind, ContextPartKind::Code);
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
