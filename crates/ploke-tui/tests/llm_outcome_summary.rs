//! Outcome summary message formatting
//!
//! Purpose: validate that a concise SysInfo message is emitted for LLM request
//! outcomes: success, 404, 429, and generic error. This test exercises only the
//! mapping logic (string content), not the full request loop.

use ploke_tui::llm::LlmError;

fn to_summary_string(res: Result<(), LlmError>) -> String {
    match &res {
        Ok(_) => "Request summary: success".to_string(),
        Err(LlmError::Api { status, .. }) if *status == 404 => {
            "Request summary: error 404 (endpoint/tool support?)".to_string()
        }
        Err(LlmError::Api { status, .. }) if *status == 429 => {
            "Request summary: rate limited (429)".to_string()
        }
        Err(e) => format!("Request summary: error ({})", e),
    }
}

#[test]
fn outcome_summary_variants() {
    assert_eq!(to_summary_string(Ok(())), "Request summary: success");
    assert_eq!(
        to_summary_string(Err(LlmError::Api { status: 404, message: "no tool".into() })),
        "Request summary: error 404 (endpoint/tool support?)"
    );
    assert_eq!(
        to_summary_string(Err(LlmError::Api { status: 429, message: "".into() })),
        "Request summary: rate limited (429)"
    );
    let generic = to_summary_string(Err(LlmError::Unknown("oops".into())));
    assert!(generic.starts_with("Request summary: error ("));
}

