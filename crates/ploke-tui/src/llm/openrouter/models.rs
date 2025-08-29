use std::io::prelude::*;
use std::{collections::HashSet, fs::File, time::Duration};

use ploke_test_utils::workspace_root;
use reqwest::Client;

use crate::llm::openrouter_catalog::ModelEntry;

use super::openrouter_catalog::ModelsResponse;

const REL_MODEL_ID_DATA: &str = "crates/ploke-tui/data/models/ids.txt";
const REL_MODEL_NAME_DATA: &str = "crates/ploke-tui/data/models/names.txt";
const REL_MODEL_PROVIDER_DATA: &str = "crates/ploke-tui/data/models/providers.txt";
const REL_MODEL_CAPABILITIES_DATA: &str = "crates/ploke-tui/data/models/capabilities.txt";
const REL_MODEL_SUPPORTED_PARAMETERS_DATA: &str =
    "crates/ploke-tui/data/models/supported_parameters.txt";

mod tests {
    use std::io::prelude::*;
    use std::ops::Deref;
    use std::{collections::HashSet, fs::File, time::Duration};

    use lazy_static::lazy_static;
    use ploke_test_utils::workspace_root;
    use reqwest::{Client, ClientBuilder, RequestBuilder, Response};

    use crate::llm::models::{
        REL_MODEL_ID_DATA, REL_MODEL_NAME_DATA, REL_MODEL_SUPPORTED_PARAMETERS_DATA,
    };
    use crate::llm::openrouter_catalog::ModelEntry;
    use crate::{
        llm::openrouter_catalog::ModelsResponse,
        test_harness::{default_headers, openrouter_env},
        user_config::openrouter_url,
    };
    use tokio::runtime::Runtime;

    use once_cell::sync::Lazy;

    // Run the request only once per test run
    static MODELS_RESPONSE: Lazy<ModelsResponse> = Lazy::new(|| {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let req_builder = OPENROUTER_MODELS_RESPONSE
                .try_clone()
                .expect("Error in response");

            let resp = req_builder
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .expect("failed response");

            resp.json::<ModelsResponse>().await.expect("failed parse")
        })
    });

    lazy_static! {
        static ref OPENROUTER_MODELS_RESPONSE: RequestBuilder = {
            let client = Client::builder()
                .timeout(Duration::from_secs(5))
                .default_headers(default_headers())
                .build()
                .expect("client");
            let op = openrouter_env().expect("No key");
            let url = op.url.join("models").expect("Malformed Url");
            let api_key = op.key;

            client.get(url).bearer_auth(&api_key)
        };
    }
    macro_rules! generate_model_field_test {
        ($test_name:ident, $field_accessor:expr, $rel_path_const:ident) => {
            #[test]
            #[cfg(feature = "live_api_tests")]
            fn $test_name() -> color_eyre::Result<()> {
                use std::collections::HashSet;

                let models_resp = MODELS_RESPONSE.deref();

                let mut all_values = HashSet::new();
                for val in models_resp
                    .data
                    .clone()
                    .into_iter()
                    .filter_map($field_accessor)
                {
                    all_values.insert(val);
                }

                let mut log_file = workspace_root();
                log_file.push($rel_path_const);

                // WRITE_MODE = update files instead of comparing
                if std::env::var("WRITE_MODE").is_ok() {
                    let mut buf = String::new();
                    for v in &all_values {
                        buf.push_str(v);
                        buf.push('\n');
                    }
                    std::fs::write(&log_file, buf)?;
                    eprintln!("Updated golden file at {:?}", log_file);
                    return Ok(());
                }

                let contents = std::fs::read_to_string(&log_file)?;
                let file_values: HashSet<_> = contents
                    .split('\n')
                    .map(|s| s.to_string())
                    .collect();

                let missing: Vec<_> = file_values.difference(&all_values).collect();
                let extras: Vec<_> = all_values.difference(&file_values).collect();

                if !missing.is_empty() {
                    eprintln!("Missing values (in file, not in API):");
                    for v in &missing {
                        eprintln!("\t{}", v);
                    }
                }
                if !extras.is_empty() {
                    eprintln!("Extra values (in API, not in file):");
                    for v in &extras {
                        eprintln!("\t{}", v);
                    }
                }

                assert!(
                    missing.is_empty() && extras.is_empty(),
                    "Differences found between API response and {:?}",
                    log_file
                );

                Ok(())
            }
        };
    }

    generate_model_field_test!(
        flakey_openrouter_model_ids,
        |m: ModelEntry| Some(m.id),
        REL_MODEL_ID_DATA
    );
    generate_model_field_test!(
        flakey_openrouter_model_names,
        |m: ModelEntry| m.name,
        REL_MODEL_NAME_DATA
    );
    generate_model_field_test!(
        flakey_supported_parameters,
        |m: ModelEntry| { m.supported_parameters.map(move |v| v.join(",")) },
        REL_MODEL_SUPPORTED_PARAMETERS_DATA
    );

    #[test]
    #[ignore = "kept as backup for macro"]
    fn flakey_openrouter_models() -> color_eyre::Result<()> {
        let models_resp = MODELS_RESPONSE.deref();
        let mut all_names = HashSet::new();
        for value in models_resp.data.clone().into_iter().filter_map(|m| m.name) {
            all_names.insert(value);
        }
        eprintln!("all_names:");
        for name in all_names {
            eprintln!("\t{}", name);
        }
        let log_file = REL_MODEL_NAME_DATA;

        let contents = std::fs::read_to_string(log_file)?;
        let file_values: HashSet<_> = contents
            .split('\n')
            .map(|s| s.to_string())
            .collect();
        Ok(())
    }
}
