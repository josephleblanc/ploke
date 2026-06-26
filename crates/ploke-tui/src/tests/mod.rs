// Testing in `ploke-tui`
// Use more crate-local tests instead of having a folder with tests outside of src, since it
// forces us to make our data structures public.
// Instead, we can run our integration tests here. Since we are a user-facing application, we are
// more concerned with running tests that ensure our application works and is correct than
// providing public-visibility functions to other applications.
// In short, we are a binary, not a lib.

// Note: UI tests that require full AppState are better as integration tests
// using the AppHarness. See tests/ui_approvals_integration.rs for the main
// deadlock fix validation tests.

pub mod ui_approvals_simple;
pub mod ui_approvals_truncation;
#[cfg(feature = "long_test")]
pub mod ui_performance_comprehensive;

#[cfg(feature = "call_graph")]
#[test]
fn build_rag_config_applies_call_context_user_config() {
    let rag = crate::user_config::RagUserConfig {
        call_context: crate::user_config::CallContextUserConfig {
            enabled: true,
            max_owner_hits: 64,
            max_sites_per_owner: 24,
            max_targets_per_site: 12,
            max_caller_hits: 2048,
            caller_factor: 0.75,
        },
        ..Default::default()
    };

    let cfg = super::build_rag_config(&rag);
    assert!(cfg.call_context.enabled);
    assert_eq!(cfg.call_context.max_owner_hits, 64);
    assert_eq!(cfg.call_context.max_sites_per_owner, 24);
    assert_eq!(cfg.call_context.max_targets_per_site, 12);
    assert_eq!(cfg.call_context.max_caller_hits, 2048);
    assert!((cfg.call_context.caller_factor - 0.75).abs() < f32::EPSILON);
}
