#![cfg(feature = "live_api_tests")]

use std::{collections::BTreeSet, env, fs, path::PathBuf};

use color_eyre::{Result, eyre::bail};
use ploke_core::tool_types::ToolName;
use ploke_llm::{
    ChatHttpConfig, ChatStepOutcome, LlmError, RequestMessage,
    request::endpoint::ToolChoice,
    router_only::{ChatCompRequest, google::Google},
};
use ploke_tui::tools::{Tool as _, ns_patch::NsPatch};
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;

const TEST_NAME: &str = "live_google_ns_patch_tool_call_canary";
const DEFAULT_MODEL: &str = "google/gemini-2.5-flash-lite";
const DEFAULT_TOKENS: u32 = 16_384;
const TARGET_FILE: &str = "docs/range-notes.txt";
const EXPECTED_DIFF: &str = concat!(
    "--- a/docs/range-notes.txt\n",
    "+++ b/docs/range-notes.txt\n",
    "@@ -1,8 +1,9 @@\n",
    " title: range notes\n",
    " status: draft\n",
    " \n",
    " Replacer checklist:\n",
    " - clear dst before replacement\n",
    "-- allow replacements outside the requested range\n",
    "+- store the requested range end before extending the search window\n",
    "+- skip matches that begin at or beyond the requested range end\n",
    " - preserve capture interpolation\n",
    " - report IO errors\n",
);

#[tokio::test]
#[ignore = "strict live direct-Google canary; requires GOOGLE_PROJECT_ID/GOOGLE_REGION, ADC, a tool-capable Gemini model, and quota"]
async fn live_google_ns_patch_tool_call_canary() -> Result<()> {
    require_google_route()?;

    let model = env::var("PLOKE_LIVE_GOOGLE_PATCH_MODEL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let tokens = env::var("PLOKE_LIVE_GOOGLE_PATCH_MAX_TOKENS")
        .ok()
        .map(|value| value.parse::<u32>())
        .transpose()?
        .unwrap_or(DEFAULT_TOKENS);

    eprintln!("{TEST_NAME}: model={model}, max_tokens={tokens}, tool_choice=auto");

    let request = ChatCompRequest::<Google>::default()
        .with_model_str(&model)?
        .with_message(RequestMessage::new_user(canary_prompt()))
        .with_max_tokens(tokens)
        .with_temperature(0.0)
        .with_tools(Some(vec![NsPatch::tool_def()]))
        .with_tool_choice(Some(ToolChoice::Auto));

    let step = match ploke_llm::chat_step(&Client::new(), &request, &ChatHttpConfig::default())
        .await
    {
        Ok(step) => step,
        Err(LlmError::FinishError {
            msg,
            full_response,
            finish_reason,
        }) => {
            write_artifact("finish_error_response.json", &full_response)?;
            bail!(
                "Google returned {finish_reason:?} instead of a parsed tool call; \
                 msg={:?}; artifact={}",
                snippet(&msg, 500),
                artifact_path("finish_error_response.json").display()
            );
        }
        Err(error) if is_google_quota_error(&error) => {
            bail!("Google quota/rate error; canary did not verify ns_patch tool calls: {error:?}");
        }
        Err(error) => return Err(error.into()),
    };

    write_artifact("response.json", &step.full_response)?;
    let finish = step
        .full_response
        .choices
        .first()
        .and_then(|choice| choice.finish_reason.as_ref());
    eprintln!(
        "{TEST_NAME}: finish_reason={finish:?}, usage={:?}, artifact={}",
        step.full_response.usage,
        artifact_path("response.json").display()
    );

    match step.outcome {
        ChatStepOutcome::ToolCalls { calls, .. } => {
            if calls.len() != 1 {
                bail!(
                    "expected exactly one non_semantic_patch tool call, got {}; calls={calls:#?}",
                    calls.len()
                );
            }
            let call = calls.first().expect("len checked");
            if call.function.name != ToolName::NsPatch {
                bail!(
                    "expected non_semantic_patch tool call, got {:?}",
                    call.function.name
                );
            }

            let args: Value = serde_json::from_str(&call.function.arguments)?;
            write_artifact("arguments.json", &args)?;
            assert_ns_patch_args(&call.function.arguments)?;
            eprintln!(
                "{TEST_NAME}: parsed {} args_len={} args_snippet={:?}",
                call.function.name.as_str(),
                call.function.arguments.len(),
                snippet(&call.function.arguments, 500)
            );
        }
        ChatStepOutcome::Content { content, reasoning } => {
            bail!(
                "expected non_semantic_patch tool call, got content; content={:?}; reasoning={:?}",
                content.as_ref().map(|value| snippet(value.as_ref(), 500)),
                reasoning.as_ref().map(|value| snippet(value.as_ref(), 500))
            );
        }
    }

    Ok(())
}

fn canary_prompt() -> String {
    format!(
        "\
This is a live ploke TUI tool-call canary. You already have the target file content.

Target file: `{TARGET_FILE}`

```text
title: range notes
status: draft

Replacer checklist:
- clear dst before replacement
- allow replacements outside the requested range
- preserve capture interpolation
- report IO errors
```

Call the `non_semantic_patch` tool exactly once. Do not answer in prose.
Use one `patches` entry with `file` exactly `{TARGET_FILE}` and `reasoning` as a short sentence.
The `diff` string must be this complete unified diff:

```diff
{EXPECTED_DIFF}```
"
    )
}

fn assert_ns_patch_args(args: &str) -> Result<()> {
    let value: Value = serde_json::from_str(args)?;
    assert_json_shape(&value)?;

    let params = NsPatch::deserialize_params(args).map_err(|err| {
        color_eyre::eyre::eyre!("ns_patch arguments failed TUI validation: {err:?}; args={args}")
    })?;
    if params.patches.len() != 1 {
        bail!("expected one patch entry, got {}", params.patches.len());
    }

    let patch = params.patches.first().expect("len checked");
    if patch.file.as_ref() != TARGET_FILE {
        bail!("expected file {TARGET_FILE:?}, got {:?}", patch.file);
    }
    if patch.reasoning.trim().is_empty() {
        bail!("reasoning must be non-empty");
    }

    let diff = patch.diff.as_ref();
    for needle in [
        "--- a/docs/range-notes.txt",
        "+++ b/docs/range-notes.txt",
        "@@ -1,8 +1,9 @@",
        "-- allow replacements outside the requested range",
        "+- skip matches that begin at or beyond the requested range end",
    ] {
        if !diff.contains(needle) {
            bail!("diff missing expected marker {needle:?}; diff={diff:?}");
        }
    }

    Ok(())
}

fn assert_json_shape(value: &Value) -> Result<()> {
    let obj = value
        .as_object()
        .ok_or_else(|| color_eyre::eyre::eyre!("tool arguments must be a JSON object"))?;
    let allowed = BTreeSet::from(["patches", "confidence"]);
    let keys = obj.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let extra = keys.difference(&allowed).copied().collect::<Vec<_>>();
    if !extra.is_empty() {
        bail!("unexpected top-level ns_patch argument keys: {extra:?}");
    }

    let patches = value
        .get("patches")
        .and_then(Value::as_array)
        .ok_or_else(|| color_eyre::eyre::eyre!("patches must be an array"))?;
    if patches.len() != 1 {
        bail!(
            "patches must contain exactly one entry, got {}",
            patches.len()
        );
    }
    let patch = patches[0]
        .as_object()
        .ok_or_else(|| color_eyre::eyre::eyre!("patches[0] must be an object"))?;
    let keys = patch.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let required = BTreeSet::from(["diff", "file", "reasoning"]);
    if keys != required {
        bail!("patches[0] keys must be exactly {required:?}, got {keys:?}");
    }

    Ok(())
}

fn require_google_route() -> Result<()> {
    let missing = ["GOOGLE_PROJECT_ID", "GOOGLE_REGION"]
        .into_iter()
        .filter(|name| {
            env::var(name)
                .ok()
                .is_none_or(|value| value.trim().is_empty())
        })
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        bail!(
            "strict live canary requires direct Google route env vars; missing/empty: {missing:?}"
        );
    }
    Ok(())
}

fn is_google_quota_error(error: &LlmError) -> bool {
    let text = format!("{error:?}");
    text.contains("RESOURCE_EXHAUSTED") || text.contains("429")
}

fn write_artifact(name: &str, value: &impl Serialize) -> Result<()> {
    let path = artifact_path(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

fn artifact_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate dir has workspace parent")
        .parent()
        .expect("workspace dir has parent")
        .join("target/test-output/ploke-tui")
        .join(format!("{TEST_NAME}.{name}"))
}

fn snippet(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}
