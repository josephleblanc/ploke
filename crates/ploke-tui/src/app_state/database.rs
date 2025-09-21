use std::{
    collections::{HashMap, HashSet},
    ops::ControlFlow,
};

use itertools::Itertools;
use ploke_core::{EmbeddingData, NodeId};
use ploke_db::{EmbedDataVerbose, NodeType, SimilarArgs, search_similar_args};
use ploke_transform::transform::transform_parsed_graph;
use serde::{Deserialize, Serialize};
use syn_parser::{
    ModuleTree, TestIds,
    parser::nodes::{AnyNodeId, AsAnyNodeId as _, ModuleNodeId, PrimaryNodeId},
    resolve::RelationIndexer,
};
use tokio::sync::oneshot;

use crate::{
    app_state::helpers::{print_module_set, printable_nodes},
    parser::{ParserOutput, run_parse_no_transform},
    utils::helper::find_file_by_prefix,
};

use super::*;

// NOTE: Consider refactoring to avoid using explicit control flow and use error handling to
// achieve the same results more clearly
pub(super) async fn save_db(state: &Arc<AppState>, event_bus: &Arc<EventBus>) -> ControlFlow<()> {
    let default_dir = if let Ok(dir) = dirs::config_local_dir().ok_or_else(|| {
        ploke_error::Error::Fatal(ploke_error::FatalError::DefaultConfigDir {
            msg: "Could not locate default config directory on system",
        })
        .emit_warning()
    }) {
        dir.join("ploke").join("data")
    } else {
        return ControlFlow::Break(());
    };
    if let Err(e) = tokio::fs::create_dir_all(&default_dir).await {
        let msg = format!(
            "Error:\nCould not create directory at default location: {}\nEncountered error while finding or creating directory: {}",
            default_dir.display(),
            e
        );
        tracing::error!(msg);
        event_bus.send(AppEvent::System(SystemEvent::BackupDb {
            file_dir: format!("{}", default_dir.display()),
            is_success: false,
            error: Some(msg),
        }));
    }
    let system_guard = state.system.read().await;
    // TODO: This error handling feels really cumbersome, should rework.

    // make sure directory exists, otherwise report error

    // Using crate focus here, which we set when we perform indexing.
    // TODO: Revisit this design. Consider how to best allow for potential switches in
    // focus of the user's target crate within the same session.
    // - Explicit command?
    // - Model-allowed tool calling?
    if let Some(crate_focus) = system_guard
        .crate_focus
        .clone()
        .iter()
        .filter_map(|cr| cr.file_name())
        .filter_map(|cr| cr.to_str())
        .next()
    {
        // let crate_focus_str = crate_focus.to_string_lossy();
        let crate_name_version = if let Ok(db_result) = state
            .db
            .get_crate_name_id(crate_focus)
            .map_err(ploke_error::Error::from)
            .inspect_err(|e| {
                e.emit_warning();
            }) {
            db_result
        } else {
            return ControlFlow::Break(());
        };

        let file_dir = default_dir.join(crate_name_version);
        tracing::info!("Checking for previous database file {}", file_dir.display());
        if let Ok(mut read_dir) = std::fs::read_dir(&default_dir) {
            tracing::info!("reading dir result\n{:?}", read_dir);
            while let Some(Ok(file)) = read_dir.next() {
                if file.path() == file_dir {
                    let _ = std::fs::remove_file(&file_dir).inspect_err(|e| {
                        tracing::error!(
                            "Error removing previous database file {}",
                            file_dir.display()
                        );
                    });
                }
            }
        }
        // TODO: Clones are bad. This is bad code. Fix it.
        // - Wish I could blame the AI but its all me :( in a rush
        match state.db.backup_db(file_dir.clone()) {
            Ok(()) => {
                event_bus.send(AppEvent::System(SystemEvent::BackupDb {
                    file_dir: format!("{}", file_dir.display()),
                    is_success: true,
                    error: None,
                }));
            }
            Err(e) => {
                event_bus.send(AppEvent::System(SystemEvent::BackupDb {
                    file_dir: format!("{}", file_dir.display()),
                    is_success: false,
                    error: Some(e.to_string()),
                }));
            }
        };
    }
    ControlFlow::Continue(())
}

/// Loads a previously saved database backup into the application.
///
/// This function searches the default configuration directory for a database backup file
/// created by the `SaveDb` command. The backup file follows a naming convention where it
/// begins with the human-readable crate name, followed by an underscore and a v5 UUID hash
/// obtained from `state.db.get_crate_name_id`.
///
/// # Process
/// 1. Locates the backup file in the default configuration directory
/// 2. Imports the backup into the current database using CozoDB's restore functionality
/// 3. Validates the restored database has content
/// 4. Updates application state to reflect the loaded crate
/// 5. Emits appropriate success/failure events
///
/// # Arguments
/// * `state` - Reference to the application state containing the database
/// * `event_bus` - Event bus for sending status updates
/// * `crate_name` - Name of the crate to load from backup
///
/// # Returns
/// Returns `Ok(())` if the database was successfully loaded, or an appropriate error
/// if the backup file was not found or the restore operation failed.
///
/// # Notes
/// The CozoDB restore operation must be performed on an empty database. If the current
/// database contains data, it will be replaced by the backup. The function handles
/// the full lifecycle of locating, validating, and restoring the database state.
pub(super) async fn load_db(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    crate_name: String,
) -> Result<(), ploke_error::Error> {
    let mut default_dir = dirs::config_local_dir().ok_or_else(|| {
        let err_msg = "Could not locate default config directory on system";
        let e =
            ploke_error::Error::Fatal(ploke_error::FatalError::DefaultConfigDir { msg: err_msg });
        e.emit_warning();
        event_bus.send(AppEvent::System(SystemEvent::LoadDb {
            crate_name: crate_name.clone(),
            file_dir: None,
            is_success: false,
            error: Some(err_msg),
        }));
        e
    })?;
    default_dir.push("ploke/data");
    let valid_file = match find_file_by_prefix(default_dir.as_path(), &crate_name).await {
        Ok(Some(path_buf)) => Ok(path_buf),
        Ok(None) => {
            let err_msg = "No backup file detected at default configuration location";
            let error = ploke_error::WarningError::PlokeDb(err_msg.to_string());
            event_bus.send(AppEvent::System(SystemEvent::LoadDb {
                crate_name: crate_name.clone(),
                file_dir: Some(Arc::new(default_dir)),
                is_success: false,
                error: Some(err_msg),
            }));
            Err(error)
        }
        Err(e) => {
            // TODO: Improve this error message
            tracing::error!("Failed to load file: {}", e);
            let err_msg = "Could not find saved file, io error";
            event_bus.send(AppEvent::System(SystemEvent::LoadDb {
                crate_name: crate_name.clone(),
                file_dir: Some(Arc::new(default_dir)),
                is_success: false,
                error: Some(err_msg),
            }));
            Err(ploke_error::FatalError::DefaultConfigDir { msg: err_msg })?
        }
    }?;

    let prior_rels_vec = state.db.relations_vec()?;
    tracing::debug!("prior rels for import: {:#?}", prior_rels_vec);
    state
        .db
        .import_from_backup(&valid_file, &prior_rels_vec)
        .map_err(ploke_db::DbError::from)
        .map_err(ploke_error::Error::from)?;
    ploke_db::create_index_primary(&state.db)?;
    // .inspect_err(|e| e.emit_error())?;

    // get count for sanity and user feedback
    match state.db.count_relations().await {
        Ok(count) if count > 0 => {
            {
                let mut system_guard = state.system.write().await;
                let script = format!(
                    "?[root_path] := *crate_context {{name: crate_name, root_path @ 'NOW' }}, crate_name = \"{crate_name}\""
                );
                let db_res = state.db.raw_query(&script)?;
                let crate_root_path = db_res
                    .rows
                    .first()
                    .and_then(|c| c.first())
                    .ok_or_else(|| {
                        let msg = "Incorrect retrieval of crate context, no first row/column";
                        tracing::error!(msg);
                        ploke_error::Error::Warning(ploke_error::WarningError::PlokeDb(
                            msg.to_string(),
                        ))
                    })
                    .map(|v| v.get_str().expect("Crate must always be a string"))?;
                let target_dir = std::env::current_dir()
                    .inspect_err(|e| tracing::error!("Error finding current dir: {e}"))
                    .ok();

                system_guard.crate_focus = target_dir.map(|cd| cd.join(crate_root_path));
            }
            event_bus.send(AppEvent::System(SystemEvent::LoadDb {
                crate_name,
                file_dir: Some(Arc::new(valid_file)),
                is_success: true,
                error: None,
            }));
            Ok(())
        }
        Ok(_count) => {
            event_bus.send(AppEvent::System(SystemEvent::LoadDb {
                crate_name,
                file_dir: Some(Arc::new(valid_file)),
                is_success: false,
                error: Some("Database backed up from file, but 0 relations found."),
            }));
            Ok(())
        }
        Err(e) => Err(e),
    }
}

pub(super) async fn scan_for_change(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    scan_tx: oneshot::Sender<Option<Vec<std::path::PathBuf>>>,
) -> Result<(), ploke_error::Error> {
    use ploke_error::Error as PlokeError;
    let guard = state.system.read().await;
    // TODO: Make a wrapper type for this and make it a method to get just the crate
    // name.
    // 1. Get the currently focused crate name, checking for errors.
    let crate_path = guard.crate_focus.as_ref().ok_or_else(|| {
        tracing::error!("Missing crate focus, cannot scan unspecified target crate");
        let e = PlokeError::from(StateError::MissingCrateFocus {
            msg: "Missing crate focus is None, cannot scan unspecified target crate",
        });
        e.emit_warning();
        e
    })?;
    let crate_name = crate_path.file_name().and_then(|os_str| os_str.to_str()).ok_or_else(|| { 
        tracing::error!("Crate name is empty, cannot scan empty crate name");
        let e = PlokeError::from(StateError::MissingCrateFocus {msg: "Missing crate focus is empty or non-utf8 string, cannot scan unspecified target crate"});
        e.emit_warning();
        e
    })?;

    tracing::info!("scan_for_change in crate_name: {}", crate_name);
    // 2. get the files in the target project from the db, with hashes
    let file_data = state.db.get_crate_files(crate_name)?;
    tracing::trace!("file_data: {:#?}", file_data);

    // 3. scan the files, returning a Vec<Option<FileData>>, where None indicates the file has not
    //    changed.
    //  - Note that this does not do anything for those files which may have been added, which will
    //  be handled in parsing during the IndexFiles event process mentioned in step 5 below.
    let result = state.io_handle.scan_changes_batch(file_data).await?;
    let vec_ok = result?;

    if !vec_ok.iter().any(|f| f.is_some()) {
        // 4. if no changes, send complete in oneshot
        match scan_tx.send(None) {
            Ok(()) => {
                tracing::info!("No file changes detected");
            }
            Err(e) => {
                tracing::error!("Error sending parse oneshot from ScanForChange");
            }
        };
    } else {
        // 5. if changes, send IndexFiles event (not yet made) or handle here.
        //  Let's see how far we get handling it here first.
        //  - Since we are parsing the whole target in any case, we might as well do it
        //  concurrently. Test sequential appraoch first, then move to be parallel earlier.

        // TODO: Move this into `syn_parser` probably
        // WARN: Just going to use a quick and dirty approach for now to get proof of concept, then later
        // on I'll do something more efficient.
        let ParserOutput { mut merged, tree } =
            run_parse_no_transform(Arc::clone(&state.db), Some(crate_path.clone()))?;

        // get the filenames to send through the oneshot
        let changed_filenames = vec_ok
            .iter()
            .filter_map(|opt| opt.as_ref().map(|f| f.file_path.clone()))
            .collect_vec();
        for file in changed_filenames.iter() {
            let filename = format!("{}", file.display());
            tracing::info!(target:"file_hashes", "Checking for details on {}", filename);
            let query_res = state.db.get_path_info(&filename)?;
            tracing::info!(target:"file_hashes", "headers:\n{}", query_res.headers.iter().join(", ") );
            let rows = query_res
                .rows
                .iter()
                .map(|r| r.iter().join(", "))
                .join("\n");
            tracing::info!(target:"file_hashes", "rows:\n {}", rows);
        }
        // WARN: Half-assed implementation, this should be a recurisve function instead of simple
        // collection.
        //  - coercing into ModuleNodeId with the test method escape hatch, do properly
        let module_uuids = vec_ok.into_iter().filter_map(|f| f.map(|i| i.id));
        let module_ids = module_uuids
            .clone()
            .map(|uid| ModuleNodeId::new_test(NodeId::Synthetic(uid)));
        // let module_ids = vec_ok.into_iter().filter_map(|f| f.map(|id|
        //     ModuleNodeId::new_test(NodeId::Synthetic(id.id))));
        let module_set: HashSet<ModuleNodeId> = module_ids.collect();

        let any_node_mod_set: Vec<AnyNodeId> =
            module_set.iter().map(|m_id| m_id.as_any()).collect();
        let printable_union_items = printable_nodes(&merged, any_node_mod_set.iter());
        tracing::info!(
            "Nodes in file set has count: {}\nitems:\n{}",
            module_set.len(),
            printable_union_items
        );

        print_module_set(&merged, &tree, &module_set);

        // NOTE: Better implementation to get all nodes in the target files that is recursive
        let mut full_mod_set: HashSet<AnyNodeId> = HashSet::new();
        for mod_id in module_set.iter() {
            full_mod_set = mods_in_file(*mod_id, full_mod_set, &tree);
            // full_mod_set.insert(mod_id.as_any());
            let printable_nodes = printable_nodes(&merged, full_mod_set.iter());
            tracing::info!(
                "recursive printable nodes for module_id:\n{}\n{}",
                mod_id,
                printable_nodes
            );
        }
        fn mods_in_file(
            current: ModuleNodeId,
            mut mods: HashSet<AnyNodeId>,
            tree: &ModuleTree,
        ) -> HashSet<AnyNodeId> {
            let start_len = mods.len();
            if let Some(tree_rels) = tree
                .get_iter_relations_from(&current.as_any())
                .map(|it| it.filter(|r| r.rel().is_contains()))
            {
                for tree_rel in tree_rels {
                    let maybe_next = tree_rel.rel().target();
                    mods.insert(maybe_next);
                    if tree
                        .get_iter_relations_from(&maybe_next)
                        .is_some_and(|mut trels| trels.any(|tr| tr.rel().is_contains()))
                    {
                        let next_mod: ModuleNodeId = maybe_next.try_into()
                            .expect("Invariant Violated: Contains should only be from ModuleNode -> PrimaryNode, found other");
                        mods = mods_in_file(next_mod, mods, tree);
                    }
                }
            }
            mods
        }

        // Gets all items that are contained by the modules.
        //  - May be missing some of the secondary node types like params, etc
        let item_set: HashSet<AnyNodeId> = module_set
            .iter()
            .filter_map(|id| tree.modules().get(id))
            .filter_map(|m| m.items())
            .flat_map(|items| items.iter().copied().map(|id| id.as_any()))
            .collect();
        let union = full_mod_set
            .iter()
            .copied()
            .map(|m_id| m_id.as_any())
            .chain(module_set.iter().copied().map(|m_id| m_id.as_any()))
            .collect::<HashSet<AnyNodeId>>()
            // let union = module_set.iter().copied().map(|m_id| m_id.as_any()).collect::<HashSet<AnyNodeId>>()
            .union(&item_set)
            .copied()
            .collect::<HashSet<AnyNodeId>>();
        // for now filter out anything that isn't one of the PrimaryNode types
        let filtered_union = union
            .into_iter()
            .filter(|&id| PrimaryNodeId::try_from(id).is_ok())
            // .filter(|&id| !matches!(id, AnyNodeId::Import(_)) || !matches!(id, AnyNodeId::Impl(_)))
            .collect::<HashSet<AnyNodeId>>();

        tracing::trace!("Nodes in union set:");
        let printable_union_items = printable_nodes(&merged, filtered_union.iter());
        tracing::trace!("prinable_union_items:\n{}", printable_union_items);
        // filter relations
        merged.graph.relations.retain(|r| {
            filtered_union.contains(&r.source()) || filtered_union.contains(&r.target())
        });
        // filter nodes
        merged.retain_all(filtered_union);
        // merged.graph.modules.retain(|m| m.is_file_based() || m.is_inline());

        transform_parsed_graph(&state.db, merged, &tree).inspect_err(|e| {
            tracing::error!("Error transforming partial graph into database:\n{e}");
        })?;

        for file_id in module_uuids {
            for node_ty in NodeType::primary_nodes() {
                tracing::info!("Retracting type: {}", node_ty.relation_str());
                let query_res = state
                    .db
                    .retract_embedded_files(file_id, node_ty)
                    .inspect_err(|e| tracing::error!("Error in retract_embed_files: {e}"))?;
                tracing::info!("Raw return of retract_embedded_files:\n{:?}", query_res);
                let to_print = query_res
                    .rows
                    .iter()
                    .map(|r| r.iter().join(" | "))
                    .join("\n");
                tracing::info!("Return of retract_embedded_files:\n{}", to_print);
            }
        }

        tracing::trace!("Finishing scanning, sending message to reindex workspace");
        event_bus.send(AppEvent::System(SystemEvent::ReIndex {
            workspace: crate_name.to_string(),
        }));
        let _ = scan_tx.send(Some(changed_filenames));
        // TODO: Add validation step here.
    }
    //

    Ok(())
}

pub(super) async fn write_query(state: &Arc<AppState>, query_content: String) {
    let result = state
        .db
        .raw_query_mut(&query_content)
        .inspect_err(|e| tracing::error!("{e}"));
    tracing::info!(target: "write_query", "testing query result\n{:#?}", result);
    if let Ok(named_rows) = result {
        let mut output = String::new();
        let (header, rows) = (named_rows.headers, named_rows.rows);
        let cols_num = header.len();
        let display_header = header.into_iter().map(|h| format!("{}", h)).join("|");
        tracing::info!(target: "write_query", "\n{display_header}");
        output.push('|');
        output.push_str(&display_header);
        output.push('|');
        output.push('\n');
        let divider = format!(
            "|{}",
            "-".chars()
                .cycle()
                .take(5)
                .chain("|".chars())
                .join("")
                .repeat(cols_num)
        );
        output.push_str(&divider);
        output.push('\n');
        rows.into_iter()
            .map(|r| {
                r.into_iter()
                    .map(|c| format!("{}", c))
                    .map(|c| format!("{}", c))
                    .join("|")
            })
            .for_each(|r| {
                tracing::info!(target: "write_query", "\n{}", r);
                output.push('|');
                output.push_str(&r);
                output.push('|');
                output.push('\n');
            });
        let outfile_name = "output.md";
        let out_file = std::env::current_dir().map(|d| d.join("query").join(outfile_name));
        if let Ok(file) = out_file {
            // Writes to file within `if let`, only handling the error case if needed
            if let Err(e) = tokio::fs::write(file, output).await {
                tracing::error!(target: "write_query", "Error writing query output to file {e}")
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct PromptData {
    prompt: String,
    k: usize,
    ef: usize,
    max_hits: usize,
    radius: f64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct QueryParams {
    k: usize,
    ef: usize,
    max_hits: usize,
    radius: f64,
}

impl From<PromptData> for QueryParams {
    fn from(value: PromptData) -> Self {
        let PromptData {
            prompt,
            k,
            ef,
            max_hits,
            radius,
        } = value;
        QueryParams {
            k,
            ef,
            max_hits,
            radius,
        }
    }
}
impl From<&PromptData> for QueryParams {
    fn from(value: &PromptData) -> Self {
        let PromptData {
            prompt,
            k,
            ef,
            max_hits,
            radius,
        } = value;
        QueryParams {
            k: *k,
            ef: *ef,
            max_hits: *max_hits,
            radius: *radius,
        }
    }
}

/// Performs batch semantic search on prompts from a file and returns results
///
/// This function reads prompts from a file, generates embeddings for each prompt,
/// performs semantic search against the database, and returns the results in a
/// structured format suitable for serialization.
///
/// # Arguments
///
/// * `state` - Shared application state containing database and embedder
/// * `prompt_file` - Path to file containing prompts (separated by "---")
/// * `out_file` - Path to output file for results (JSON format)
/// * `max_hits` - Maximum number of similar snippets to return per prompt
/// * `threshold` - Optional similarity threshold for filtering results
///
/// # Returns
///
/// Returns a vector of batch results containing prompt indices, original prompts,
/// and their corresponding code snippets found through semantic search.
/// Results are automatically written to the specified output file as JSON.
pub(super) async fn batch_prompt_search(
    state: &Arc<AppState>,
    prompt_file: String,
    out_file: String,
    max_hits: Option<usize>,
    threshold: Option<f32>,
) -> color_eyre::Result<Vec<BatchResult>> {
    use ploke_embed::indexer::EmbeddingProcessor;
    use std::fs;

    let raw_prompts = fs::read_to_string(&prompt_file)?;
    let prompt_json = serde_json::from_str(&raw_prompts)?;
    let prompt_data: Vec<PromptData> = serde_json::from_value(prompt_json)?;

    if prompt_data.is_empty() {
        return Ok(Vec::new());
    }

    // let max_hits: usize = max_hits.unwrap_or(10);
    let _threshold = threshold.unwrap_or(0.0);

    let mut results = Vec::new();

    for (prompt_idx, prompt_item) in prompt_data.into_iter().enumerate() {
        let query_params: QueryParams = (&prompt_item).into();
        let PromptData {
            prompt,
            k,
            ef,
            max_hits,
            radius,
        } = prompt_item;
        tracing::info!("Processing prompt {}: {}", prompt_idx, prompt);

        let embeddings = state
            .embedder
            .generate_embeddings(vec![prompt.clone()])
            .await?;

        if let Some(embedding) = embeddings.into_iter().next() {
            for ty in NodeType::primary_nodes() {
                let ef_range = 1..=101;

                let args = SimilarArgs {
                    db: &state.db,
                    vector_query: &embedding,
                    k,
                    ef,
                    ty,
                    max_hits,
                    radius,
                };
                let EmbedDataVerbose { typed_data, dist } = search_similar_args(args)?;
                let snippets = typed_data.v.iter().map(|i| i.name.clone()).collect_vec();
                let file_paths = typed_data
                    .v
                    .iter()
                    .map(|f| f.file_path.clone())
                    .collect_vec();

                let code_snippets = state.io_handle.get_snippets_batch(typed_data.v).await?;

                let mut ok_snippets: Vec<SnippetInfo> = Vec::new();
                for (((snippet_result, name), dist), file_path) in code_snippets
                    .into_iter()
                    .zip(snippets)
                    .zip(dist)
                    .zip(file_paths)
                {
                    let unformatted = snippet_result?;
                    let snippet = unformatted.split("\\n").join("\n");
                    let snippet_info = SnippetInfo {
                        name,
                        dist,
                        file_path: format!("{}", file_path.display()),
                        snippet,
                    };
                    ok_snippets.push(snippet_info);
                }

                results.push(BatchResult {
                    prompt_idx,
                    node_type: ty.relation_str(),
                    prompt: prompt.clone(),
                    snippet_info: ok_snippets,
                    query_params,
                });
            }
        }
    }

    // Write results to file
    let json_content = serde_json::to_string_pretty(&results)?;

    fs::write(&out_file, json_content)?;

    Ok(results)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SnippetInfo {
    name: String,
    dist: f64,
    file_path: String,
    snippet: String,
}

/// Result structure for batch prompt search operations
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BatchResult {
    pub prompt_idx: usize,
    pub node_type: &'static str,
    pub prompt: String,
    pub snippet_info: Vec<SnippetInfo>,
    pub query_params: QueryParams,
}

#[cfg(test)]
mod test {

    use std::{ops::Index, path::PathBuf};

    use cozo::DataValue;
    use ploke_db::{Database, QueryResult};
    use ploke_embed::local::EmbeddingConfig;
    use ploke_rag::RagService;
    use syn_parser::parser::nodes::ToCozoUuid;

    use crate::{llm::manager::llm_manager, tracing_setup::init_tracing};

    use super::*;
    use ploke_embed::{
        indexer::{EmbeddingProcessor, EmbeddingSource},
        local::LocalEmbedder,
    };
    use rand::Rng;
    use tokio::time::{Duration, sleep};

    use super::error::{ErrorExt, ErrorSeverity, ResultExt};
    use color_eyre::Result;
    use futures::{FutureExt, StreamExt};
    use ploke_test_utils::{init_test_tracing, setup_db_full, setup_db_full_crate, workspace_root};
    use thiserror::Error;

    #[tokio::test]
    async fn test_update_embed() -> color_eyre::Result<()> {
        if std::env::var("PLOKE_RUN_UPDATE_EMBED").ok().as_deref() != Some("1") {
            eprintln!("Skipping: PLOKE_RUN_UPDATE_EMBED!=1");
            return Ok(());
        }
        // init_test_tracing(Level::DEBUG);
        let workspace_root = workspace_root();
        let target_crate = "fixture_update_embed";
        let workspace = "tests/fixture_crates/fixture_update_embed";

        // ensure file begins in same state by using backup
        let backup_file = PathBuf::from(format!(
            "{}/{}/src/backup_main.bak",
            workspace_root.display(),
            workspace
        ));
        tracing::trace!("reading from backup files: {}", backup_file.display());
        let backup_contents = std::fs::read(&backup_file)?;
        let target_main = backup_file.with_file_name("main.rs");
        std::fs::write(&target_main, backup_contents)?;

        let cozo_db = if target_crate.starts_with("fixture") {
            setup_db_full(target_crate)
        } else if target_crate.starts_with("crates") {
            let crate_name = target_crate.trim_start_matches("crates/");
            setup_db_full_crate(crate_name)
        } else {
            panic!("Incorrect usage of the test db setup");
        }?;

        dotenvy::dotenv().ok();

        let mut config = config::Config::builder()
            .add_source(
                config::File::with_name(
                    &dirs::config_dir()
                        .unwrap() // TODO: add error handling
                        .join("ploke/config.toml")
                        .to_string_lossy(),
                )
                .required(false),
            )
            .add_source(config::Environment::default().separator("_"))
            .build()?
            .try_deserialize::<crate::user_config::UserConfig>()
            .unwrap_or_else(|_| crate::user_config::UserConfig::default());

        tracing::debug!("Registry prefs loaded: {:#?}", config.registry);
        let new_db = ploke_db::Database::new(cozo_db);
        let db_handle = Arc::new(new_db);

        // Initial parse is now optional - user can run indexing on demand
        // run_parse(Arc::clone(&db_handle), Some(TARGET_DIR_FIXTURE.into()))?;

        // TODO: Change IoManagerHandle so it doesn't spawn its own thread, then use similar pattern to
        // spawning state meager below.
        let io_handle = ploke_io::IoManagerHandle::new();

        // TODO: These numbers should be tested for performance under different circumstances.
        let event_bus_caps = EventBusCaps::default();
        let event_bus = Arc::new(EventBus::new(event_bus_caps));

        let processor = config.load_embedding_processor()?;
        let proc_arc = Arc::new(processor);

        // TODO:
        // 1 Implement the cancellation token propagation in IndexerTask
        // 2 Add error handling for embedder initialization failures
        let indexer_task = IndexerTask::new(
            db_handle.clone(),
            io_handle.clone(),
            Arc::clone(&proc_arc), // Use configured processor
            CancellationToken::new().0,
            8,
        );

        let rag = RagService::new(Arc::clone(&db_handle), Arc::clone(&proc_arc))?;
        let state = Arc::new(AppState {
            chat: ChatState::new(ChatHistory::new()),
            config: ConfigState::default(),
            system: SystemState::default(),
            indexing_state: RwLock::new(None),
            indexer_task: Some(Arc::new(indexer_task)),
            indexing_control: Arc::new(Mutex::new(None)),
            db: db_handle.clone(),
            embedder: Arc::clone(&proc_arc),
            io_handle: io_handle.clone(),
            proposals: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            rag: Some(Arc::new(rag)),
            budget: TokenBudget::default(), // rag_tx: rag_event_tx.clone()
        });
        {
            let mut system_guard = state.system.write().await;
            let path = workspace_root.join(workspace);
            system_guard.crate_focus = Some(path);
            tracing::trace!("system_guard.crate_focus: {:?}", system_guard.crate_focus);
        }

        // Create command channel with backpressure
        let (cmd_tx, cmd_rx) = mpsc::channel::<StateCommand>(1024);

        let (rag_event_tx, rag_event_rx) = mpsc::channel(10);
        // let context_manager = ContextManager::new(rag_event_rx, Arc::clone(&event_bus));
        // tokio::spawn(context_manager.run());

        let (cancellation_token, cancel_handle) = CancellationToken::new();
        let (filemgr_tx, filemgr_rx) = mpsc::channel::<AppEvent>(256);
        let file_manager = FileManager::new(
            io_handle.clone(),
            event_bus.subscribe(EventPriority::Background),
            event_bus.background_tx.clone(),
            rag_event_tx.clone(),
            event_bus.realtime_tx.clone(),
        );

        tokio::spawn(file_manager.run());

        // Spawn state manager first
        tokio::spawn(state_manager(
            state.clone(),
            cmd_rx,
            event_bus.clone(),
            rag_event_tx,
        ));

        // Set global event bus for error handling
        set_global_event_bus(event_bus.clone()).await;

        // let script = r#"?[name, id, embedding] := *function{name, id, embedding @ 'NOW' }"#;
        let script = r#"?[name, time, is_assert, maybe_null, id] := *function{ id, at, name, embedding }
                                or *struct{ id, at, name, embedding } 
                                or *module{ id, at, name, embedding } 
                                or *static{ id, at, name, embedding } 
                                or *const{ id, at, name, embedding }, 
                                  time = format_timestamp(at),
                                  is_assert = to_bool(at),
                                  maybe_null = !is_null(embedding)
        "#;
        let query_result = db_handle.raw_query(script)?;
        let printable_rows = query_result
            .rows
            .iter()
            .map(|r| r.iter().join(", "))
            .join("\n");
        tracing::trace!("rows from db:\n{printable_rows}");

        // Spawn subsystems with backpressure-aware command sender
        let command_style = config.command_style;
        tokio::spawn(llm_manager(
            event_bus.subscribe(EventPriority::Background),
            state.clone(),
            cmd_tx.clone(), // Clone for each subsystem
            event_bus.clone(),
        ));
        tokio::spawn(run_event_bus(Arc::clone(&event_bus)));

        // setup target file:

        cmd_tx
            .send(StateCommand::IndexWorkspace {
                workspace: workspace.to_string(),
                needs_parse: false,
            })
            .await?;
        let mut app_rx = event_bus.index_subscriber();
        while let Ok(event) = app_rx.recv().await {
            match event {
                IndexingStatus {
                    status: IndexStatus::Running,
                    ..
                } => {
                    tracing::trace!("IndexStatus Running");
                }
                IndexingStatus {
                    status: IndexStatus::Completed,
                    ..
                } => {
                    tracing::trace!("IndexStatus Completed, breaking loop");
                    break;
                }
                _ => {}
            }
        }

        // print database output after indexing
        // or *struct{name, id, embedding & 'NOW'}
        let query_result = db_handle.raw_query(script)?;
        let printable_rows = query_result
            .rows
            .iter()
            .map(|r| r.iter().join(", "))
            .join("\n");
        tracing::trace!("rows from db:\n{printable_rows}");

        fn iter_col<'a>(
            query_result: &'a QueryResult,
            col_title: &str,
        ) -> Option<impl Iterator<Item = &'a DataValue>> {
            let col_idx = query_result
                .headers
                .iter()
                .enumerate()
                .find(|(idx, col)| col.as_str() == col_title)
                .map(|(idx, col)| idx)?;
            Some(query_result.rows.iter().map(move |r| r.index(col_idx)))
        }
        fn is_id_embed_null(db_handle: &Database, ty: NodeType, id: AnyNodeId) -> Result<bool> {
            let rel_name = ty.relation_str();
            let cozo_id = id.to_cozo_uuid();
            let one_script = format!(
                "?[name, item_id, is_embedding_null] := *{rel_name}{{ name, id: item_id, embedding @ 'NOW' }},
                    is_embedding_null = is_null(embedding),
                    item_id = {cozo_id}"
            );
            let query = db_handle.raw_query(&one_script)?;
            let is_embedding_null_now = iter_col(&query, "is_embedding_null")
                .expect("column not found")
                .next()
                .expect("row not found")
                .get_bool()
                .expect("cell not expected datatype (bool)");
            Ok(is_embedding_null_now)
        }
        fn is_name_embed_null(db_handle: &Database, ty: NodeType, name: &str) -> Result<bool> {
            let rel_name = ty.relation_str();
            let one_script = format!(
                "?[item_name, id, is_embedding_null] := *{rel_name}{{ name: item_name, id, embedding @ 'NOW' }},
                    is_embedding_null = is_null(embedding),
                    item_name = {name:?}"
            );
            let query = db_handle.raw_query(&one_script)?;
            let is_embedding_null_now = iter_col(&query, "is_embedding_null")
                .expect("column not found")
                .next()
                .expect("row not found")
                .get_bool()
                .expect("cell not expected datatype (bool)");
            Ok(is_embedding_null_now)
        }
        let one_script = r#"
            ?[name, id, is_embedding_null] := *const{ name, id, embedding @ 'NOW' },
                is_embedding_null = is_null(embedding)
        "#;
        let query_one = db_handle.raw_query(one_script)?;
        let is_const_embedding_null_now = iter_col(&query_one, "is_embedding_null")
            .expect("column not found")
            .next()
            .expect("row not found")
            .get_bool()
            .expect("cell not expected datatype (bool)");
        assert!(!is_const_embedding_null_now);

        // items in as-yet unchanged file, expect to be embedded initially (before scan sets them to null
        // again)
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Module,
            "double_inner_mod"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Const,
            "NUMBER_ONE"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Static,
            "STR_TWO"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Struct,
            "TestStruct"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Module,
            "inner_test_mod"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "func_with_params"
        )?);
        assert!(!is_name_embed_null(&db_handle, NodeType::Function, "main")?);
        // items not in changed file, expect to be remain embedded
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "simple_four"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Struct,
            "OtherStruct"
        )?);

        let mut target_file = {
            let mut system_guard = state.system.write().await;
            system_guard.crate_focus = Some(workspace_root.join(workspace));
            system_guard
                .crate_focus
                .clone()
                .expect("Crate focus not set")
        };
        tracing::trace!("target_file before pushes:\n{}", target_file.display());
        target_file.push("src");
        target_file.push("main.rs");
        tracing::trace!("target_file after pushes:\n{}", target_file.display());

        // ----- start test function ------
        let (scan_tx, scan_rx) = oneshot::channel();
        let result = scan_for_change(&state.clone(), &event_bus.clone(), scan_tx).await;
        tracing::trace!("result of scan_for_change: {:?}", result);
        // ----- end test start test ------

        tracing::trace!("waiting for scan_rx");

        // ----- await on end of test function `scan_for_change` -----
        match scan_rx.await {
            Ok(_) => tracing::trace!("scan_rx received for end of scan_for_change"),
            Err(_) => tracing::trace!("error in scan_rx awaiting on end of scan_for_change"),
        };

        // print database output after scan
        let query_result = db_handle.raw_query(script)?;
        let printable_rows = query_result
            .rows
            .iter()
            .map(|r| r.iter().join(", "))
            .join("\n");
        tracing::trace!("rows from db:\n{printable_rows}");

        // Nothing should have changed after running scan on the target when the target has not
        // changed.
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Const,
            "NUMBER_ONE"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Static,
            "STR_TWO"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Struct,
            "TestStruct"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Module,
            "double_inner_mod"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Module,
            "inner_test_mod"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "func_with_params"
        )?);
        assert!(!is_name_embed_null(&db_handle, NodeType::Function, "main")?);
        // Same here
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "simple_four"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Struct,
            "OtherStruct"
        )?);

        // ----- make change to target file -----
        let contents = std::fs::read_to_string(&target_file)?;
        tracing::trace!("reading file:\n{}", &contents);
        let changed = contents
            .lines()
            .map(|l| {
                if l.contains("pub struct TestStruct(pub i32)") {
                    "struct TestStruct(pub i32);"
                } else {
                    l
                }
            })
            .join("\n");
        tracing::trace!("writing changed file:\n{}", &changed);
        std::fs::write(&target_file, changed)?;

        // ----- start second scan -----
        let (scan_tx, scan_rx) = oneshot::channel();
        let result = scan_for_change(&state.clone(), &event_bus.clone(), scan_tx).await;
        tracing::trace!("result of after second scan_for_change: {:?}", result);
        // ----- end second scan -----

        // print database output after second scan
        let query_result = db_handle.raw_query(script)?;
        let printable_rows = query_result
            .rows
            .iter()
            .map(|r| r.iter().join(", "))
            .join("\n");
        tracing::trace!("rows from db:\n{printable_rows}");

        // items in changed file, expect to have null embeddings after scan
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Module,
            "double_inner_mod"
        )?);
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Const,
            "NUMBER_ONE"
        )?);
        assert!(is_name_embed_null(&db_handle, NodeType::Static, "STR_TWO")?);
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Struct,
            "TestStruct"
        )?);
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Module,
            "inner_test_mod"
        )?);
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "func_with_params"
        )?);
        assert!(is_name_embed_null(&db_handle, NodeType::Function, "main")?);
        // items not in changed file, expect to be remain embedded
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "simple_four"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Struct,
            "OtherStruct"
        )?);

        // -- simulating sending response from app back to index --
        // At the end of `scan_for_change`, an `AppEvent` is sent, which is processed inside the
        // app event loop (not running here), which should print a message and then send another
        // message to index the unembedded items in the database, which should currently only be
        // the items detected as having changed through `scan_for_change`.

        cmd_tx
            .send(StateCommand::IndexWorkspace {
                workspace: workspace.to_string(),
                needs_parse: false,
            })
            .await?;
        let mut app_rx = event_bus.index_subscriber();
        while let Ok(event) = app_rx.recv().await {
            match event {
                IndexingStatus {
                    status: IndexStatus::Running,
                    ..
                } => {
                    tracing::trace!("IndexStatus Running");
                }
                IndexingStatus {
                    status: IndexStatus::Completed,
                    ..
                } => {
                    tracing::trace!("IndexStatus Completed, breaking loop");
                    break;
                }
                _ => {}
            }
        }

        // print database output after reindex following the second scan
        let query_result = db_handle.raw_query(script)?;
        let printable_rows = query_result
            .rows
            .iter()
            .map(|r| r.iter().join(", "))
            .join("\n");
        tracing::debug!("rows from db:\n{printable_rows}");

        // items in changed file, expect to have embeddings again after scan
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Const,
            "NUMBER_ONE"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Static,
            "STR_TWO"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Struct,
            "TestStruct"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Module,
            "double_inner_mod"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Module,
            "inner_test_mod"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "func_with_params"
        )?);
        assert!(!is_name_embed_null(&db_handle, NodeType::Function, "main")?);
        // items not in changed file, expect to be remain embedded
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "simple_four"
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Struct,
            "OtherStruct"
        )?);

        tracing::trace!("changing back:\n{}", target_file.display());
        let contents = std::fs::read_to_string(&target_file)?;
        tracing::trace!("reading file:\n{}", &contents);
        let changed = contents
            .lines()
            .map(|l| {
                if l.contains("struct TestStruct(pub i32)") {
                    "pub struct TestStruct(pub i32);"
                } else {
                    l
                }
            })
            .join("\n");
        tracing::trace!("writing changed file:\n{}", &changed);
        std::fs::write(&target_file, changed)?;
        Ok(())
    }
}
