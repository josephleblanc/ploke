use super::*;
use crate::cli::prototype1_state::walk::phase::WalkPhase;
use crate::inner::core::{RegisteredRunRole, RunIntent, RunStorageRoots};
use crate::inner::registry::RunRegistration;
use crate::model_registry::save_parent_patcher_model;
use crate::record::read_compressed_record;
use crate::run_registry::RunExecutionStatus;
use ploke_core::ArcStr;
use ploke_tui::chat_history::{MessageKind, MessageStatus};
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;
use uuid::Uuid;

fn sample_message(
    kind: MessageKind,
    content: &str,
    tool_call_id: Option<&str>,
) -> crate::record::ConversationMessage {
    crate::record::ConversationMessage {
        id: Uuid::nil(),
        branch_id: Uuid::nil(),
        status: MessageStatus::Completed,
        metadata: None,
        parent: None,
        children: Vec::new(),
        selected_child: None,
        content: content.to_string(),
        kind,
        tool_call_id: tool_call_id.map(ArcStr::from),
        tool_payload: None,
        context_status: Default::default(),
        last_included_turn: None,
        include_count: 0,
    }
}

fn write_test_run_record(path: &Path, run_arm: crate::runner::RunArm) {
    let prepared = crate::spec::PreparedSingleRun {
        task_id: "org__repo-1".to_string(),
        repo_root: PathBuf::from("/tmp/repo"),
        output_dir: PathBuf::from("/tmp/output"),
        issue: crate::spec::IssueInput {
            title: None,
            body: None,
            body_path: None,
        },
        base_sha: None,
        head_sha: None,
        budget: crate::spec::EvalBudget::default(),
        source: None,
        campaign: None,
    };
    let record = crate::record::RunRecord::new(&prepared, run_arm);
    crate::record::write_compressed_record(path, &record).expect("write record");
}

fn prepared_run_with_repo_root(
    task_id: &str,
    repo_root: PathBuf,
) -> crate::spec::PreparedSingleRun {
    crate::spec::PreparedSingleRun {
        task_id: task_id.to_string(),
        repo_root,
        output_dir: PathBuf::from("/tmp/output"),
        issue: crate::spec::IssueInput {
            title: Some("Fix the thing".to_string()),
            body: Some("Body".to_string()),
            body_path: None,
        },
        base_sha: Some("deadbeef".to_string()),
        head_sha: None,
        budget: crate::spec::EvalBudget::default(),
        source: None,
        campaign: None,
    }
}

fn hold_env_lock() -> std::sync::MutexGuard<'static, ()> {
    crate::test_support::env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct EvalHomeGuard {
    old: Option<OsString>,
}

impl EvalHomeGuard {
    fn set_to(path: &Path) -> Self {
        let old = std::env::var_os("PLOKE_EVAL_HOME");
        unsafe {
            std::env::set_var("PLOKE_EVAL_HOME", path);
        }
        Self { old }
    }
}

impl Drop for EvalHomeGuard {
    fn drop(&mut self) {
        if let Some(old) = self.old.take() {
            unsafe {
                std::env::set_var("PLOKE_EVAL_HOME", old);
            }
        } else {
            unsafe {
                std::env::remove_var("PLOKE_EVAL_HOME");
            }
        }
    }
}

#[test]
fn protocol_llm_config_explicit_direct_google_selects_direct_google_route() {
    let cfg = protocol_llm_config(
        Some("google/gemini-2.5-flash".to_string()),
        Some(ModelRouteSource::DirectGoogle),
        Some("google".to_string()),
        120,
        1,
        400,
        ProtocolReasoningPolicy::default(),
    )
    .expect("protocol config");

    assert!(cfg.route_source.is_direct_google());
    assert!(cfg.provider_slug.is_none());
    assert_eq!(cfg.provider_display(), "google");
    assert_eq!(cfg.reasoning, ProtocolReasoningPolicy::disabled());
}

#[test]
fn protocol_llm_config_openrouter_provider_keeps_provider_pin() {
    let _lock = hold_env_lock();
    let tmp = tempdir().expect("tempdir");
    let _guard = EvalHomeGuard::set_to(tmp.path());

    let cfg = protocol_llm_config(
        Some("x-ai/grok-4-fast".to_string()),
        None,
        Some("xai".to_string()),
        120,
        1,
        400,
        ProtocolReasoningPolicy::default(),
    )
    .expect("protocol config");

    assert!(cfg.route_source.is_openrouter());
    assert_eq!(cfg.provider_slug.as_deref(), Some("xai"));
    assert_eq!(cfg.provider_display(), "xai");
}

#[test]
fn protocol_llm_config_explicit_openrouter_keeps_google_ai_studio_pin() {
    let cfg = protocol_llm_config(
        Some("google/gemini-3.5-flash".to_string()),
        Some(ModelRouteSource::OpenRouter),
        Some("google-ai-studio".to_string()),
        120,
        1,
        400,
        ProtocolReasoningPolicy::default(),
    )
    .expect("protocol config");

    assert!(cfg.route_source.is_openrouter());
    assert_eq!(cfg.provider_slug.as_deref(), Some("google-ai-studio"));
    assert_eq!(cfg.provider_display(), "google-ai-studio");
}

#[test]
fn protocol_llm_config_ignores_openrouter_preference_for_direct_google_registry_row() {
    let _lock = hold_env_lock();
    let tmp = tempdir().expect("tempdir");
    let _guard = EvalHomeGuard::set_to(tmp.path());
    let models_dir = tmp.path().join("models");
    fs::create_dir_all(&models_dir).expect("models dir");
    fs::write(
        models_dir.join("registry.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "data": [{
                "id": "google/gemini-3.5-flash",
                "name": "gemini-3.5-flash",
                "created": 0,
                "description": "Direct Google test row",
                "architecture": {
                    "input_modalities": ["text"],
                    "modality": "text->text",
                    "output_modalities": ["text"],
                    "tokenizer": "Gemini"
                },
                "top_provider": {
                    "is_moderated": false,
                    "context_length": null,
                    "max_completion_tokens": null
                },
                "pricing": {
                    "prompt": 0.0,
                    "completion": 0.0
                },
                "canonical_slug": "google/gemini-3.5-flash",
                "context_length": 1048576,
                "hugging_face_id": null,
                "per_request_limits": null,
                "supported_parameters": ["tools"],
                "route_source": "direct_google"
            }]
        }))
        .expect("registry json"),
    )
    .expect("write registry");
    fs::write(
        models_dir.join("provider-preferences.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "selected_providers": {
                "google/gemini-3.5-flash": {
                    "slug": "google-ai-studio"
                }
            }
        }))
        .expect("provider prefs json"),
    )
    .expect("write provider prefs");

    let cfg = protocol_llm_config(
        Some("google/gemini-3.5-flash".to_string()),
        None,
        None,
        120,
        1,
        400,
        ProtocolReasoningPolicy::default(),
    )
    .expect("protocol config");

    assert!(cfg.route_source.is_direct_google());
    assert!(cfg.provider_slug.is_none());
    assert_eq!(cfg.provider_display(), "google");
    assert_eq!(cfg.reasoning, ProtocolReasoningPolicy::disabled());
}

#[test]
fn provider_current_reports_google_for_direct_google_registry_row() {
    let _lock = hold_env_lock();
    let tmp = tempdir().expect("tempdir");
    let _guard = EvalHomeGuard::set_to(tmp.path());
    let models_dir = tmp.path().join("models");
    fs::create_dir_all(&models_dir).expect("models dir");
    fs::write(
        models_dir.join("registry.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "data": [{
                "id": "google/gemini-3.5-flash",
                "name": "gemini-3.5-flash",
                "created": 0,
                "description": "Direct Google test row",
                "architecture": {
                    "input_modalities": ["text"],
                    "modality": "text->text",
                    "output_modalities": ["text"],
                    "tokenizer": "Gemini"
                },
                "top_provider": {
                    "is_moderated": false,
                    "context_length": null,
                    "max_completion_tokens": null
                },
                "pricing": {
                    "prompt": 0.0,
                    "completion": 0.0
                },
                "canonical_slug": "google/gemini-3.5-flash",
                "context_length": 1048576,
                "hugging_face_id": null,
                "per_request_limits": null,
                "supported_parameters": ["tools"],
                "route_source": "direct_google"
            }]
        }))
        .expect("registry json"),
    )
    .expect("write registry");
    fs::write(
        models_dir.join("provider-preferences.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "selected_providers": {
                "google/gemini-3.5-flash": {
                    "slug": "google-ai-studio"
                }
            }
        }))
        .expect("provider prefs json"),
    )
    .expect("write provider prefs");

    let (model, provider) = current_provider_for_model(Some("google/gemini-3.5-flash".to_string()))
        .expect("current provider");

    assert_eq!(model.to_string(), "google/gemini-3.5-flash");
    let provider = provider.expect("direct Google provider sentinel");
    assert_eq!(provider.slug.as_str(), "google");
}

#[test]
fn protocol_llm_config_preserves_explicit_direct_google_reasoning_omit() {
    let cfg = protocol_llm_config(
        Some("google/gemini-3.5-flash".to_string()),
        Some(ModelRouteSource::DirectGoogle),
        Some("google".to_string()),
        120,
        1,
        400,
        ProtocolReasoningPolicy::omit(),
    )
    .expect("protocol config");

    assert!(cfg.route_source.is_direct_google());
    assert_eq!(cfg.reasoning, ProtocolReasoningPolicy::omit());
}

#[test]
fn protocol_llm_config_explicit_direct_google_rejects_openrouter_provider_pin() {
    let err = protocol_llm_config(
        Some("google/gemini-3.5-flash".to_string()),
        Some(ModelRouteSource::DirectGoogle),
        Some("google-ai-studio".to_string()),
        120,
        1,
        400,
        ProtocolReasoningPolicy::default(),
    )
    .expect_err("direct Google must reject OpenRouter provider pin");

    assert!(
        err.to_string()
            .contains("direct Google route does not accept OpenRouter provider")
    );
}

#[test]
fn parent_patcher_selection_ignores_openrouter_preference_for_direct_google_registry_row() {
    let _lock = hold_env_lock();
    let tmp = tempdir().expect("tempdir");
    let _guard = EvalHomeGuard::set_to(tmp.path());
    let models_dir = tmp.path().join("models");
    fs::create_dir_all(&models_dir).expect("models dir");
    fs::write(
        models_dir.join("registry.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "data": [{
                "id": "google/gemini-3.5-flash",
                "name": "gemini-3.5-flash",
                "created": 0,
                "description": "Direct Google test row",
                "architecture": {
                    "input_modalities": ["text"],
                    "modality": "text->text",
                    "output_modalities": ["text"],
                    "tokenizer": "Gemini"
                },
                "top_provider": {
                    "is_moderated": false,
                    "context_length": null,
                    "max_completion_tokens": null
                },
                "pricing": {
                    "prompt": 0.0,
                    "completion": 0.0
                },
                "canonical_slug": "google/gemini-3.5-flash",
                "context_length": 1048576,
                "hugging_face_id": null,
                "per_request_limits": null,
                "supported_parameters": ["tools"],
                "route_source": "direct_google"
            }]
        }))
        .expect("registry json"),
    )
    .expect("write registry");
    fs::write(
        models_dir.join("provider-preferences.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "selected_providers": {
                "google/gemini-3.5-flash": {
                    "slug": "google-ai-studio"
                }
            }
        }))
        .expect("provider prefs json"),
    )
    .expect("write provider prefs");
    let model_id: ModelId = "google/gemini-3.5-flash".parse().expect("model id");
    save_parent_patcher_model(&model_id).expect("save parent patcher model");

    let selection = load_parent_patcher_model_selection().expect("parent patcher selection");

    assert!(matches!(
        selection.router(),
        ploke_llm::router_only::RouterVariants::Google(_)
    ));
    assert!(selection.provider().is_none());
}

#[test]
fn headless_model_selection_explicit_direct_google_rejects_openrouter_provider_pin() {
    let _lock = hold_env_lock();
    let tmp = tempdir().expect("tempdir");
    let _guard = EvalHomeGuard::set_to(tmp.path());
    let models_dir = tmp.path().join("models");
    fs::create_dir_all(&models_dir).expect("models dir");
    fs::write(
        models_dir.join("registry.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "data": [{
                "id": "google/gemini-3.5-flash",
                "name": "gemini-3.5-flash",
                "created": 0,
                "description": "Direct Google test row",
                "architecture": {
                    "input_modalities": ["text"],
                    "modality": "text->text",
                    "output_modalities": ["text"],
                    "tokenizer": "Gemini"
                },
                "top_provider": {
                    "is_moderated": false,
                    "context_length": null,
                    "max_completion_tokens": null
                },
                "pricing": {
                    "prompt": 0.0,
                    "completion": 0.0
                },
                "canonical_slug": "google/gemini-3.5-flash",
                "context_length": 1048576,
                "hugging_face_id": null,
                "per_request_limits": null,
                "supported_parameters": ["tools"],
                "route_source": "direct_google"
            }]
        }))
        .expect("registry json"),
    )
    .expect("write registry");
    let model_id: ModelId = "google/gemini-3.5-flash".parse().expect("model id");
    let provider = ProviderKey::new("google-ai-studio").expect("provider key");

    let err = headless_model_selection(model_id, Some(provider))
        .expect_err("explicit OpenRouter provider must reject direct Google");

    assert!(
        err.to_string()
            .contains("does not accept OpenRouter provider")
    );
}

fn procedure_summary(
    complete_total: usize,
    missing_total: usize,
) -> crate::closure::ProcedureClosureSummary {
    crate::closure::ProcedureClosureSummary {
        expected_total: complete_total + missing_total,
        complete_total,
        failed_total: 0,
        missing_total,
        incompatible_total: 0,
        partial_total: 0,
        ineligible_total: 0,
    }
}

fn protocol_summary(
    status: ClosureClass,
    call_review_missing: usize,
    segment_review_missing: usize,
    last_transition_at: Option<&str>,
) -> crate::closure::ProtocolClosureSummary {
    let mut status_by_procedure = BTreeMap::new();
    status_by_procedure.insert(
        "tool-call-intent-segments".to_string(),
        procedure_summary(1, 0),
    );
    status_by_procedure.insert(
        "tool-call-review".to_string(),
        procedure_summary(usize::from(call_review_missing == 0), call_review_missing),
    );
    status_by_procedure.insert(
        "tool-call-segment-review".to_string(),
        procedure_summary(
            usize::from(segment_review_missing == 0),
            segment_review_missing,
        ),
    );

    crate::closure::ProtocolClosureSummary {
        expected_total: 1,
        full_total: usize::from(matches!(status, ClosureClass::Complete)),
        partial_total: usize::from(matches!(status, ClosureClass::Partial)),
        failed_total: 0,
        missing_total: 0,
        incompatible_total: 0,
        ineligible_total: 0,
        in_progress_total: 0,
        status,
        required_procedures: vec![
            "tool-call-intent-segments".to_string(),
            "tool-call-review".to_string(),
            "tool-call-segment-review".to_string(),
        ],
        status_by_procedure,
        last_transition_at: last_transition_at.map(str::to_string),
    }
}

fn protocol_report(
    before: crate::closure::ProtocolClosureSummary,
    after: crate::closure::ProtocolClosureSummary,
    failures: Vec<String>,
) -> ClosureAdvanceProtocolReport {
    ClosureAdvanceProtocolReport {
        campaign_id: CampaignId::from("campaign-google"),
        dry_run: false,
        before,
        after,
        selected_runs: vec![ProtocolRunPlan {
            instance_id: "BurntSushi__ripgrep-2209".to_string(),
            segmentation_needed: false,
            missing_call_indices: vec![0, 1, 2, 3, 4],
            missing_segment_indices: vec![0, 1, 2],
        }],
        executed_runs: 0,
        segmentations_created: 0,
        call_reviews_created: 0,
        segment_reviews_created: 0,
        failures,
    }
}

#[test]
fn failed_protocol_report_with_unchanged_closure_is_no_progress() {
    let before = protocol_summary(ClosureClass::Partial, 1, 1, Some("2026-05-22T00:20:31Z"));
    let after = before.clone();
    let report = protocol_report(before, after, vec!["HTTP status 429".to_string()]);

    assert!(!protocol_report_made_progress(&report));
    assert!(!protocol_report_allows_continue(&report));
    assert!(format_remaining_protocol_work(&report.after).contains("tool-call-review"));
    assert!(format_protocol_selected_runs(&report.selected_runs).contains("missing_calls=5"));
}

#[test]
fn unchanged_incomplete_protocol_report_without_failures_is_blocked() {
    let before = protocol_summary(ClosureClass::Partial, 1, 1, Some("2026-05-22T00:20:31Z"));
    let after = before.clone();
    let report = protocol_report(before, after, Vec::new());

    assert!(!protocol_report_allows_continue(&report));
}

#[test]
fn failed_protocol_report_with_changed_closure_counts_as_progress() {
    let before = protocol_summary(ClosureClass::Partial, 1, 1, Some("2026-05-22T00:20:31Z"));
    let after = protocol_summary(ClosureClass::Partial, 0, 1, Some("2026-05-22T00:21:31Z"));
    let report = protocol_report(before, after, vec!["later review failed".to_string()]);

    assert!(protocol_report_made_progress(&report));
    assert!(protocol_report_allows_continue(&report));
}

// regr:jsonretry:22-05-26_04-26
#[test]
fn intent_segmentation_truncated_json_parse_is_retryable() {
    let err = ploke_protocol::SequenceError::Second(ploke_protocol::MergeError::Branches(
        ploke_protocol::FanOutError::Right(ploke_protocol::ProtocolLlmError::ParseJson {
            detail: "EOF while parsing a string at line 9 column 48".to_string(),
            content: r#"{"segments":[{"rationale":"The agent repeatedly reads "#.to_string(),
        }),
    ));

    assert!(is_retryable_intent_segmentation_error(&err));
}

// regr:jsonretry:24-05-26_15-35
#[test]
fn tool_call_review_malformed_json_parse_is_retryable_for_all_judgment_branches() {
    let usefulness: review::ToolCallReviewError = ploke_protocol::SequenceError::Second(
        ploke_protocol::MergeError::Branches(ploke_protocol::FanOutError::Left(
            ploke_protocol::FanOutError::Left(protocol_json_parse_error()),
        )),
    );
    let redundancy: review::ToolCallReviewError = ploke_protocol::SequenceError::Second(
        ploke_protocol::MergeError::Branches(ploke_protocol::FanOutError::Left(
            ploke_protocol::FanOutError::Right(protocol_json_parse_error()),
        )),
    );
    let recoverability: review::ToolCallReviewError =
        ploke_protocol::SequenceError::Second(ploke_protocol::MergeError::Branches(
            ploke_protocol::FanOutError::Right(protocol_json_parse_error()),
        ));
    let missing_content: review::ToolCallReviewError = ploke_protocol::SequenceError::Second(
        ploke_protocol::MergeError::Branches(ploke_protocol::FanOutError::Left(
            ploke_protocol::FanOutError::Left(ploke_protocol::ProtocolLlmError::MissingContent),
        )),
    );

    assert!(is_retryable_local_analysis_review_error(&usefulness));
    assert!(is_retryable_local_analysis_review_error(&redundancy));
    assert!(is_retryable_local_analysis_review_error(&recoverability));
    assert!(!is_retryable_local_analysis_review_error(&missing_content));
}

fn protocol_json_parse_error() -> ploke_protocol::ProtocolLlmError {
    ploke_protocol::ProtocolLlmError::ParseJson {
        detail: "expected `,` or `}` at line 5 column 3".to_string(),
        content: "{ malformed protocol review json }".to_string(),
    }
}

#[test]
fn eval_closure_repo_cache_override_accepts_child_owned_run_roots() {
    let tmp = tempdir().expect("tempdir");
    let repo_cache = tmp
        .path()
        .join("node")
        .join("instance-targets")
        .join(&CampaignId::from("campaign"));
    let repo_root = repo_cache.join("BurntSushi").join("ripgrep");
    fs::create_dir_all(&repo_root).expect("repo root");

    let run = prepared_run_with_repo_root(
        "BurntSushi__ripgrep-2209",
        repo_root.canonicalize().expect("canonical repo root"),
    );

    ensure_prepared_runs_under_repo_cache(&[run], &repo_cache)
        .expect("child-owned root should satisfy repo cache invariant");
}

#[test]
fn eval_closure_repo_cache_override_rejects_shared_cache_escape() {
    let tmp = tempdir().expect("tempdir");
    let repo_cache = tmp
        .path()
        .join("node")
        .join("instance-targets")
        .join(&CampaignId::from("campaign"));
    let shared_root = tmp.path().join("shared").join("BurntSushi").join("ripgrep");
    fs::create_dir_all(&repo_cache).expect("repo cache");
    fs::create_dir_all(&shared_root).expect("shared root");

    let run = prepared_run_with_repo_root(
        "BurntSushi__ripgrep-2209",
        shared_root.canonicalize().expect("canonical shared root"),
    );

    let err = ensure_prepared_runs_under_repo_cache(&[run], &repo_cache)
        .expect_err("shared cache root should be rejected");
    let text = err.to_string();
    assert!(
        text.contains("escaped repo cache override"),
        "error should explain repo-cache escape: {text}"
    );
}

#[test]
fn repo_cache_clone_preflight_accepts_child_owned_cache() {
    let tmp = tempdir().expect("tempdir");
    let source = tmp.path().join("repos").join("BurntSushi").join("ripgrep");
    let repo_cache = tmp
        .path()
        .join("node")
        .join("instance-targets")
        .join(&CampaignId::from("campaign"));
    fs::create_dir_all(&source).expect("source repo");
    fs::create_dir_all(&repo_cache).expect("repo cache");

    let target = ensure_repo_cache_clone_preflight("BurntSushi", "ripgrep", &source, &repo_cache)
        .expect("child-owned clone target should be accepted");

    assert_eq!(
        target,
        repo_cache
            .canonicalize()
            .expect("canonical cache")
            .join("BurntSushi")
            .join("ripgrep")
    );
}

#[test]
fn repo_cache_clone_preflight_rejects_shared_cache_root() {
    let tmp = tempdir().expect("tempdir");
    let repo_cache = tmp.path().join("repos");
    let source = repo_cache.join("BurntSushi").join("ripgrep");
    fs::create_dir_all(&source).expect("source repo");

    let err = ensure_repo_cache_clone_preflight("BurntSushi", "ripgrep", &source, &repo_cache)
        .expect_err("shared repo cache must be rejected before clone/remove work");
    let text = err.to_string();
    assert!(
        text.contains("contains shared source repo"),
        "error should explain shared-cache overlap: {text}"
    );
}

#[test]
fn repo_cache_clone_preflight_rejects_path_components() {
    let tmp = tempdir().expect("tempdir");
    let source = tmp.path().join("repos").join("BurntSushi").join("ripgrep");
    let repo_cache = tmp
        .path()
        .join("node")
        .join("instance-targets")
        .join(&CampaignId::from("campaign"));
    fs::create_dir_all(&source).expect("source repo");
    fs::create_dir_all(&repo_cache).expect("repo cache");

    let err = ensure_repo_cache_clone_preflight("BurntSushi", "../ripgrep", &source, &repo_cache)
        .expect_err("path components should not be accepted as repo names");
    let text = err.to_string();
    assert!(
        text.contains("invalid repo component"),
        "error should explain invalid repo component: {text}"
    );
}

#[test]
fn eval_closure_repo_cache_override_rejects_dotdot_spelled_escape() {
    let tmp = tempdir().expect("tempdir");
    let repo_cache = tmp
        .path()
        .join("node")
        .join("instance-targets")
        .join(&CampaignId::from("campaign"));
    let shared_root = tmp.path().join("shared").join("BurntSushi").join("ripgrep");
    fs::create_dir_all(&repo_cache).expect("repo cache");
    fs::create_dir_all(&shared_root).expect("shared root");

    let escaped_spelling = repo_cache
        .join("..")
        .join("..")
        .join("..")
        .join("shared")
        .join("BurntSushi")
        .join("ripgrep");
    let run = prepared_run_with_repo_root("BurntSushi__ripgrep-2209", escaped_spelling);

    let err = ensure_prepared_runs_under_repo_cache(&[run], &repo_cache)
        .expect_err("canonicalized dotdot escape should be rejected");
    let text = err.to_string();
    assert!(
        text.contains("escaped repo cache override"),
        "error should explain repo-cache escape: {text}"
    );
}

#[derive(Debug, Deserialize)]
struct LiveGoogleJsonOk {
    ok: bool,
    route: String,
}

#[cfg(feature = "live_api_tests")]
fn is_google_protocol_quota_error(error: &ploke_protocol::ProtocolLlmError) -> bool {
    let text = format!("{error:?}");
    text.contains("RESOURCE_EXHAUSTED") || text.contains("429")
}

#[tokio::test]
#[cfg(feature = "live_api_tests")]
#[ignore = "live Google API test for ploke-eval protocol JSON route configuration"]
async fn live_google_protocol_json_adjudication_uses_direct_route_success_or_quota() {
    crate::test_support::install_default_google_route_env();
    let model_id = std::env::var("PLOKE_EVAL_LIVE_GOOGLE_MODEL_ID")
        .or_else(|_| std::env::var("PLOKE_LIVE_GOOGLE_CHAT_MODEL"))
        .unwrap_or_else(|_| "google/gemini-2.5-flash".to_string());
    let model_id = if model_id.contains('/') {
        model_id
    } else {
        format!("google/{model_id}")
    };
    let cfg = protocol_llm_config(
        Some(model_id),
        Some(ModelRouteSource::DirectGoogle),
        Some("google".to_string()),
        120,
        1,
        128,
        ProtocolReasoningPolicy::default(),
    )
    .expect("Google protocol config");
    assert!(cfg.route_source.is_direct_google());
    assert!(cfg.provider_slug.is_none());
    assert_eq!(cfg.provider_display(), "google");

    let prompt = ploke_protocol::JsonChatPrompt {
        system: "Return JSON only. Do not use markdown.".to_string(),
        user: "Return exactly this JSON object: {\"ok\":true,\"route\":\"google\"}".to_string(),
    };
    let client = reqwest::Client::new();
    let result =
        match ploke_protocol::adjudicate_json::<LiveGoogleJsonOk>(&client, &cfg, &prompt).await {
            Ok(result) => result,
            Err(error) if is_google_protocol_quota_error(&error) => {
                println!("live Google protocol JSON route reached Google quota response");
                return;
            }
            Err(error) => panic!("live Google protocol JSON adjudication failed: {error:?}"),
        };

    assert!(result.parsed.ok);
    assert_eq!(result.parsed.route, "google");
    assert!(result.response.model.contains("gemini"));
    println!("live Google protocol JSON route returned sentinel JSON");
}

#[tokio::test]
#[cfg(feature = "live_api_tests")]
#[ignore = "live Google API test for Prototype 1 protocol model override routing"]
async fn live_google_protocol_override_uses_direct_route_success_or_quota() {
    if !crate::test_support::live_google_env_or_skip(
        "live_google_protocol_override_uses_direct_route_success_or_quota",
    )
    .await
    {
        return;
    }
    crate::test_support::install_default_google_route_env();
    let model_id = std::env::var("PLOKE_EVAL_LIVE_GOOGLE_MODEL_ID")
        .or_else(|_| std::env::var("PLOKE_LIVE_GOOGLE_CHAT_MODEL"))
        .unwrap_or_else(|_| "google/gemini-2.5-flash".to_string());
    let model_id = if model_id.contains('/') {
        model_id
    } else {
        format!("google/{model_id}")
    };
    let config = ResolvedCampaignConfig {
        campaign_id: CampaignId::from("live-google-protocol-override"),
        benchmark_family: BenchmarkFamily::MultiSweBenchRust,
        dataset_sources: Vec::new(),
        model_id: "anthropic/claude-3.5-sonnet".to_string(),
        provider_slug: Some("anthropic".to_string()),
        route_source: ModelRouteSource::OpenRouter,
        required_procedures: Vec::new(),
        instances_root: PathBuf::from("/tmp/ploke-live-google-protocol-override/instances"),
        batches_root: PathBuf::from("/tmp/ploke-live-google-protocol-override/batches"),
        eval: Default::default(),
        protocol: ProtocolCampaignPolicy {
            model_id: Some(model_id),
            provider_slug: Some("google".to_string()),
            route_source: Some(ModelRouteSource::DirectGoogle),
            max_tokens: 120,
            tool_review_parallelism: 1,
            ..ProtocolCampaignPolicy::default()
        },
        framework: crate::spec::FrameworkConfig::default(),
    };
    let cfg = protocol_llm_config(
        Some(config.protocol.model_id_for(&config.model_id)),
        config.protocol.route_source_for(config.route_source),
        config
            .protocol
            .provider_slug_for(config.provider_slug.as_deref()),
        120,
        1,
        128,
        config.protocol.reasoning,
    )
    .expect("Google protocol config from override policy");
    assert!(cfg.route_source.is_direct_google());
    assert!(cfg.provider_slug.is_none());
    assert_eq!(cfg.provider_display(), "google");

    let prompt = ploke_protocol::JsonChatPrompt {
        system: "Return JSON only. Do not use markdown.".to_string(),
        user: "Return exactly this JSON object: {\"ok\":true,\"route\":\"google\"}".to_string(),
    };
    let client = reqwest::Client::new();
    let result =
        match ploke_protocol::adjudicate_json::<LiveGoogleJsonOk>(&client, &cfg, &prompt).await {
            Ok(result) => result,
            Err(error) if is_google_protocol_quota_error(&error) => {
                println!("live Google protocol override route reached Google quota response");
                return;
            }
            Err(error) => panic!("live Google protocol override failed: {error:?}"),
        };

    assert!(result.parsed.ok);
    assert_eq!(result.parsed.route, "google");
    assert!(result.response.model.contains("gemini"));
}

fn sample_run_intent(base: &Path, instances_root: &Path) -> RunIntent {
    RunIntent {
        task_id: "org__repo-1".to_string(),
        repo_root: base.join("repo"),
        storage_roots: RunStorageRoots::new(
            base.join("registries"),
            instances_root.join("org__repo-1").join("runs"),
        ),
        base_sha: Some("deadbeef".to_string()),
        budget: crate::spec::EvalBudget::default(),
        model_id: Some("model".to_string()),
        provider_slug: Some("provider".to_string()),
        campaign_id: None,
        batch_id: None,
        run_arm_id: "structured-current-policy".to_string(),
        run_role: RegisteredRunRole::Treatment,
    }
}

fn register_attempt(
    eval_home: &Path,
    run_id: &str,
    run_arm: crate::runner::RunArm,
    finished_at: &str,
) -> RunRegistration {
    let instances_root = eval_home.join("instances");
    let mut intent = sample_run_intent(eval_home, &instances_root);
    intent.run_arm_id = run_arm.id.clone();
    intent.run_role = match run_arm.role {
        crate::runner::RunArmRole::Control => RegisteredRunRole::Control,
        crate::runner::RunArmRole::Treatment => RegisteredRunRole::Treatment,
    };
    let mut registration =
        RunRegistration::register_with_run_id(intent, run_id).expect("registration");
    registration.lifecycle.execution_status = RunExecutionStatus::Completed;
    registration.lifecycle.finished_at = Some(finished_at.to_string());
    std::fs::create_dir_all(&registration.artifacts.run_root).expect("run root");
    write_test_run_record(&registration.artifacts.record_path, run_arm);
    registration.persist().expect("persist registration");
    registration
}

#[test]
fn extracts_size_from_parameter_phrase() {
    let text = "Cogito v2 is a multilingual, instruction-tuned Mixture of Experts (MoE) large language model with 671 billion parameters.";
    assert_eq!(extract_model_size(text), Some("671B".to_string()));
}

#[test]
fn extracts_size_from_suffix_notation() {
    let text = "Meta's latest class of model (Llama 3.1) launched with a variety of sizes & flavors. This 405B instruct-tuned version is optimized for high quality dialogue usecases.";
    assert_eq!(extract_model_size(text), Some("405B".to_string()));
}

#[test]
fn formats_pricing_per_million_tokens() {
    assert_eq!(display_price_per_million(0.00000018), "$0.18/M");
    assert_eq!(display_price_per_million(0.00000059), "$0.59/M");
}

#[test]
fn default_batch_id_uses_dataset_key_and_specific() {
    let batch_id = default_batch_id(Some("ripgrep"), None, false, &[], &["2209".to_string()]);
    assert_eq!(batch_id, "ripgrep-2209");
}

#[test]
fn sanitize_batch_component_collapses_non_alnum_runs() {
    assert_eq!(
        sanitize_batch_component("BurntSushi/ripgrep:pr-2209"),
        "burntsushi-ripgrep-pr-2209"
    );
}

#[test]
fn render_messages_json_renders_empty_lists_as_json_array() {
    let messages: &[crate::record::ConversationMessage] = &[];
    assert_eq!(
        render_messages_json(messages).expect("render should succeed"),
        "[]"
    );
}

#[test]
fn render_messages_table_renders_compact_blocks() {
    let messages = vec![
        sample_message(MessageKind::System, "first line\nsecond line", None),
        sample_message(MessageKind::Assistant, "assistant response", Some("call-1")),
    ];

    let rendered = render_messages_table(&messages);
    assert!(rendered.contains("Message 1"));
    assert!(rendered.contains("role .......... system"));
    assert!(rendered.contains("content ....... first line"));
    assert!(rendered.contains("Message 2"));
    assert!(rendered.contains("role .......... assistant"));
    assert!(rendered.contains("tool call id ... call-1"));
    assert!(!rendered.contains("\"kind\""));
}

fn sample_tool_request(tool: &str, arguments: &str) -> crate::runner::ToolRequestRecord {
    crate::runner::ToolRequestRecord {
        request_id: "req-1".to_string(),
        parent_id: "parent-1".to_string(),
        call_id: "call-1".to_string(),
        tool: tool.to_string(),
        arguments: arguments.into(),
    }
}

fn sample_tool_call_completed(
    tool: &str,
    arguments: &str,
    summary: &str,
    fields: &[(&str, &str)],
) -> crate::record::ToolExecutionRecord {
    let mut payload = ploke_tui::tools::ToolUiPayload::new(
        ploke_tui::tools::ToolName::RequestCodeContext,
        ArcStr::from("call-1"),
        summary,
    );
    for (name, value) in fields {
        payload = payload.with_field(*name, *value);
    }

    crate::record::ToolExecutionRecord {
        request: sample_tool_request(tool, arguments),
        result: crate::record::ToolResult::Completed(crate::runner::ToolCompletedRecord {
            request_id: "req-1".to_string(),
            parent_id: "parent-1".to_string(),
            call_id: "call-1".to_string(),
            tool: tool.to_string(),
            content: "synthetic output".to_string(),
            ui_payload: Some(payload),
            latency_ms: 17,
        }),
        latency_ms: 17,
    }
}

#[test]
#[ignore = "diagnostic prompt dump for tool-call intent segmentation context"]
fn diagnostic_dump_tool_call_segmentation_prompt() {
    let record_path = PathBuf::from(
        "/home/brasides/.ploke-eval/instances/prototype1/prototype1-typed-bridge-test-1777120923237/treatments/branch-c766961d14708d45/instances/clap-rs__clap-3670/runs/run-1777136748312-structured-current-policy-a9cd20b3/record.json.gz",
    );
    let record = read_compressed_record(&record_path).expect("read diagnostic record");
    let sequence =
        build_tool_call_sequence_subject(&record).expect("build tool-call sequence subject");
    let context = ploke_protocol::SequenceReviewContext {
        sequence,
        signals: ploke_protocol::tool_calls::segment::derive_sequence_signals_for_diagnostics(
            &build_tool_call_sequence_subject(&record).expect("rebuild tool-call sequence subject"),
        ),
    };
    let rendered =
        ploke_protocol::tool_calls::segment::render_sequence_context_for_diagnostics(&context);

    eprintln!("SEGMENT_DIAG record_path={}", record_path.display());
    eprintln!("SEGMENT_DIAG rendered_chars={}", rendered.len());
    eprintln!("SEGMENT_DIAG total_calls={}", context.sequence.calls.len());
    for call in &context.sequence.calls {
        eprintln!(
            "SEGMENT_DIAG call={} tool={} kind={:?} failed={} summary_len={} args_len={} result_len={} search_term_len={} path_hint_len={}",
            call.index,
            call.tool_name,
            call.tool_kind,
            call.failed,
            call.summary.len(),
            call.args_preview.len(),
            call.result_preview.len(),
            call.search_term
                .as_ref()
                .map(|value| value.len())
                .unwrap_or(0),
            call.path_hint
                .as_ref()
                .map(|value| value.len())
                .unwrap_or(0),
        );
        eprintln!("SEGMENT_DIAG summary[{}]={}", call.index, call.summary);
        eprintln!("SEGMENT_DIAG args[{}]={}", call.index, call.args_preview);
        eprintln!(
            "SEGMENT_DIAG result[{}]={}",
            call.index, call.result_preview
        );
    }
    eprintln!("SEGMENT_DIAG rendered_begin");
    eprintln!("{rendered}");
    eprintln!("SEGMENT_DIAG rendered_end");
}

fn sample_tool_call_failed(
    tool: &str,
    arguments: &str,
    summary: &str,
    fields: &[(&str, &str)],
) -> crate::record::ToolExecutionRecord {
    let mut payload = ploke_tui::tools::ToolUiPayload::new(
        ploke_tui::tools::ToolName::NsPatch,
        ArcStr::from("call-2"),
        summary,
    );
    payload.error_code = Some(ploke_tui::tools::ToolErrorCode::InvalidFormat);
    for (name, value) in fields {
        payload = payload.with_field(*name, *value);
    }

    crate::record::ToolExecutionRecord {
        request: sample_tool_request(tool, arguments),
        result: crate::record::ToolResult::Failed(crate::runner::ToolFailedRecord {
            request_id: "req-2".to_string(),
            parent_id: "parent-2".to_string(),
            call_id: "call-2".to_string(),
            tool: Some(tool.to_string()),
            error: "synthetic error".to_string(),
            ui_payload: Some(payload),
            latency_ms: 31,
        }),
        latency_ms: 31,
    }
}

#[test]
fn patch_state_summary_reports_staged_only_completion_as_staged() {
    let tool_calls = vec![sample_tool_call_completed(
        "non_semantic_patch",
        r#"{"patches":[]}"#,
        "edit staged",
        &[("status", "pending"), ("staged", "1"), ("applied", "0")],
    )];

    assert_eq!(summarize_patch_state(&tool_calls), "staged");
}

#[test]
fn patch_state_summary_reports_actual_apply_as_applied() {
    let tool_calls = vec![sample_tool_call_completed(
        "non_semantic_patch",
        r#"{"patches":[]}"#,
        "edit applied",
        &[("status", "applied"), ("staged", "0"), ("applied", "1")],
    )];

    assert_eq!(summarize_patch_state(&tool_calls), "applied");
}

#[test]
fn patch_state_summary_reports_failed_only_as_failed() {
    let tool_calls = vec![sample_tool_call_failed(
        "non_semantic_patch",
        r#"{"patches":[]}"#,
        "malformed diff",
        &[("field", "diff")],
    )];

    assert_eq!(summarize_patch_state(&tool_calls), "failed");
}

#[test]
fn render_tool_loop_table_renders_compact_blocks() {
    let tool_calls = vec![
        sample_tool_call_completed(
            "request_code_context",
            r#"{"search_term":"Arg::with_name(\"iglob\")"}"#,
            "Context assembled",
            &[("returned", "10 snippets"), ("top score", "0.031")],
        ),
        sample_tool_call_failed(
            "non_semantic_patch",
            r#"{"patches":[{"file":"src/main.rs"}]}"#,
            "Send one patch per tool call",
            &[("field", "patches"), ("expected", "array length of 1")],
        ),
    ];

    let rendered = render_tool_loop_table(1, &tool_calls);
    assert!(rendered.contains("Turn 1"));
    assert!(rendered.contains("[0] request_code_context"));
    assert!(rendered.contains("input"));
    assert!(rendered.contains("status"));
    assert!(rendered.contains("summary"));
    assert!(rendered.contains("returned"));
    assert!(rendered.contains("top score"));
    assert!(rendered.contains("code ........ internal") || rendered.contains("code"));
    assert!(rendered.contains("field"));
    assert!(rendered.contains("expected"));
    assert!(!rendered.contains("\"arguments\""));
}

#[test]
fn tool_call_next_step_index_uses_first_real_index() {
    assert_eq!(tool_call_next_step_index(0), None);
    assert_eq!(tool_call_next_step_index(3), Some(0));
}

#[test]
fn prototype1_harness_attempt_parses_model_budget_overrides() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "prototype1-harness",
        "attempt",
        "--request",
        "/tmp/request-r1.json",
        "--model-id",
        "anthropic/claude-sonnet-4",
        "--provider",
        "anthropic",
        "--max-attempts",
        "1",
        "--timeout-secs",
        "180",
        "--format",
        "json",
    ])
    .expect("prototype1 harness attempt should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command:
                LoopSubcommand::Prototype1Harness(Prototype1HarnessCommand {
                    command: Prototype1HarnessSubcommand::Attempt(cmd),
                }),
        }) => {
            assert_eq!(cmd.request, PathBuf::from("/tmp/request-r1.json"));
            assert_eq!(cmd.model_id.as_deref(), Some("anthropic/claude-sonnet-4"));
            assert_eq!(cmd.provider.as_deref(), Some("anthropic"));
            assert_eq!(cmd.max_attempts, Some(1));
            assert_eq!(cmd.timeout_secs, Some(180));
            assert_eq!(cmd.format, InspectOutputFormat::Json);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn prototype1_harness_sweep_parses_parallel_lanes() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "prototype1-harness",
        "sweep",
        "--request",
        "/tmp/request-r1.json",
        "--request",
        "/tmp/request-r2.json",
        "--model-id",
        "anthropic/claude-sonnet-4",
        "--model-id",
        "openai/gpt-4.1",
        "--parallel",
        "2",
    ])
    .expect("prototype1 harness sweep should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command:
                LoopSubcommand::Prototype1Harness(Prototype1HarnessCommand {
                    command: Prototype1HarnessSubcommand::Sweep(cmd),
                }),
        }) => {
            assert_eq!(cmd.requests.len(), 2);
            assert_eq!(cmd.model_ids.len(), 2);
            assert_eq!(cmd.parallel, 2);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn run_single_agent_path_parses_under_run_tree() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "run",
        "single",
        "agent",
        "--instance",
        "BurntSushi__ripgrep-2209",
    ])
    .expect("run single agent should parse");

    match parsed.command {
        Command::Run(RunCommand {
            command:
                RunSubcommand::Single(RunSingleWorkflowCommand {
                    command: RunSingleWorkflowSubcommand::Agent(cmd),
                }),
        }) => assert_eq!(cmd.instance.as_deref(), Some("BurntSushi__ripgrep-2209")),
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn just_single_shortcut_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "just",
        "single",
        "--instance",
        "BurntSushi__ripgrep-2209",
    ])
    .expect("just single should parse");

    match parsed.command {
        Command::Just(JustCommand {
            command: JustSubcommand::Single(cmd),
        }) => assert_eq!(cmd.instance.as_deref(), Some("BurntSushi__ripgrep-2209")),
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn just_old_hyphenated_alias_still_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "just",
        "run-msb-agent-single",
        "--instance",
        "BurntSushi__ripgrep-2209",
    ])
    .expect("just should keep the old hyphenated alias as a migration path");

    match parsed.command {
        Command::Just(JustCommand {
            command: JustSubcommand::Single(cmd),
        }) => assert_eq!(cmd.instance.as_deref(), Some("BurntSushi__ripgrep-2209")),
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn run_list_parses_instance_selector() {
    let parsed =
        Cli::try_parse_from(["ploke-eval", "run", "list", "--instance", "sharkdp__fd-658"])
            .expect("run list should parse");

    match parsed.command {
        Command::Run(RunCommand {
            command: RunSubcommand::List(cmd),
        }) => assert_eq!(cmd.instance.as_deref(), Some("sharkdp__fd-658")),
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn select_attempt_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "select",
        "attempt",
        "3",
        "--instance",
        "sharkdp__fd-658",
    ])
    .expect("select attempt should parse");

    match parsed.command {
        Command::Select(SelectCommand {
            command: SelectSubcommand::Attempt(cmd),
        }) => {
            assert_eq!(cmd.attempt, 3);
            assert_eq!(cmd.instance.as_deref(), Some("sharkdp__fd-658"));
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn registry_show_parses_dataset_selector() {
    let parsed =
        Cli::try_parse_from(["ploke-eval", "registry", "show", "--dataset", "sharkdp__fd"])
            .expect("registry show should parse");

    match parsed.command {
        Command::Registry(RegistryCommand {
            command: RegistrySubcommand::Show(cmd),
        }) => assert_eq!(cmd.dataset, "sharkdp__fd"),
        other => panic!("unexpected command shape: {:?}", other),
    }
}

fn sample_target_registry() -> TargetRegistry {
    TargetRegistry {
        schema_version: crate::target_registry::TARGET_REGISTRY_SCHEMA_VERSION.to_string(),
        benchmark_family: BenchmarkFamily::MultiSweBenchRust,
        updated_at: "2026-04-21T00:00:00Z".to_string(),
        dataset_sources: vec![
            crate::target_registry::RegistryDatasetSource {
                key: Some("fd".to_string()),
                path: PathBuf::from("/tmp/sharkdp__fd_dataset.jsonl"),
                label: "sharkdp__fd".to_string(),
                url: Some("https://example.invalid/fd".to_string()),
            },
            crate::target_registry::RegistryDatasetSource {
                key: Some("ripgrep".to_string()),
                path: PathBuf::from("/tmp/BurntSushi__ripgrep_dataset.jsonl"),
                label: "BurntSushi__ripgrep".to_string(),
                url: Some("https://example.invalid/ripgrep".to_string()),
            },
        ],
        entries: vec![
            RegistryEntry {
                instance_id: "sharkdp__fd-497".to_string(),
                dataset_label: "sharkdp__fd".to_string(),
                repo_family: "sharkdp__fd".to_string(),
                source: crate::target_registry::RegistrySource {
                    dataset_path: PathBuf::from("/tmp/sharkdp__fd_dataset.jsonl"),
                    org: "sharkdp".to_string(),
                    repo: "fd".to_string(),
                    number: 497,
                    base_sha: "abc123".to_string(),
                },
                state: crate::target_registry::RegistryEntryState::Active,
            },
            RegistryEntry {
                instance_id: "sharkdp__fd-658".to_string(),
                dataset_label: "sharkdp__fd".to_string(),
                repo_family: "sharkdp__fd".to_string(),
                source: crate::target_registry::RegistrySource {
                    dataset_path: PathBuf::from("/tmp/sharkdp__fd_dataset.jsonl"),
                    org: "sharkdp".to_string(),
                    repo: "fd".to_string(),
                    number: 658,
                    base_sha: "def456".to_string(),
                },
                state: crate::target_registry::RegistryEntryState::Active,
            },
            RegistryEntry {
                instance_id: "BurntSushi__ripgrep-2209".to_string(),
                dataset_label: "BurntSushi__ripgrep".to_string(),
                repo_family: "BurntSushi__ripgrep".to_string(),
                source: crate::target_registry::RegistrySource {
                    dataset_path: PathBuf::from("/tmp/BurntSushi__ripgrep_dataset.jsonl"),
                    org: "BurntSushi".to_string(),
                    repo: "ripgrep".to_string(),
                    number: 2209,
                    base_sha: "fedcba".to_string(),
                },
                state: crate::target_registry::RegistryEntryState::Active,
            },
        ],
    }
}

#[test]
fn registry_dataset_view_filters_to_one_dataset_family() {
    let registry = sample_target_registry();
    let view = registry_dataset_view(&registry, "sharkdp__fd").expect("view should resolve");

    assert_eq!(view.dataset, "sharkdp__fd");
    assert_eq!(view.entries.len(), 2);
    assert_eq!(view.entries[0].instance_id, "sharkdp__fd-497");
    assert_eq!(view.entries[1].instance_id, "sharkdp__fd-658");
    assert_eq!(
        view.source_paths,
        vec![PathBuf::from("/tmp/sharkdp__fd_dataset.jsonl")]
    );
}

#[test]
fn registry_dataset_view_reports_available_datasets_on_miss() {
    let registry = sample_target_registry();
    let err = registry_dataset_view(&registry, "tokio-rs__tokio").expect_err("missing dataset");

    match err {
        PrepareError::InvalidBatchSelection { detail } => {
            assert!(detail.contains("tokio-rs__tokio"));
            assert!(detail.contains("sharkdp__fd"));
            assert!(detail.contains("BurntSushi__ripgrep"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn inspect_tool_calls_accepts_missing_record_selector() {
    Cli::try_parse_from(["ploke-eval", "inspect", "tool-calls"])
        .expect("inspect tool-calls should default to the most recent run");
}

#[test]
fn protocol_issue_detection_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "protocol",
        "issue-detection",
        "--instance",
        "tokio-rs__bytes-543",
    ])
    .expect("protocol issue-detection should parse");

    match parsed.command {
        Command::Protocol(ProtocolCommand {
            command: ProtocolSubcommand::IssueDetection(cmd),
        }) => assert_eq!(cmd.instance.as_deref(), Some("tokio-rs__bytes-543")),
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn inspect_issue_overview_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "inspect",
        "issue-overview",
        "--instance",
        "tokio-rs__bytes-543",
    ])
    .expect("inspect issue-overview should parse");

    match parsed.command {
        Command::Inspect(InspectCommand {
            command: InspectSubcommand::IssueOverview(cmd),
        }) => assert_eq!(cmd.instance.as_deref(), Some("tokio-rs__bytes-543")),
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_prototype1_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "prototype1",
        "--dataset-key",
        "clap-rs__clap",
        "--instance",
        "clap-rs__clap-3670",
        "--stop-after",
        "intervention-apply",
        "--dry-run",
    ])
    .expect("loop prototype1 should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1(cmd),
        }) => {
            assert_eq!(cmd.dataset_key.as_deref(), Some("clap-rs__clap"));
            assert_eq!(cmd.instance, vec!["clap-rs__clap-3670".to_string()]);
            assert_eq!(cmd.max_generations, 1);
            assert_eq!(cmd.max_total_nodes, 32);
            assert_eq!(cmd.min_children, 2);
            assert_eq!(cmd.max_children, 6);
            assert_eq!(
                cmd.child_schedule_mode,
                Prototype1ChildScheduleMode::FullBatch
            );
            assert!(cmd.require_keep_for_continuation);
            assert!(cmd.explore_from_rejected);
            assert_eq!(cmd.stop_after, Prototype1LoopStopAfter::InterventionApply);
            assert!(cmd.dry_run);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_prototype1_command_parses_continued_branch_source() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "prototype1",
        "--dataset-key",
        "clap-rs__clap",
        "--instance",
        "clap-rs__clap-3670",
        "--source-campaign",
        "prototype1-campaign",
        "--source-branch-id",
        "branch-123",
        "--stop-after",
        "compare",
    ])
    .expect("loop prototype1 continued-source form should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1(cmd),
        }) => {
            assert_eq!(
                cmd.source_campaign.as_ref().map(|id| id.as_str()),
                Some("prototype1-campaign")
            );
            assert_eq!(cmd.source_branch_id.as_deref(), Some("branch-123"));
            assert_eq!(cmd.stop_after, Prototype1LoopStopAfter::Compare);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_prototype1_setup_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "prototype1-setup",
        "--dataset-key",
        "clap-rs__clap",
        "--instance",
        "clap-rs__clap-3670",
        "--model-id",
        "x-ai/grok-4-fast",
        "--provider",
        "xai",
        "--embedding-model-id",
        "perplexity/pplx-embed-v1-4b",
        "--embedding-provider",
        "perplexity",
        "--campaign",
        "p1-clap",
        "--profile",
        "overnight-edit-surface",
    ])
    .expect("loop prototype1-setup should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1Setup(cmd),
        }) => {
            assert_eq!(cmd.dataset_key.as_deref(), Some("clap-rs__clap"));
            assert_eq!(cmd.instance, vec!["clap-rs__clap-3670".to_string()]);
            assert_eq!(cmd.model_id.as_deref(), Some("x-ai/grok-4-fast"));
            assert_eq!(cmd.provider.as_deref(), Some("xai"));
            assert_eq!(
                cmd.embedding_model_id.as_deref(),
                Some("perplexity/pplx-embed-v1-4b")
            );
            assert_eq!(cmd.embedding_provider.as_deref(), Some("perplexity"));
            assert_eq!(cmd.campaign.as_ref().map(|id| id.as_str()), Some("p1-clap"));
            assert_eq!(cmd.profile.as_deref(), Some("overnight-edit-surface"));
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_prototype1_state_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "prototype1-state",
        "--campaign",
        "prototype1-campaign",
        "--node-id",
        "branch-abc-g1",
        "--handoff-invocation",
        "/tmp/prototype1-successor.json",
        "--stop-after",
        "build",
        "--candidate-generator",
        "broad-harness-request",
    ])
    .expect("loop prototype1-state should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1State(cmd),
        }) => {
            assert_eq!(
                cmd.campaign.as_ref().map(|id| id.as_str()),
                Some("prototype1-campaign")
            );
            assert_eq!(cmd.node_id.as_deref(), Some("branch-abc-g1"));
            assert_eq!(
                cmd.handoff_invocation.as_deref(),
                Some(std::path::Path::new("/tmp/prototype1-successor.json"))
            );
            assert_eq!(cmd.stop_after, Prototype1StateStopAfter::Build);
            assert_eq!(
                cmd.candidate_generator,
                Prototype1CandidateGenerator::BroadHarnessRequest
            );
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_walk_use_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "walk",
        "use",
        "/tmp/parent",
        "--socket",
        "/tmp/walk.sock",
    ])
    .expect("loop walk use should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1StateWalk(cmd),
        }) => match cmd.command {
            Prototype1StateWalkSubcommand::Use(cmd) => {
                assert_eq!(cmd.repo_root, Some(PathBuf::from("/tmp/parent")));
                assert_eq!(cmd.socket, Some(PathBuf::from("/tmp/walk.sock")));
                assert_eq!(cmd.format, InspectOutputFormat::Table);
            }
            other => panic!("unexpected walk subcommand: {:?}", other),
        },
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_walk_start_ttl_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "walk",
        "start",
        "--repo-root",
        "/tmp/parent",
        "--ttl-secs",
        "60",
        "--until",
        "r5",
    ])
    .expect("loop walk start should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1StateWalk(cmd),
        }) => match cmd.command {
            Prototype1StateWalkSubcommand::Start(cmd) => {
                assert_eq!(cmd.repo_root, Some(PathBuf::from("/tmp/parent")));
                assert_eq!(cmd.ttl_secs, Some(60));
                assert!(!cmd.no_ttl);
                assert_eq!(cmd.until, WalkPhase::R5);
                assert_eq!(cmd.format, InspectOutputFormat::Table);
                assert!(!cmd.with_version);
            }
            other => panic!("unexpected walk subcommand: {:?}", other),
        },
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_walk_step_handoff_admission_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "walk",
        "step",
        "--repo-root",
        "/tmp/parent",
        "--until",
        "r13b",
        "--watch",
        "--allow",
        "git-changes",
    ])
    .expect("loop walk step should parse handoff admission flags");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1StateWalk(cmd),
        }) => match cmd.command {
            Prototype1StateWalkSubcommand::Step(cmd) => {
                assert_eq!(cmd.repo_root, Some(PathBuf::from("/tmp/parent")));
                assert_eq!(cmd.until, Some(WalkPhase::R13b));
                assert!(cmd.watch);
                assert_eq!(cmd.allow, vec!["git-changes".to_string()]);
            }
            other => panic!("unexpected walk subcommand: {:?}", other),
        },
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_walk_summary_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "walk",
        "summary",
        "--repo-root",
        "/tmp/parent",
        "--format",
        "json",
        "-v",
    ])
    .expect("loop walk summary should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1StateWalk(cmd),
        }) => match cmd.command {
            Prototype1StateWalkSubcommand::Summary(cmd) => {
                assert_eq!(cmd.repo_root, Some(PathBuf::from("/tmp/parent")));
                assert_eq!(cmd.format, InspectOutputFormat::Json);
                assert!(cmd.verbose);
            }
            other => panic!("unexpected walk subcommand: {:?}", other),
        },
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_walk_show_with_version_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "walk",
        "show",
        "--repo-root",
        "/tmp/parent",
        "--with-version",
    ])
    .expect("loop walk show should parse --with-version");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1StateWalk(cmd),
        }) => match cmd.command {
            Prototype1StateWalkSubcommand::Show(cmd) => {
                assert_eq!(cmd.control.repo_root, Some(PathBuf::from("/tmp/parent")));
                assert_eq!(cmd.control.format, InspectOutputFormat::Table);
                assert!(cmd.control.with_version);
                assert!(cmd.command.is_none());
            }
            other => panic!("unexpected walk subcommand: {:?}", other),
        },
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_walk_show_delta_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "walk",
        "show",
        "delta",
        "--verbose",
        "--no-color",
    ])
    .expect("loop walk show delta should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1StateWalk(cmd),
        }) => match cmd.command {
            Prototype1StateWalkSubcommand::Show(cmd) => match cmd.command {
                Some(Prototype1StateWalkShowSubcommand::Delta(delta)) => {
                    assert!(delta.verbose);
                    assert!(delta.no_color);
                }
                other => panic!("unexpected show subcommand: {:?}", other),
            },
            other => panic!("unexpected walk subcommand: {:?}", other),
        },
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_walk_llm_timeline_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "walk",
        "llm",
        "--repo-root",
        "/tmp/parent",
        "timeline",
        "--lane",
        "node-1",
        "--session-id",
        "session-1",
    ])
    .expect("loop walk llm timeline should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1StateWalk(cmd),
        }) => match cmd.command {
            Prototype1StateWalkSubcommand::Llm(cmd) => {
                assert_eq!(cmd.control.repo_root, Some(PathBuf::from("/tmp/parent")));
                match cmd.command {
                    Prototype1StateWalkLlmSubcommand::Timeline(timeline) => {
                        assert_eq!(timeline.lane.as_deref(), Some("node-1"));
                        assert_eq!(timeline.session_id.as_deref(), Some("session-1"));
                    }
                    other => panic!("unexpected llm subcommand: {:?}", other),
                }
            }
            other => panic!("unexpected walk subcommand: {:?}", other),
        },
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_walk_llm_prompt_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "walk",
        "llm",
        "prompt",
        "--lane",
        "node-1",
        "--step",
        "0",
        "--role",
        "user",
        "--message",
        "4",
        "--full",
        "--json",
    ])
    .expect("loop walk llm prompt should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1StateWalk(cmd),
        }) => match cmd.command {
            Prototype1StateWalkSubcommand::Llm(cmd) => match cmd.command {
                Prototype1StateWalkLlmSubcommand::Prompt(prompt) => {
                    assert_eq!(prompt.lane.as_deref(), Some("node-1"));
                    assert_eq!(prompt.step, Some(0));
                    assert_eq!(prompt.role.as_deref(), Some("user"));
                    assert_eq!(prompt.message, Some(4));
                    assert!(prompt.full);
                    assert!(prompt.json);
                }
                other => panic!("unexpected llm subcommand: {:?}", other),
            },
            other => panic!("unexpected walk subcommand: {:?}", other),
        },
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_walk_llm_protocol_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "walk",
        "llm",
        "protocol",
        "--lane",
        "node-1",
        "--session-id",
        "session-1",
        "--json",
    ])
    .expect("loop walk llm protocol should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1StateWalk(cmd),
        }) => match cmd.command {
            Prototype1StateWalkSubcommand::Llm(cmd) => match cmd.command {
                Prototype1StateWalkLlmSubcommand::Protocol(protocol) => {
                    assert_eq!(protocol.lane.as_deref(), Some("node-1"));
                    assert_eq!(protocol.session_id.as_deref(), Some("session-1"));
                    assert!(protocol.json);
                }
                other => panic!("unexpected llm subcommand: {:?}", other),
            },
            other => panic!("unexpected walk subcommand: {:?}", other),
        },
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_walk_llm_tool_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "walk",
        "llm",
        "tool",
        "--lane",
        "node-1",
        "--step",
        "11",
        "--call",
        "1",
        "--name",
        "non_semantic_patch",
        "--json",
    ])
    .expect("loop walk llm tool should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1StateWalk(cmd),
        }) => match cmd.command {
            Prototype1StateWalkSubcommand::Llm(cmd) => match cmd.command {
                Prototype1StateWalkLlmSubcommand::Tool(tool) => {
                    assert_eq!(tool.lane.as_deref(), Some("node-1"));
                    assert_eq!(tool.step, Some(11));
                    assert_eq!(tool.call, Some(1));
                    assert_eq!(tool.name.as_deref(), Some("non_semantic_patch"));
                    assert!(tool.json);
                }
                other => panic!("unexpected llm subcommand: {:?}", other),
            },
            other => panic!("unexpected walk subcommand: {:?}", other),
        },
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_walk_replay_and_back_commands_parse() {
    let replay = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "walk",
        "replay",
        "--repo-root",
        "/tmp/parent",
        "--index",
        "7",
    ])
    .expect("loop walk replay should parse");

    match replay.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1StateWalk(cmd),
        }) => match cmd.command {
            Prototype1StateWalkSubcommand::Replay(cmd) => {
                assert_eq!(cmd.control.repo_root, Some(PathBuf::from("/tmp/parent")));
                assert_eq!(cmd.index, Some(7));
                assert_eq!(cmd.tail, 3);
            }
            other => panic!("unexpected walk subcommand: {:?}", other),
        },
        other => panic!("unexpected command shape: {:?}", other),
    }

    let back = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "walk",
        "back",
        "--steps",
        "2",
        "--tail",
        "4",
    ])
    .expect("loop walk back should parse");

    match back.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1StateWalk(cmd),
        }) => match cmd.command {
            Prototype1StateWalkSubcommand::Back(cmd) => {
                assert_eq!(cmd.steps, 2);
                assert_eq!(cmd.tail, 4);
            }
            other => panic!("unexpected walk subcommand: {:?}", other),
        },
        other => panic!("unexpected command shape: {:?}", other),
    }

    let forward = Cli::try_parse_from(["ploke-eval", "loop", "walk", "forward"])
        .expect("loop walk forward should parse");

    match forward.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1StateWalk(cmd),
        }) => match cmd.command {
            Prototype1StateWalkSubcommand::Forward(cmd) => {
                assert_eq!(cmd.steps, 1);
                assert_eq!(cmd.tail, 3);
            }
            other => panic!("unexpected walk subcommand: {:?}", other),
        },
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_walk_branch_live_requires_explicit_provenance_capability() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "walk",
        "branch-live",
        "--repo-root",
        "/tmp/parent",
        "--reason",
        "debug from historical cursor",
        "--allow",
        "provenance-record",
    ])
    .expect("loop walk branch-live should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1StateWalk(cmd),
        }) => match cmd.command {
            Prototype1StateWalkSubcommand::BranchLive(cmd) => {
                assert_eq!(cmd.control.repo_root, Some(PathBuf::from("/tmp/parent")));
                assert_eq!(cmd.reason, "debug from historical cursor");
                assert_eq!(cmd.allow, vec!["provenance-record".to_string()]);
            }
            other => panic!("unexpected walk subcommand: {:?}", other),
        },
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_walk_serve_no_ttl_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "walk",
        "serve",
        "--repo-root",
        "/tmp/parent",
        "--no-ttl",
    ])
    .expect("loop walk serve should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1StateWalk(cmd),
        }) => match cmd.command {
            Prototype1StateWalkSubcommand::Serve(cmd) => {
                assert_eq!(cmd.repo_root, Some(PathBuf::from("/tmp/parent")));
                assert!(cmd.ttl_secs.is_none());
                assert!(cmd.no_ttl);
            }
            other => panic!("unexpected walk subcommand: {:?}", other),
        },
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_prototype1_doctor_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "prototype1-doctor",
        "--repo-root",
        "/tmp/repo",
        "--format",
        "json",
    ])
    .expect("loop prototype1-doctor should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1Doctor(cmd),
        }) => {
            assert_eq!(cmd.control.repo_root, Some(PathBuf::from("/tmp/repo")));
            assert_eq!(cmd.control.format, InspectOutputFormat::Json);
            assert!(!cmd.live_protocol_preflight);
            assert!(!cmd.headless_tui_setup_preflight);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_prototype1_doctor_live_protocol_preflight_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "prototype1-doctor",
        "--repo-root",
        "/tmp/repo",
        "--live-protocol-preflight",
    ])
    .expect("loop prototype1-doctor live preflight should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1Doctor(cmd),
        }) => {
            assert_eq!(cmd.control.repo_root, Some(PathBuf::from("/tmp/repo")));
            assert_eq!(cmd.control.format, InspectOutputFormat::Table);
            assert!(cmd.live_protocol_preflight);
            assert!(!cmd.headless_tui_setup_preflight);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_prototype1_doctor_headless_tui_setup_preflight_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "prototype1-doctor",
        "--repo-root",
        "/tmp/repo",
        "--headless-tui-setup-preflight",
    ])
    .expect("loop prototype1-doctor headless setup preflight should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1Doctor(cmd),
        }) => {
            assert_eq!(cmd.control.repo_root, Some(PathBuf::from("/tmp/repo")));
            assert_eq!(cmd.control.format, InspectOutputFormat::Table);
            assert!(!cmd.live_protocol_preflight);
            assert!(cmd.headless_tui_setup_preflight);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_prototype1_prompt_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "prototype1-prompt",
        "--repo-root",
        "/tmp/repo",
    ])
    .expect("loop prototype1-prompt should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1Prompt(cmd),
        }) => {
            assert_eq!(cmd.repo_root, Some(PathBuf::from("/tmp/repo")));
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_prototype1_continue_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "prototype1-continue",
        "--repo-root",
        "/tmp/repo",
    ])
    .expect("loop prototype1-continue should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1Continue(cmd),
        }) => {
            assert_eq!(cmd.repo_root, Some(PathBuf::from("/tmp/repo")));
            assert_eq!(cmd.format, InspectOutputFormat::Table);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_prototype1_step_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "prototype1-step",
        "--repo-root",
        "/tmp/repo",
    ])
    .expect("loop prototype1-step should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1Step(cmd),
        }) => {
            assert_eq!(cmd.repo_root, Some(PathBuf::from("/tmp/repo")));
            assert_eq!(cmd.format, InspectOutputFormat::Table);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_prototype1_runner_invocation_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "prototype1-runner",
        "--invocation",
        "/tmp/runtime.json",
        "--execute",
        "--format",
        "json",
    ])
    .expect("loop prototype1-runner --invocation should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1Runner(cmd),
        }) => {
            assert!(cmd.campaign.is_none());
            assert!(cmd.node_id.is_none());
            assert_eq!(cmd.invocation, Some(PathBuf::from("/tmp/runtime.json")));
            assert!(cmd.execute);
            assert!(!cmd.stop_on_error);
            assert_eq!(cmd.format, InspectOutputFormat::Json);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_prototype1_state_candidate_generator_defaults_to_broad_harness_surface() {
    let parsed = Cli::try_parse_from(["ploke-eval", "loop", "prototype1-state"])
        .expect("loop prototype1-state should parse with generator defaults");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1State(cmd),
        }) => {
            assert_eq!(
                cmd.candidate_generator,
                Prototype1CandidateGenerator::BroadHarnessRequest
            );
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_prototype1_state_identity_init_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "prototype1-state",
        "--campaign",
        "prototype1-campaign",
        "--node-id",
        "node-639a992e45ac3533",
        "--repo-root",
        "/tmp/repo",
        "--init-parent-identity",
        "--identity-branch",
        "prototype1-parent-gen0",
        "--identity-instance",
        "clap-rs__clap-3670",
    ])
    .expect("loop prototype1-state identity init should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1State(cmd),
        }) => {
            assert_eq!(
                cmd.campaign.as_ref().map(|id| id.as_str()),
                Some("prototype1-campaign")
            );
            assert_eq!(cmd.node_id.as_deref(), Some("node-639a992e45ac3533"));
            assert!(cmd.init_parent_identity);
            assert_eq!(
                cmd.identity_branch.as_deref(),
                Some("prototype1-parent-gen0")
            );
            assert_eq!(cmd.identity_instance.as_deref(), Some("clap-rs__clap-3670"));
            assert_eq!(
                cmd.repo_root.as_deref(),
                Some(std::path::Path::new("/tmp/repo"))
            );
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn history_score_alias_command_parses() {
    let parsed = Cli::try_parse_from(["ploke-eval", "history", "score"])
        .expect("history score alias should parse");

    match parsed.command {
        Command::History(HistoryCommand {
            command: HistorySubcommand::Scores(scores),
            ..
        }) => {
            assert_eq!(scores.format, InspectOutputFormat::Table);
            assert_eq!(scores.rows, 20);
            assert_eq!(scores.generation, None);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn history_score_selection_review_command_parses() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "history",
        "score-selection-review",
        "--campaign",
        "prototype1-campaign",
        "--format",
        "json",
        "--rows",
        "12",
        "--generation",
        "2",
    ])
    .expect("history score-selection-review should parse");

    match parsed.command {
        Command::History(HistoryCommand {
            campaign,
            command: HistorySubcommand::ScoreSelectionReview(review),
            ..
        }) => {
            assert_eq!(
                campaign.as_ref().map(|id| id.as_str()),
                Some("prototype1-campaign")
            );
            assert_eq!(review.format, InspectOutputFormat::Json);
            assert_eq!(review.rows, 12);
            assert_eq!(review.generation, Some(2));
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_prototype1_state_parent_checkout_form_parses() {
    let parsed = Cli::try_parse_from(["ploke-eval", "loop", "prototype1-state"])
        .expect("loop prototype1-state should allow parent checkout inference");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1State(cmd),
        }) => {
            assert_eq!(cmd.campaign, None);
            assert_eq!(cmd.node_id, None);
            assert_eq!(cmd.repo_root, None);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn loop_prototype1_command_parses_search_policy_flags() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "loop",
        "prototype1",
        "--dataset-key",
        "clap-rs__clap",
        "--instance",
        "clap-rs__clap-3670",
        "--max-generations",
        "5",
        "--max-total-nodes",
        "99",
        "--min-children",
        "3",
        "--max-children",
        "5",
        "--child-schedule-mode",
        "adaptive-batch",
        "--stop-on-first-keep",
        "--require-keep-for-continuation",
        "false",
        "--explore-from-rejected",
        "false",
    ])
    .expect("loop prototype1 search-policy form should parse");

    match parsed.command {
        Command::Loop(LoopCommand {
            command: LoopSubcommand::Prototype1(cmd),
        }) => {
            assert_eq!(cmd.max_generations, 5);
            assert_eq!(cmd.max_total_nodes, 99);
            assert_eq!(cmd.min_children, 3);
            assert_eq!(cmd.max_children, 5);
            assert_eq!(
                cmd.child_schedule_mode,
                Prototype1ChildScheduleMode::AdaptiveBatch
            );
            assert!(cmd.stop_on_first_keep);
            assert!(!cmd.require_keep_for_continuation);
            assert!(!cmd.explore_from_rejected);
            assert_eq!(cmd.stop_after, Prototype1LoopStopAfter::InterventionApply);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn resolve_record_path_defaults_to_last_run_record() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let eval_home = tmp.path().join("eval-home");
    let run_dir = eval_home.join("instances").join("demo-run");
    std::fs::create_dir_all(&run_dir).expect("run dir");
    crate::run_history::record_last_run_at(&eval_home, &run_dir).expect("record last run");

    let resolution = resolve_record_path_from_eval_home(None, None, None, eval_home)
        .expect("default record path should resolve");

    assert_eq!(resolution.record_path, run_dir.join("record.json.gz"));
}

#[test]
fn abbreviate_path_tail_keeps_filename_suffix() {
    let path = "/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/globset/src/lib.rs";
    let abbreviated = abbreviate_path_tail(path, 32);
    assert!(abbreviated.starts_with(".../"));
    assert!(abbreviated.ends_with("globset/src/lib.rs"));
}

#[test]
fn summarize_tool_inputs_prefers_parsed_fields() {
    let args: ToolArgumentsJson = r#"{"file":"/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/globset/src/lib.rs","start_line":80,"end_line":120}"#.into();
    let summary = summarize_tool_inputs("read_file", &args);
    assert!(summary.contains("lines=80-120"));
    assert!(summary.contains("file=.../globset/src/lib.rs"));
}

#[test]
fn render_tool_inputs_surfaces_derived_line_window() {
    let args: ToolArgumentsJson =
        r#"{"file":"/tmp/demo.rs","start_line":10,"end_line":24,"max_bytes":4096}"#.into();
    let lines = render_tool_inputs("read_file", &args);
    assert_eq!(lines[0], "lines (derived from start_line/end_line): 10-24");
    assert!(lines.iter().any(|line| line == "file: /tmp/demo.rs"));
    assert!(lines.iter().any(|line| line == "max_bytes: 4096"));
}

#[test]
fn tool_argument_string_extracts_legacy_search_code_query() {
    let args: ToolArgumentsJson = r#"{"query":"handle_request"}"#.into();

    assert_eq!(
        extract_argument_string("search_code", &args, &["search_term", "query"]).as_deref(),
        Some("handle_request")
    );
    assert_eq!(
        summarize_tool_inputs("search_code", &args),
        "query=handle_request"
    );
}

#[test]
fn tool_argument_string_extracts_legacy_query_codebase_search_term() {
    let args: ToolArgumentsJson = r#"{"search_term":"ToolRequestRecord"}"#.into();

    assert_eq!(
        extract_argument_string("query_codebase", &args, &["search_term", "query"]).as_deref(),
        Some("ToolRequestRecord")
    );
}

#[test]
fn render_payload_block_reports_exact_inspector_truncation_metadata() {
    let rendered = render_payload_block("abcdefghijklmnopqrstuvwxyz", 10);
    assert_eq!(rendered.text, "abc...wxyz");
    assert_eq!(rendered.raw_bytes, 26);
    assert_eq!(rendered.normalized_chars, 26);
    assert_eq!(rendered.shown_source_chars, 7);
    assert!(rendered.inspector_truncated);
    assert_eq!(
        rendered.inspector_truncation_note().as_deref(),
        Some(
            "Inspector display truncated the normalized payload: 7/26 source chars shown (+ ellipsis), 19 elided; stored raw payload is 26 bytes."
        )
    );
}

#[test]
fn inspect_tool_calls_accepts_positional_index() {
    let parsed = Cli::try_parse_from(["ploke-eval", "inspect", "tool-calls", "5"])
        .expect("tool-calls should accept a positional index");

    match parsed.command {
        Command::Inspect(InspectCommand {
            command: InspectSubcommand::ToolCalls(cmd),
        }) => assert_eq!(cmd.index, Some(5)),
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn inspect_tool_overview_accepts_campaign_and_tool() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "inspect",
        "tool-overview",
        "--campaign",
        "rust-baseline-grok4-xai",
        "--tool",
        "apply_code_edit",
    ])
    .expect("tool-overview should parse campaign and tool");

    match parsed.command {
        Command::Inspect(InspectCommand {
            command: InspectSubcommand::ToolOverview(cmd),
        }) => {
            assert_eq!(cmd.campaign, CampaignId::from("rust-baseline-grok4-xai"));
            assert_eq!(cmd.tool.as_deref(), Some("apply_code_edit"));
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn inspect_operational_accepts_metrics_alias() {
    let parsed = Cli::try_parse_from(["ploke-eval", "inspect", "metrics"])
        .expect("operational metrics should accept the metrics alias");

    match parsed.command {
        Command::Inspect(InspectCommand {
            command: InspectSubcommand::Operational(_),
        }) => {}
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn summarize_failure_reason_uses_first_non_empty_line() {
    let summary = summarize_failure_reason(
        "\napply_code_edit: No matching node found (strict+fallback)\nerror code: invalid_format\n",
    );
    assert_eq!(
        summary,
        "apply_code_edit: No matching node found (strict+fallback)"
    );
}

#[test]
fn tool_result_failure_projection_uses_structured_summary() {
    let summary = summarize_failure_reason(
        r#"{"message":"  apply_code_edit failed\nmissing protected surface  ","error":"fallback"}"#,
    );
    assert_eq!(summary, "apply_code_edit failed missing protected surface");
}

#[test]
fn tool_result_failure_projection_skips_non_string_fields() {
    let summary =
        summarize_failure_reason(r#"{"message":{"nested":true},"error":"fallback reason"}"#);
    assert_eq!(summary, "fallback reason");
}

#[test]
fn inspect_turn_accepts_positional_turn() {
    let parsed = Cli::try_parse_from(["ploke-eval", "inspect", "turn", "1"])
        .expect("turn should accept a positional turn number");

    match parsed.command {
        Command::Inspect(InspectCommand {
            command: InspectSubcommand::Turn(cmd),
        }) => {
            assert_eq!(cmd.turn, Some(1));
            assert_eq!(cmd.turn_flag, None);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn run_replay_turn_live_command_parses_cursor_and_workspace() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "run",
        "replay",
        "turn-live",
        "--run-dir",
        "/tmp/run",
        "--workspace",
        "/tmp/workspace",
        "--artifact-kind",
        "trace",
        "--artifact-path",
        "agent-turn-trace.json",
        "--event-index",
        "3",
        "--model-id",
        "openai/gpt-5",
        "--provider",
        "openrouter",
        "--max-attempts",
        "1",
        "--timeout-secs",
        "30",
        "--format",
        "json",
    ])
    .expect("run replay turn-live should parse");

    match parsed.command {
        Command::Run(RunCommand {
            command:
                RunSubcommand::Replay(RunReplayCommand {
                    command: RunReplaySubcommand::TurnLive(cmd),
                }),
        }) => {
            assert_eq!(cmd.run_dir, PathBuf::from("/tmp/run"));
            assert_eq!(cmd.workspace, PathBuf::from("/tmp/workspace"));
            assert_eq!(cmd.artifact_kind, ReplayTurnArtifactArg::Trace);
            assert_eq!(cmd.artifact_path, "agent-turn-trace.json");
            assert_eq!(cmd.event_index, 3);
            assert_eq!(cmd.through_response_index, None);
            assert!(!cmd.through_event);
            assert_eq!(cmd.tail, ReplayTailArg::Live);
            assert_eq!(cmd.branch_in, None);
            assert_eq!(cmd.branch_out, None);
            assert_eq!(cmd.model_id.as_deref(), Some("openai/gpt-5"));
            assert_eq!(cmd.provider.as_deref(), Some("openrouter"));
            assert_eq!(cmd.max_attempts, 1);
            assert_eq!(cmd.timeout_secs, 30);
            assert_eq!(cmd.format, InspectOutputFormat::Json);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn run_replay_turn_live_command_parses_response_prefix_and_stop_tail() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "run",
        "replay",
        "turn-live",
        "--run-dir",
        "/tmp/run",
        "--workspace",
        "/tmp/workspace",
        "--event-index",
        "3",
        "--through-response-index",
        "8",
        "--tail",
        "stop",
    ])
    .expect("run replay turn-live should parse response prefix and stop tail");

    match parsed.command {
        Command::Run(RunCommand {
            command:
                RunSubcommand::Replay(RunReplayCommand {
                    command: RunReplaySubcommand::TurnLive(cmd),
                }),
        }) => {
            assert_eq!(cmd.event_index, 3);
            assert_eq!(cmd.through_response_index, Some(8));
            assert!(!cmd.through_event);
            assert_eq!(cmd.tail, ReplayTailArg::Stop);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn run_replay_turn_live_command_parses_event_prefix() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "run",
        "replay",
        "turn-live",
        "--run-dir",
        "/tmp/run",
        "--workspace",
        "/tmp/workspace",
        "--event-index",
        "11",
        "--through-event",
    ])
    .expect("run replay turn-live should parse event prefix");

    match parsed.command {
        Command::Run(RunCommand {
            command:
                RunSubcommand::Replay(RunReplayCommand {
                    command: RunReplaySubcommand::TurnLive(cmd),
                }),
        }) => {
            assert_eq!(cmd.event_index, 11);
            assert!(cmd.through_event);
            assert_eq!(cmd.through_response_index, None);
            assert_eq!(cmd.tail, ReplayTailArg::Live);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn run_replay_turn_live_command_parses_live_step_branch_tape() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "run",
        "replay",
        "turn-live",
        "--run-dir",
        "/tmp/run",
        "--workspace",
        "/tmp/workspace",
        "--event-index",
        "11",
        "--through-event",
        "--tail",
        "live-step",
        "--branch-in",
        "/tmp/branch-in.json",
        "--branch-out",
        "/tmp/branch-out.json",
    ])
    .expect("run replay turn-live should parse live-step branch tape");

    match parsed.command {
        Command::Run(RunCommand {
            command:
                RunSubcommand::Replay(RunReplayCommand {
                    command: RunReplaySubcommand::TurnLive(cmd),
                }),
        }) => {
            assert_eq!(cmd.event_index, 11);
            assert!(cmd.through_event);
            assert_eq!(cmd.tail, ReplayTailArg::LiveStep);
            assert_eq!(cmd.branch_in, Some(PathBuf::from("/tmp/branch-in.json")));
            assert_eq!(cmd.branch_out, Some(PathBuf::from("/tmp/branch-out.json")));
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn run_replay_self_edit_live_command_parses_headless_result_source() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "run",
        "replay",
        "self-edit-live",
        "--request",
        "/tmp/request.json",
        "--result",
        "/tmp/result.headless-tui.json",
        "--workspace",
        "/tmp/workspace",
        "--event-index",
        "7",
        "--through-event",
        "--tail",
        "stop",
        "--format",
        "json",
    ])
    .expect("run replay self-edit-live should parse headless result source");

    match parsed.command {
        Command::Run(RunCommand {
            command:
                RunSubcommand::Replay(RunReplayCommand {
                    command: RunReplaySubcommand::SelfEditLive(cmd),
                }),
        }) => {
            assert_eq!(cmd.request, PathBuf::from("/tmp/request.json"));
            assert_eq!(
                cmd.result,
                Some(PathBuf::from("/tmp/result.headless-tui.json"))
            );
            assert_eq!(cmd.raw_full_response, None);
            assert_eq!(cmd.workspace, PathBuf::from("/tmp/workspace"));
            assert_eq!(cmd.event_index, Some(7));
            assert!(cmd.through_event);
            assert_eq!(cmd.through_response_index, None);
            assert_eq!(cmd.tail, ReplayTailArg::Stop);
            assert_eq!(cmd.format, InspectOutputFormat::Json);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn run_replay_self_edit_live_command_parses_raw_full_response_source() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "run",
        "replay",
        "self-edit-live",
        "--request",
        "/tmp/request.json",
        "--raw-full-response",
        "/tmp/llm_full_response.log",
        "--workspace",
        "/tmp/workspace",
        "--through-response-index",
        "15",
        "--tail",
        "stop",
    ])
    .expect("run replay self-edit-live should parse raw provider source");

    match parsed.command {
        Command::Run(RunCommand {
            command:
                RunSubcommand::Replay(RunReplayCommand {
                    command: RunReplaySubcommand::SelfEditLive(cmd),
                }),
        }) => {
            assert_eq!(cmd.request, PathBuf::from("/tmp/request.json"));
            assert_eq!(cmd.result, None);
            assert_eq!(
                cmd.raw_full_response,
                Some(PathBuf::from("/tmp/llm_full_response.log"))
            );
            assert_eq!(cmd.workspace, PathBuf::from("/tmp/workspace"));
            assert_eq!(cmd.event_index, None);
            assert_eq!(cmd.through_response_index, Some(15));
            assert_eq!(cmd.tail, ReplayTailArg::Stop);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn run_replay_self_edit_live_raw_rejects_event_index() {
    let err = Cli::try_parse_from([
        "ploke-eval",
        "run",
        "replay",
        "self-edit-live",
        "--request",
        "/tmp/request.json",
        "--raw-full-response",
        "/tmp/llm_full_response.log",
        "--workspace",
        "/tmp/workspace",
        "--event-index",
        "7",
    ])
    .expect_err("raw self-edit replay must reject headless event slicing");
    let err_text = err.to_string();
    assert!(
        err_text.contains("raw-full-response") || err_text.contains("event-index"),
        "error should explain source/slicing conflict: {err_text}"
    );
}

#[test]
fn run_replay_self_edit_live_raw_rejects_through_event() {
    let err = Cli::try_parse_from([
        "ploke-eval",
        "run",
        "replay",
        "self-edit-live",
        "--request",
        "/tmp/request.json",
        "--raw-full-response",
        "/tmp/llm_full_response.log",
        "--workspace",
        "/tmp/workspace",
        "--through-event",
    ])
    .expect_err("raw self-edit replay must reject headless event slicing");
    let err_text = err.to_string();
    assert!(
        err_text.contains("raw-full-response") || err_text.contains("through-event"),
        "error should explain source/slicing conflict: {err_text}"
    );
}

#[test]
fn run_replay_inspect_command_parses_run_workspace_and_limit() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "run",
        "replay",
        "inspect",
        "--run-dir",
        "/tmp/run",
        "--workspace",
        "/tmp/workspace",
        "--limit",
        "12",
        "--tool-events-only",
        "--format",
        "json",
    ])
    .expect("run replay inspect should parse");

    match parsed.command {
        Command::Run(RunCommand {
            command:
                RunSubcommand::Replay(RunReplayCommand {
                    command: RunReplaySubcommand::Inspect(cmd),
                }),
        }) => {
            assert_eq!(cmd.run_dir, PathBuf::from("/tmp/run"));
            assert_eq!(cmd.workspace, Some(PathBuf::from("/tmp/workspace")));
            assert_eq!(cmd.limit, Some(12));
            assert!(cmd.tool_events_only);
            assert_eq!(cmd.format, InspectOutputFormat::Json);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn inspect_turn_accepts_loop_show_option() {
    let parsed = Cli::try_parse_from(["ploke-eval", "inspect", "turn", "1", "--show", "loop"])
        .expect("turn should accept the loop show option");

    match parsed.command {
        Command::Inspect(InspectCommand {
            command: InspectSubcommand::Turn(cmd),
        }) => assert_eq!(cmd.show, TurnShowOption::Loop),
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn inspect_turn_accepts_responses_show_option() {
    let parsed = Cli::try_parse_from(["ploke-eval", "inspect", "turn", "1", "--show", "responses"])
        .expect("turn should accept the responses show option");

    match parsed.command {
        Command::Inspect(InspectCommand {
            command: InspectSubcommand::Turn(cmd),
        }) => assert_eq!(cmd.show, TurnShowOption::Responses),
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn inspect_turn_still_accepts_long_turn_flag() {
    let parsed = Cli::try_parse_from(["ploke-eval", "inspect", "turn", "--turn", "1"])
        .expect("turn should still accept the hidden --turn flag");

    match parsed.command {
        Command::Inspect(InspectCommand {
            command: InspectSubcommand::Turn(cmd),
        }) => {
            assert_eq!(cmd.turn, None);
            assert_eq!(cmd.turn_flag, Some(1));
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn inspect_conversations_accepts_turns_alias() {
    let parsed = Cli::try_parse_from(["ploke-eval", "inspect", "turns"])
        .expect("conversations should accept the turns alias");

    match parsed.command {
        Command::Inspect(InspectCommand {
            command: InspectSubcommand::Conversations(_),
        }) => {}
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn inspect_turn_messages_accepts_role_filters() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "inspect",
        "turn",
        "1",
        "--show",
        "messages",
        "--exclude-roles",
        "system,user",
    ])
    .expect("turn messages should accept role filters");

    match parsed.command {
        Command::Inspect(InspectCommand {
            command: InspectSubcommand::Turn(cmd),
        }) => {
            assert_eq!(cmd.turn, Some(1));
            assert_eq!(
                cmd.exclude_roles,
                vec![InspectMessageRole::System, InspectMessageRole::User]
            );
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

fn sample_closure_state_for_submission_export(
    instances_root: PathBuf,
) -> crate::closure::ClosureState {
    crate::closure::ClosureState {
        schema_version: crate::closure::CLOSURE_STATE_SCHEMA_VERSION.to_string(),
        campaign_id: CampaignId::from("campaign-1"),
        updated_at: "2026-04-17T00:00:00Z".to_string(),
        config: crate::closure::ClosureConfig {
            benchmark_family: BenchmarkFamily::MultiSweBenchRust,
            model_id: Some("x-ai/grok-4-fast".to_string()),
            route_source: None,
            provider_slug: Some("xai".to_string()),
            registry_path: None,
            dataset_sources: Vec::new(),
            required_procedures: Vec::new(),
            instances_root,
            batches_root: PathBuf::from("/tmp/batches"),
            framework: crate::spec::FrameworkConfig::default(),
        },
        registry: crate::closure::RegistryClosureSummary {
            expected_total: 3,
            mapped_total: 3,
            missing_total: 0,
            ambiguous_total: 0,
            status: ClosureClass::Complete,
        },
        eval: crate::closure::EvalClosureSummary {
            expected_total: 3,
            complete_total: 2,
            failed_total: 1,
            missing_total: 0,
            partial_total: 0,
            in_progress_total: 0,
            status: ClosureClass::Partial,
            last_transition_at: None,
        },
        protocol: crate::closure::ProtocolClosureSummary {
            expected_total: 0,
            full_total: 0,
            partial_total: 0,
            failed_total: 0,
            missing_total: 0,
            incompatible_total: 0,
            ineligible_total: 0,
            in_progress_total: 0,
            status: ClosureClass::Complete,
            required_procedures: Vec::new(),
            status_by_procedure: BTreeMap::new(),
            last_transition_at: None,
        },
        instances: vec![
            crate::closure::ClosureInstanceRow {
                instance_id: "org__repo-1".to_string(),
                dataset_label: "org__repo".to_string(),
                repo_family: "org__repo".to_string(),
                registry_status: crate::closure::RegistryInstanceStatus::Mapped,
                eval_status: ClosureClass::Complete,
                protocol_status: ClosureClass::Complete,
                eval_failure: None,
                protocol_failure: None,
                artifacts: crate::closure::ClosureArtifactRefs::default(),
                protocol_procedures: BTreeMap::new(),
                protocol_counts: None,
                last_event_at: None,
            },
            crate::closure::ClosureInstanceRow {
                instance_id: "org__repo-2".to_string(),
                dataset_label: "org__repo".to_string(),
                repo_family: "org__repo".to_string(),
                registry_status: crate::closure::RegistryInstanceStatus::Mapped,
                eval_status: ClosureClass::Complete,
                protocol_status: ClosureClass::Complete,
                eval_failure: None,
                protocol_failure: None,
                artifacts: crate::closure::ClosureArtifactRefs::default(),
                protocol_procedures: BTreeMap::new(),
                protocol_counts: None,
                last_event_at: None,
            },
            crate::closure::ClosureInstanceRow {
                instance_id: "org__repo-3".to_string(),
                dataset_label: "org__repo".to_string(),
                repo_family: "org__repo".to_string(),
                registry_status: crate::closure::RegistryInstanceStatus::Mapped,
                eval_status: ClosureClass::Failed,
                protocol_status: ClosureClass::Ineligible,
                eval_failure: Some("failed".to_string()),
                protocol_failure: None,
                artifacts: crate::closure::ClosureArtifactRefs::default(),
                protocol_procedures: BTreeMap::new(),
                protocol_counts: None,
                last_event_at: None,
            },
        ],
    }
}

#[test]
fn default_campaign_submission_export_path_uses_campaign_directory() {
    let path = default_campaign_submission_export_path(&CampaignId::from("campaign-1"), false)
        .expect("default path");
    assert!(path.ends_with("campaign-1/multi-swe-bench-submission.jsonl"));

    let nonempty_path =
        default_campaign_submission_export_path(&CampaignId::from("campaign-1"), true)
            .expect("nonempty path");
    assert!(nonempty_path.ends_with("campaign-1/multi-swe-bench-submission.nonempty.jsonl"));
}

#[test]
fn campaign_export_submissions_parses_nonempty_flag() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "campaign",
        "export-submissions",
        "--campaign",
        "campaign-1",
        "--nonempty-only",
    ])
    .expect("campaign export-submissions should parse");

    match parsed.command {
        Command::Campaign(CampaignCommand {
            command: CampaignSubcommand::ExportSubmissions(cmd),
        }) => {
            assert_eq!(cmd.campaign, CampaignId::from("campaign-1"));
            assert!(cmd.nonempty_only);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn mbe_write_config_parses_explicit_artifacts() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "mbe",
        "write-config",
        "--run",
        "/tmp/run.json",
        "--submission",
        "/tmp/multi-swe-bench-submission.jsonl",
        "--output-dir",
        "/tmp/mbe",
        "--repo-dir",
        "/tmp/repos",
        "--workers",
        "2",
    ])
    .expect("mbe write-config should parse");

    match parsed.command {
        Command::Mbe(MbeCommand {
            command: MbeSubcommand::WriteConfig(cmd),
        }) => {
            assert_eq!(cmd.request.run, Some(PathBuf::from("/tmp/run.json")));
            assert_eq!(
                cmd.request.submission,
                Some(PathBuf::from("/tmp/multi-swe-bench-submission.jsonl"))
            );
            assert_eq!(cmd.request.instance, None);
            assert_eq!(cmd.request.attempt, None);
            assert_eq!(cmd.request.output_dir, Some(PathBuf::from("/tmp/mbe")));
            assert_eq!(cmd.request.repo_dir, Some(PathBuf::from("/tmp/repos")));
            assert_eq!(cmd.request.workers, 2);
            assert_eq!(cmd.python, "python");
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn mbe_run_parses_explicit_artifacts_and_python() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "mbe",
        "run",
        "--run",
        "/tmp/run.json",
        "--submission",
        "/tmp/multi-swe-bench-submission.jsonl",
        "--output-dir",
        "/tmp/mbe",
        "--repo-dir",
        "/tmp/repos",
        "--workers",
        "2",
        "--python",
        "python3",
    ])
    .expect("mbe run should parse");

    match parsed.command {
        Command::Mbe(MbeCommand {
            command: MbeSubcommand::Run(cmd),
        }) => {
            assert_eq!(cmd.request.run, Some(PathBuf::from("/tmp/run.json")));
            assert_eq!(
                cmd.request.submission,
                Some(PathBuf::from("/tmp/multi-swe-bench-submission.jsonl"))
            );
            assert_eq!(cmd.request.instance, None);
            assert_eq!(cmd.request.attempt, None);
            assert_eq!(cmd.request.output_dir, Some(PathBuf::from("/tmp/mbe")));
            assert_eq!(cmd.request.repo_dir, Some(PathBuf::from("/tmp/repos")));
            assert_eq!(cmd.request.workers, 2);
            assert_eq!(cmd.python, "python3");
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn mbe_run_parses_run_only_default_artifacts() {
    let parsed = Cli::try_parse_from(["ploke-eval", "mbe", "run", "--run", "/tmp/run.json"])
        .expect("mbe run should parse with default artifact paths");

    match parsed.command {
        Command::Mbe(MbeCommand {
            command: MbeSubcommand::Run(cmd),
        }) => {
            assert_eq!(cmd.request.run, Some(PathBuf::from("/tmp/run.json")));
            assert_eq!(cmd.request.instance, None);
            assert_eq!(cmd.request.attempt, None);
            assert_eq!(cmd.request.submission, None);
            assert_eq!(cmd.request.output_dir, None);
            assert_eq!(cmd.request.repo_dir, None);
            assert_eq!(cmd.request.workers, 1);
            assert_eq!(cmd.python, "python");
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn mbe_run_parses_instance_shortcut() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "mbe",
        "run",
        "--instance",
        "BurntSushi__ripgrep-2209",
        "--attempt",
        "2",
    ])
    .expect("mbe run should parse with instance shortcut");

    match parsed.command {
        Command::Mbe(MbeCommand {
            command: MbeSubcommand::Run(cmd),
        }) => {
            assert_eq!(cmd.request.run, None);
            assert_eq!(
                cmd.request.instance,
                Some("BurntSushi__ripgrep-2209".to_string())
            );
            assert_eq!(cmd.request.attempt, Some(2));
            assert_eq!(cmd.request.submission, None);
            assert_eq!(cmd.request.output_dir, None);
            assert_eq!(cmd.request.repo_dir, None);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn mbe_campaign_candidates_parses_campaign() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "mbe",
        "campaign-candidates",
        "--campaign",
        "campaign-1",
        "--nonempty-only",
    ])
    .expect("mbe campaign-candidates should parse");

    match parsed.command {
        Command::Mbe(MbeCommand {
            command: MbeSubcommand::CampaignCandidates(cmd),
        }) => {
            assert_eq!(cmd.campaign, CampaignId::from("campaign-1"));
            assert!(cmd.nonempty_only);
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn mbe_run_campaign_candidate_parses_node() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "mbe",
        "run-campaign-candidate",
        "--campaign",
        "campaign-1",
        "--node",
        "node-1",
        "--output-dir",
        "/tmp/mbe",
        "--repo-dir",
        "/tmp/repos",
        "--workers",
        "2",
        "--python",
        "python3",
    ])
    .expect("mbe run-campaign-candidate should parse");

    match parsed.command {
        Command::Mbe(MbeCommand {
            command: MbeSubcommand::RunCampaignCandidate(cmd),
        }) => {
            assert_eq!(cmd.campaign, CampaignId::from("campaign-1"));
            assert_eq!(cmd.node, "node-1");
            assert_eq!(cmd.output_dir, Some(PathBuf::from("/tmp/mbe")));
            assert_eq!(cmd.repo_dir, Some(PathBuf::from("/tmp/repos")));
            assert_eq!(cmd.workers, 2);
            assert_eq!(cmd.python, "python3");
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}

#[test]
fn mbe_runs_parses_instance() {
    let parsed = Cli::try_parse_from([
        "ploke-eval",
        "mbe",
        "runs",
        "--instance",
        "BurntSushi__ripgrep-2209",
    ])
    .expect("mbe runs should parse");

    match parsed.command {
        Command::Mbe(MbeCommand {
            command: MbeSubcommand::Runs(cmd),
        }) => {
            assert_eq!(cmd.instance, "BurntSushi__ripgrep-2209");
        }
        other => panic!("unexpected command shape: {:?}", other),
    }
}
