use std::io::prelude::*;
use std::{collections::HashSet, fs::File, time::Duration};

use ploke_test_utils::workspace_root;
use reqwest::Client;

use crate::llm::openrouter_catalog::ModelEntry;

use super::openrouter_catalog::ModelsResponse;

    pub const REL_MODEL_ID_DATA: &str = "crates/ploke-tui/data/models/ids.txt";
    pub const REL_MODEL_NAME_DATA: &str = "crates/ploke-tui/data/models/names.txt";
    pub const REL_MODEL_PROVIDER_DATA: &str = "crates/ploke-tui/data/models/providers.txt";
    pub const REL_MODEL_CAPABILITIES_DATA: &str = "crates/ploke-tui/data/models/capabilities.txt";
    pub const REL_MODEL_SUPPORTED_PARAMETERS_DATA: &str = "crates/ploke-tui/data/models/supported_parameters.txt";
    pub const REL_MODEL_SUPPORTS_TOOLS_DATA: &str = "crates/ploke-tui/data/models/supports_tools.json";
    pub const REL_MODEL_ALL_DATA: &str = "crates/ploke-tui/data/models/all.json";
    pub const REL_MODEL_ALL_DATA_RAW: &str = "crates/ploke-tui/data/models/all_raw.json";
    pub const REL_MODEL_ALL_DATA_RAW_EP: &str = "crates/ploke-tui/data/models/all_raw_ep.json";
    pub const REL_MODEL_ALL_DATA_STATS: &str = "crates/ploke-tui/data/models/all_raw_stats.txt";

#[cfg(feature = "live_api_tests")]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::io::{BufReader, BufWriter, prelude::*};
    use std::ops::Deref;
    use std::{collections::HashSet, fs::File, time::Duration};

    use itertools::Itertools;
    use lazy_static::lazy_static;
    use ploke_test_utils::workspace_root;
    use reqwest::{Client, ClientBuilder, RequestBuilder, Response};
    use serde_json::Value;

    use crate::llm::models::{
        REL_MODEL_ALL_DATA, REL_MODEL_ID_DATA, REL_MODEL_NAME_DATA,
        REL_MODEL_SUPPORTED_PARAMETERS_DATA, REL_MODEL_SUPPORTS_TOOLS_DATA,
    };
    use crate::llm::openrouter_catalog::ModelEntry;
    use crate::llm::provider_endpoints::{ModelEndpoint, ModelEndpointsData, SupportedParameters, SupportsTools};
use std::path::PathBuf;
    use crate::{
        llm::openrouter_catalog::ModelsResponse,
        test_harness::{default_headers, openrouter_env},
        user_config::openrouter_url,
    };
    use tokio::runtime::Runtime;

    use once_cell::sync::Lazy;

    use super::{REL_MODEL_ALL_DATA_RAW, REL_MODEL_ALL_DATA_RAW_EP, REL_MODEL_ALL_DATA_STATS};

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
    // Run the request only once per test run
    static MODELS_RESPONSE_EP: Lazy<ModelEndpointsData> = Lazy::new(|| {
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

            resp.json::<ModelEndpointsData>().await.expect("failed parse")
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
            let url = op.base_url.join("models").expect("Malformed Url");
            let api_key = op.key;

            client.get(url).bearer_auth(&api_key)
        };
    }

    lazy_static! {
        static ref OPENROUTER_MODELS_RESPONSE_EP: RequestBuilder = {
            let client = Client::builder()
                .timeout(Duration::from_secs(5))
                .default_headers(default_headers())
                .build()
                .expect("client");
            let op = openrouter_env().expect("No key");
            let url = op.base_url.join("models").expect("Malformed Url");
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

                let contents = match std::fs::read_to_string(&log_file) {
                    Ok(s) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        let alt: std::path::PathBuf = std::path::PathBuf::from(
                            log_file.to_string_lossy().replace("data/models/", "ai_temp_data/"),
                        );
                        match std::fs::read_to_string(&alt) {
                            Ok(s) => s,
                            Err(e2) if e2.kind() == std::io::ErrorKind::NotFound => {
                                eprintln!("Skipping: golden file not found at {:?} or {:?}", log_file, alt);
                                return Ok(());
                            }
                            Err(e2) => return Err(e2.into()),
                        }
                    }
                    Err(e) => return Err(e.into()),
                };
                let file_values: HashSet<_> = contents
                    .split_terminator('\n')
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
        |m: ModelEntry| {
            m.supported_parameters.map(|v| {
                v.iter()
                    .map(|sp| serde_json::to_string(sp).unwrap().trim_matches('"').to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            })
        },
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
            .split_terminator('\n')
            .map(|s| s.to_string())
            .collect();
        Ok(())
    }
    #[test]
    fn flakey_supports_tools() -> color_eyre::Result<()> {
        let models_resp = MODELS_RESPONSE.deref();
        let rel_path_const = REL_MODEL_SUPPORTS_TOOLS_DATA;

        let tools_filter = |m: &ModelEntry| {
            m.supported_parameters
                .as_ref()
                .is_some_and(|p| p.supports_tools())
                // .is_some_and(|p| p.contains(&SupportedParameters::Tools))
        };

        let all_values: Vec<ModelEntry> = models_resp
            .data
            .clone()
            .into_iter()
            .filter(tools_filter)
            .collect_vec();

        let mut log_file = workspace_root();
        log_file.push(rel_path_const);

        // WRITE_MODE = update files instead of comparing
        if std::env::var("WRITE_MODE").is_ok() {
            let f = File::create(&log_file)?;
            serde_json::to_writer_pretty(f, &all_values)?;
            eprintln!("Updated golden file at {:?}", log_file);
            return Ok(());
        }

        let file = match File::open(&log_file) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let alt: std::path::PathBuf = std::path::PathBuf::from(
                    log_file.to_string_lossy().replace("data/models/", "ai_temp_data/"),
                );
                match File::open(&alt) {
                    Ok(f) => f,
                    Err(e2) if e2.kind() == std::io::ErrorKind::NotFound => {
                        eprintln!("Skipping: golden file not found at {:?} or {:?}", log_file, alt);
                        return Ok(());
                    }
                    Err(e2) => return Err(e2.into()),
                }
            }
            Err(e) => return Err(e.into()),
        };
        let buf_reader = BufReader::new(file);
        let contents: Vec<ModelEntry> = serde_json::from_reader(buf_reader)?;
        let file_id_map = contents
            .into_iter()
            .map(|m| (m.id.clone(), m))
            .collect::<HashMap<String, ModelEntry>>();
        let file_id_set = file_id_map.keys().cloned().collect::<HashSet<String>>();
        let resp_id_map = all_values
            .into_iter()
            .map(|m| (m.id.clone(), m))
            .collect::<HashMap<String, ModelEntry>>();
        let resp_id_set = resp_id_map.keys().cloned().collect::<HashSet<String>>();

        let missing: Vec<_> = file_id_set.difference(&resp_id_set).collect();
        let extras: Vec<_> = resp_id_set.difference(&file_id_set).collect();

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

    /// Write the current tool-capable models to ai_temp_data for quick reference and reuse.
    #[test]
    fn write_tool_capable_models_to_ai_temp() -> color_eyre::Result<()> {
        use itertools::Itertools;
        use std::fs;
        let models_resp = MODELS_RESPONSE.deref();

        let tools_filter = |m: &ModelEntry| {
            m.supported_parameters
                .as_ref()
                .is_some_and(|p| p.supports_tools())
        };

        let selected: Vec<ModelEntry> = models_resp
            .data
            .clone()
            .into_iter()
            .filter(tools_filter)
            .collect_vec();

        // ai_temp_data directory under crate
        let mut ai_dir = workspace_root();
        ai_dir.push("crates/ploke-tui/ai_temp_data");
        fs::create_dir_all(&ai_dir).ok();

        // Write JSON with full entries
        let mut out_json = ai_dir.clone();
        out_json.push("tool_capable_models.json");
        let f = File::create(&out_json)?;
        serde_json::to_writer_pretty(f, &selected)?;

        // Also write a plain text list of model ids for quick scanning
        let mut out_txt = ai_dir.clone();
        out_txt.push("tool_capable_models.txt");
        let mut buf = String::new();
        for id in selected.iter().map(|m| m.id.as_str()) {
            buf.push_str(id);
            buf.push('\n');
        }
        fs::write(&out_txt, buf)?;

        eprintln!(
            "Wrote tool-capable models to:\n  {}\n  {}",
            out_json.display(),
            out_txt.display()
        );
        Ok(())
    }

    #[test]
    #[ignore]
    fn flakey_models_all_raw_ep() -> color_eyre::Result<()> {
        let models_resp = {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let req_builder = OPENROUTER_MODELS_RESPONSE_EP
                    .try_clone()
                    .expect("Error in response");

                let resp = req_builder
                    .send()
                    .await
                    .and_then(|r| r.error_for_status())
                    .expect("failed response");

                resp.json::<Vec<ModelEndpoint>>()
                    .await
                    .expect("failed parse")
            })
        };
        let rel_path_const = REL_MODEL_ALL_DATA_RAW_EP;

        let mut log_file = workspace_root();
        log_file.push(rel_path_const);

        // WRITE_MODE = update files instead of comparing
        if std::env::var("WRITE_MODE").is_ok() {
            let f = File::create(&log_file)?;
            serde_json::to_writer_pretty(f, &models_resp)?;
            eprintln!("Updated golden file at {:?}", log_file);
            return Ok(());
        }
        Ok(())
    }

    #[test]
    #[cfg(feature = "live_api_tests")]
    #[cfg(feature = "json_visitor")]
    fn flakey_models_all_raw() -> color_eyre::Result<()> {
        let models_resp = {
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

                resp.json::<serde_json::Value>()
                    .await
                    .expect("failed parse")
            })
        };
        let rel_path_const = REL_MODEL_ALL_DATA_RAW;

        let mut log_file = workspace_root();
        log_file.push(rel_path_const);

        // WRITE_MODE = update files instead of comparing
        if std::env::var("WRITE_MODE").is_ok() {
            let f = File::create(&log_file)?;
            serde_json::to_writer_pretty(f, &models_resp)?;
            eprintln!("Updated golden file at {:?}", log_file);
        }

        // Also write to ai_temp_data if requested for exploration
        if std::env::var("PLOKE_AI_TMP").is_ok() {
            let mut ai_dir = workspace_root();
            ai_dir.push("crates/ploke-tui/ai_temp_data");
            std::fs::create_dir_all(&ai_dir).ok();
            let mut out_path = ai_dir.clone();
            out_path.push("models_all_raw.json");
            let f = File::create(&out_path)?;
            serde_json::to_writer_pretty(f, &models_resp)?;
            eprintln!("Wrote models_all_raw.json to {}", out_path.display());
            // Run visitor and write stats into ai_temp_data
            crate::llm::openrouter::json_visitor::explore_file_visit_to_dir(&out_path, Some(&ai_dir));
            // Cardinality for id/canonical_slug and provider identities
            crate::llm::openrouter::json_visitor::analyze_cardinality_to_dir(
                &out_path,
                &ai_dir,
                &["id", "canonical_slug", "providers.id", "providers.provider", "providers.name", "providers.slug", "providers.provider_slug"],
            );
        }
        Ok(())
    }

    #[test]
    #[cfg(feature = "live_api_tests")]
    #[cfg(feature = "json_visitor")]
    fn ai_temp_endpoint_dump_and_stats() -> color_eyre::Result<()> {
        use ploke_core::ArcStr;

        use crate::llm::openrouter::provider_endpoints::ProvEnd;
        use std::time::{SystemTime, UNIX_EPOCH};
        if std::env::var("PLOKE_AI_TMP").is_err() {
            eprintln!("PLOKE_AI_TMP not set; skipping dump");
            return Ok(());
        }
        // Build a known endpoint URL and use ProvEnd to fetch raw
        use crate::llm::openrouter::providers::ProviderSlug;
        let rt = Runtime::new().unwrap();
        let v = rt.block_on(async move {
            let pe = ProvEnd { author: ArcStr::from("deepseek"), model: ArcStr::from("deepseek-chat-v3.1") };
            pe.call_endpoint_raw().await
        })?;

        let mut ai_dir = workspace_root();
        ai_dir.push("crates/ploke-tui/ai_temp_data");
        std::fs::create_dir_all(&ai_dir).ok();
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let mut out_path = ai_dir.clone();
        out_path.push(format!("endpoint_qwen3-30b-a3b-thinking-2507_{}.json", ts));
        let f = File::create(&out_path)?;
        serde_json::to_writer_pretty(f, &v)?;
        eprintln!("Wrote endpoint raw JSON to {}", out_path.display());
        // Analyze endpoints array and write stats to ai_temp_data
        crate::llm::openrouter::json_visitor::explore_endpoints_file_visit_to_dir(&out_path, &ai_dir);
        crate::llm::openrouter::json_visitor::analyze_endpoints_cardinality_to_dir(
            &out_path,
            &ai_dir,
            &["id", "name", "provider", "slug", "provider_slug"],
        );
        Ok(())
    }

    #[test]
    fn flakey_models_all_raw_stats() -> color_eyre::Result<()> {
        let models_resp = {
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

                resp.json::<serde_json::Value>()
                    .await
                    .expect("failed parse")
            })
        };
        let rel_path_const = REL_MODEL_ALL_DATA_STATS;

        let mut log_file = workspace_root();
        log_file.push(rel_path_const);

        // WRITE_MODE = update files instead of comparing
        if std::env::var("WRITE_MODE").is_ok() {
            let f = File::create(&log_file)?;
            serde_json::to_writer_pretty(f, &models_resp)?;
            eprintln!("Updated golden file at {:?}", log_file);
            return Ok(());
        }
        Ok(())
    }

    #[test]
    fn flakey_models_all() -> color_eyre::Result<()> {
        let models_resp = MODELS_RESPONSE.deref();
        let rel_path_const = REL_MODEL_ALL_DATA;

        let all_values: Vec<ModelEntry> = models_resp.data.clone().into_iter().collect_vec();

        let mut log_file = workspace_root();
        log_file.push(rel_path_const);

        // WRITE_MODE = update files instead of comparing
        if std::env::var("WRITE_MODE").is_ok() {
            let f = File::create(&log_file)?;
            serde_json::to_writer_pretty(f, &all_values)?;
            eprintln!("Updated golden file at {:?}", log_file);
            return Ok(());
        }

        let file = match File::open(&log_file) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let alt: std::path::PathBuf = std::path::PathBuf::from(
                    log_file.to_string_lossy().replace("data/models/", "ai_temp_data/"),
                );
                match File::open(&alt) {
                    Ok(f) => f,
                    Err(e2) if e2.kind() == std::io::ErrorKind::NotFound => {
                        eprintln!("Skipping: golden file not found at {:?} or {:?}", log_file, alt);
                        return Ok(());
                    }
                    Err(e2) => return Err(e2.into()),
                }
            }
            Err(e) => return Err(e.into()),
        };
        let buf_reader = BufReader::new(file);
        let contents: Vec<ModelEntry> = serde_json::from_reader(buf_reader)?;
        let file_id_map = contents
            .into_iter()
            .map(|m| (m.id.clone(), m))
            .collect::<HashMap<String, ModelEntry>>();
        let file_id_set = file_id_map.keys().cloned().collect::<HashSet<String>>();
        let resp_id_map = all_values
            .into_iter()
            .map(|m| (m.id.clone(), m))
            .collect::<HashMap<String, ModelEntry>>();
        let resp_id_set = resp_id_map.keys().cloned().collect::<HashSet<String>>();

        let missing: Vec<_> = file_id_set.difference(&resp_id_set).collect();
        let extras: Vec<_> = resp_id_set.difference(&file_id_set).collect();

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
}
