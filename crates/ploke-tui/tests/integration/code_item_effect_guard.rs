use std::borrow::Cow;

use ploke_tui::tools::{
    Tool,
    code_item_effect_guard::{CodeItemEffectGuard, CodeItemEffectGuardParams},
    code_item_endpoint::CodeItemEndpoint,
};

use crate::call_graph_tool_support::{
    AxumTaskSpawnEffectToolFixture, assert_task_spawn_effects, ui_field,
};

#[tokio::test]
async fn code_item_effect_guard_classifies_real_corpus_task_spawn_paths() {
    let fixture = AxumTaskSpawnEffectToolFixture::new().await;
    let crate_root = fixture
        .file_path
        .parent()
        .and_then(|src| src.parent())
        .expect("axum form.rs should live below crate src");
    let owner = CodeItemEndpoint {
        item_name: Cow::Borrowed("deserialize_error_status_codes"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        body_contains: None,
    };
    let test_client_new = CodeItemEndpoint {
        item_name: Cow::Borrowed("new"),
        file_path: Cow::Owned(
            crate_root
                .join("src/test_helpers/test_client.rs")
                .display()
                .to_string(),
        ),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Borrowed("crate::test_helpers::test_client"),
        owner_trait: None,
        owner_type: Some(Cow::Borrowed("TestClient")),
        parent_name: None,
        body_contains: None,
    };

    let guarded = CodeItemEffectGuard::execute(
        CodeItemEffectGuardParams {
            owner: owner.clone(),
            guard: test_client_new,
            effect_class: Cow::Borrowed("async_task_spawn"),
            max_depth: Some(3),
            max_paths: Some(16),
        },
        fixture.ctx("axum-task-spawn-effect-guarded"),
    )
    .await
    .expect("guarded effect guard tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&guarded.content).expect("deserialize CodeItemEffectGuardResult");
    let effects = payload
        .get("effects")
        .and_then(serde_json::Value::as_array)
        .expect("effects array");
    let violations = payload
        .get("violations")
        .and_then(serde_json::Value::as_array)
        .expect("violations array");

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis:
    //   "Can this entrypoint reach a sensitive sink without passing through
    //   the reviewed guard?"
    //
    // Source-oracle chain:
    //   axum/src/form.rs:262
    //     `deserialize_error_status_codes` calls `TestClient::new(app)`.
    //   axum/src/test_helpers/test_client.rs:36
    //     `TestClient::new` calls `spawn_service(svc)`.
    //   axum/src/test_helpers/test_client.rs:23
    //     `spawn_service` calls `tokio::spawn(...)`.
    assert_eq!(
        payload.get("owner_id").and_then(serde_json::Value::as_str),
        Some(fixture.owner.to_string().as_str())
    );
    assert_eq!(
        payload
            .get("effect_class")
            .and_then(serde_json::Value::as_str),
        Some("async_task_spawn")
    );
    assert_eq!(
        payload.get("guarded").and_then(serde_json::Value::as_bool),
        Some(true),
        "TestClient::new should guard every resolved path to the task-spawn effect: {payload:#?}"
    );
    assert_task_spawn_effects(effects, &fixture, "code_item_effect_guard effects");
    assert!(
        violations.is_empty(),
        "guarded report should not include violating effects: {payload:#?}"
    );
    assert_source_file(
        payload
            .get("source_files")
            .and_then(serde_json::Value::as_array)
            .expect("source_files array"),
        "axum/src/test_helpers/test_client.rs",
        "code_item_effect_guard source files",
    );
    assert!(
        payload
            .get("proof_context")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|rows| !rows.is_empty()),
        "effect guard tool should surface proof context rows: {payload:#?}"
    );
    let ui = guarded.ui_payload.as_ref().expect("guarded ui payload");
    assert_eq!(ui_field(ui, "guarded"), "true");
    assert_eq!(ui_field(ui, "effects"), effects.len().to_string());
    assert_eq!(ui_field(ui, "violations"), "0");

    let router_new = CodeItemEndpoint {
        item_name: Cow::Borrowed("new"),
        file_path: Cow::Owned(crate_root.join("src/routing/mod.rs").display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Borrowed("crate::routing"),
        owner_trait: None,
        owner_type: Some(Cow::Borrowed("Router")),
        parent_name: None,
        body_contains: None,
    };
    let unguarded = CodeItemEffectGuard::execute(
        CodeItemEffectGuardParams {
            owner,
            guard: router_new,
            effect_class: Cow::Borrowed("async_task_spawn"),
            max_depth: Some(3),
            max_paths: Some(16),
        },
        fixture.ctx("axum-task-spawn-effect-unguarded"),
    )
    .await
    .expect("unguarded effect guard tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&unguarded.content).expect("deserialize unguarded result");
    let effects = payload
        .get("effects")
        .and_then(serde_json::Value::as_array)
        .expect("unguarded effects array");
    let violations = payload
        .get("violations")
        .and_then(serde_json::Value::as_array)
        .expect("unguarded violations array");
    assert_eq!(
        payload.get("guarded").and_then(serde_json::Value::as_bool),
        Some(false),
        "unrelated Router::new should not guard the task-spawn path: {payload:#?}"
    );
    assert_task_spawn_effects(
        effects,
        &fixture,
        "unguarded code_item_effect_guard effects",
    );
    assert_eq!(
        violations.len(),
        1,
        "unguarded report should return the reachable task-spawn effect as a violation: {payload:#?}"
    );
    assert_eq!(
        violations[0]
            .get("call_site")
            .and_then(|site| site.get("site_id"))
            .and_then(serde_json::Value::as_str),
        Some(fixture.spawn_site.to_string().as_str())
    );
    let ui = unguarded.ui_payload.as_ref().expect("unguarded ui payload");
    assert_eq!(ui_field(ui, "guarded"), "false");
    assert_eq!(ui_field(ui, "violations"), violations.len().to_string());
}

fn assert_source_file(files: &[serde_json::Value], suffix: &str, label: &str) {
    assert!(
        files
            .iter()
            .any(|file| file.as_str().is_some_and(|path| path.ends_with(suffix))),
        "{label} should include source file ending with {suffix:?}: {files:#?}"
    );
}
