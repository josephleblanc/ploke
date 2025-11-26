use std::{
    collections::{HashMap, HashSet},
    ops::ControlFlow,
};

use itertools::Itertools;
use ploke_core::{EmbeddingData, FileData, NodeId};
use ploke_db::{multi_embedding::VECTOR_DIMENSION_SPECS, search_similar_args, EmbedDataVerbose, NodeType, SimilarArgs};
#[cfg(feature = "multi_embedding")]
use ploke_core::{EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSetId, EmbeddingShape};
use ploke_db::multi_embedding::schema::vector_dims::sample_vector_dimension_specs;
#[cfg(feature = "multi_embedding")]
use ploke_db::{SimilarArgsForSet, search_similar_args_for_set};
#[cfg(feature = "multi_embedding")]
use crate::user_config::EmbeddingConfig;
use ploke_transform::transform::transform_parsed_graph;
use serde::{Deserialize, Serialize};
use syn_parser::{
    ModuleTree, TestIds,
    parser::nodes::{AnyNodeId, AsAnyNodeId as _, ModuleNodeId, PrimaryNodeId},
    resolve::RelationIndexer,
};
use tokio::sync::oneshot;
use tracing::{ debug, error, info, trace };

use crate::{
    app_state::helpers::{print_module_set, printable_nodes}, parser::{run_parse_no_transform, ParserOutput}, tracing_setup::SCAN_CHANGE, utils::helper::find_file_by_prefix
};

use super::*;

pub const TUI_DB_TARGET: &str = "tracing_db_target";

// NOTE: Consider refactoring to avoid using explicit control flow and use error handling to
// achieve the same results more clearly
pub(super) async fn save_db(state: &Arc<AppState>, event_bus: &Arc<EventBus>) {
     let dir_res = dirs::config_local_dir().ok_or_else(|| {
         ploke_error::Error::Fatal(ploke_error::FatalError::DefaultConfigDir {
             msg: "Could not locate default config directory on system",
         })
     });

     let default_dir = match dir_res {
         Ok(dir) => dir.join("ploke").join("data"),
         Err(e) => {
             e.emit_warning();
             event_bus.send(AppEvent::System(SystemEvent::BackupDb {
                 file_dir: "<none>".into(),
                 is_success: false,
                 error: Some(e.to_string()),
             }));
             return;
         }
     };
    if let Err(e) = tokio::fs::create_dir_all(&default_dir).await {
        let msg = format!(
            "Error:\nCould not create directory at default location: {}\nEncountered error while finding or creating directory: {}",
            default_dir.display(),
            e
        );
        error!(msg);
        event_bus.send(AppEvent::System(SystemEvent::BackupDb {
            file_dir: format!("{}", default_dir.display()),
            is_success: false,
            error: Some(msg),
        }));
    }
    let system_guard = state.system.read().await;
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
        let crate_name_version = match state
            .db
            .get_crate_name_id(crate_focus)
            .map_err(ploke_error::Error::from)
        {
                Ok(db_result) => {
                db_result
            }
            Err(e) => {
                e.emit_warning();
                let err_msg = format!("Error loading crate: {}", e);
                handlers::chat::add_msg_immediate(state, event_bus, Uuid::new_v4(), err_msg, 
                    chat_history::MessageKind::SysInfo).await;
            return;
            }
        };
        debug!(save_crate_focus = ?crate_focus);

        let file_dir = default_dir.join(crate_name_version);
        info!("Checking for previous database file {}", file_dir.display());
        if let Ok(mut read_dir) = std::fs::read_dir(&default_dir) {
            info!("reading dir result\n{:?}", read_dir);
            while let Some(Ok(file)) = read_dir.next() {
                if file.path() == file_dir {
                    let _ = std::fs::remove_file(&file_dir).inspect_err(|e| {
                        error!(
                            "Error removing previous database file {}",
                            file_dir.display()
                        );
                    });
                }
            }
        }
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
    } else {
        // Explicitly surface a message if no active crate is selected
        let msg = "No active crate selected. Use `/index start <path>` or `/load crate <name>` before saving the database.".to_string();
        handlers::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg,
            chat_history::MessageKind::SysInfo,
        )
        .await;
    }
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
            root_path: None,
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
                root_path: None,
                is_success: false,
                error: Some(err_msg),
            }));
            Err(error)
        }
        Err(e) => {
            // TODO: Improve this error message
            error!("Failed to load file: {}", e);
            let err_msg = "Could not find saved file, io error";
            event_bus.send(AppEvent::System(SystemEvent::LoadDb {
                crate_name: crate_name.clone(),
                file_dir: Some(Arc::new(default_dir)),
                root_path: None,
                is_success: false,
                error: Some(err_msg),
            }));
            Err(ploke_error::FatalError::DefaultConfigDir { msg: err_msg })?
        }
    }?;

    let prior_rels_vec = state.db.relations_vec()?;
    debug!("prior rels for import: {:#?}", prior_rels_vec);
    state
        .db
        .import_from_backup(&valid_file, &prior_rels_vec)
        .map_err(ploke_db::DbError::from)
        .map_err(ploke_error::Error::from)?;

    // by default, use sentence-transformers model with sane hnsw settings from lazy static
    let default_model = VECTOR_DIMENSION_SPECS[0].clone();
    ploke_db::create_index_primary(&state.db, default_model)?;
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
                        error!(msg);
                        ploke_error::Error::Warning(ploke_error::WarningError::PlokeDb(
                            msg.to_string(),
                        ))
                    })
                    .map(|v| v.get_str().expect("Crate must always be a string"))?;
                // crate_root_path is expected to be absolute from DB context; use directly
                let root_path = std::path::PathBuf::from(crate_root_path);
                system_guard.crate_focus = Some(root_path.clone());
                // Also update IoManager roots for IO-level enforcement
                debug!(load_db_crate_focus = ?root_path);
                drop(system_guard);
                state
                    .io_handle
                    .update_roots(
                        Some(vec![root_path.clone()]),
                        Some(ploke_io::path_policy::SymlinkPolicy::DenyCrossRoot),
                    )
                    .await;
                event_bus.send(AppEvent::System(SystemEvent::LoadDb {
                    crate_name,
                    file_dir: Some(Arc::new(valid_file)),
                    root_path: Some(Arc::new(root_path)),
                    is_success: true,
                    error: None,
                }));
            }
            Ok(())
        }
        Ok(_count) => {
            event_bus.send(AppEvent::System(SystemEvent::LoadDb {
                crate_name,
                file_dir: Some(Arc::new(valid_file)),
                root_path: None,
                is_success: false,
                error: Some("Database backed up from file, but 0 relations found."),
            }));
            Ok(())
        }
        Err(e) => Err(e),
    }
}

#[cfg(feature = "test_harness")]
pub async fn test_set_crate_focus_from_db(
    state: &Arc<AppState>,
    crate_name: String,
) -> Result<(), ploke_error::Error> {
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
            error!(msg);
            ploke_error::Error::Warning(ploke_error::WarningError::PlokeDb(msg.to_string()))
        })
        .map(|v| v.get_str().expect("Crate must always be a string"))?;
    let root_path = std::path::PathBuf::from(crate_root_path);
    {
        let mut system_guard = state.system.write().await;
        system_guard.crate_focus = Some(root_path.clone());
    }
    state
        .io_handle
        .update_roots(
            Some(vec![root_path]),
            Some(ploke_io::path_policy::SymlinkPolicy::DenyCrossRoot),
        )
        .await;
    Ok(())
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
        error!("Missing crate focus, cannot scan unspecified target crate");
        let e = PlokeError::from(StateError::MissingCrateFocus {
            msg: "Missing crate focus is None, cannot scan unspecified target crate",
        });
        e.emit_warning();
        e
    })?;
    let crate_name = crate_path.file_name().and_then(|os_str| os_str.to_str()).ok_or_else(|| { 
        error!("Crate name is empty, cannot scan empty crate name");
        let e = PlokeError::from(StateError::MissingCrateFocus {msg: "Missing crate focus is empty or non-utf8 string, cannot scan unspecified target crate"});
        e.emit_warning();
        e
    })?;

    info!("scan_for_change in crate_name: {}", crate_name);
    // 2. get the files in the target project from the db, with hashes
    let file_data = state.db.get_crate_files(crate_name)?;
    trace!(target: SCAN_CHANGE, "file_data: {:#?}", file_data);

    // 2.5. Check for files that have been removed
    let (file_data, removed_file_data): ( Vec<_>, Vec<_> ) = file_data.into_iter().partition(|f| f.file_path.exists());

    // 3. scan the files, returning a Vec<Option<FileData>>, where None indicates the file has not
    //    changed.
    //  - Note that this does not do anything for those files which may have been added, which will
    //  be handled in parsing during the IndexFiles event process mentioned in step 5 below.
    let result = state.io_handle.scan_changes_batch(file_data).await.inspect_err(|e| {
            error!("Error in state.io_handle.scan_changes_batch: {e}");
    })?;
    let vec_ok = result?;

    if !vec_ok.iter().any(|f| f.is_some()) && removed_file_data.is_empty() {
        // 4. if no changes, send complete in oneshot
        match scan_tx.send(None) {
            Ok(()) => {
                info!("No file changes detected");
            }
            Err(e) => {
                error!("Error sending parse oneshot from ScanForChange");
            }
        };
    } else {
        // 5. if changes, send IndexFiles event (not yet made) or handle here.
        //  Let's see how far we get handling it here first.
        //  - Since we are parsing the whole target in any case, we might as well do it
        //  concurrently. Test sequential approach first, then move to be parallel earlier.

        // TODO: Move this into `syn_parser` probably
        // WARN: Just going to use a quick and dirty approach for now to get proof of concept, then later
        // on I'll do something more efficient.
        let ParserOutput { mut merged, tree } =
            run_parse_no_transform(Arc::clone(&state.db), Some(crate_path.clone()))?;

        // get the filenames to send through the oneshot
        let changed_filenames = vec_ok
            .iter()
            .filter_map(|opt| opt.as_ref().map(|f| f.file_path.clone()))
            .chain(removed_file_data.iter().map(|f| f.file_path.clone()))
            .collect_vec();
        for file in changed_filenames.iter() {
            let filename = format!("{}", file.display());
            info!(target:"file_hashes", "Checking for details on {}", filename);
            let query_res = state.db.get_path_info(&filename)?;
            info!(target:"file_hashes", "headers:\n{}", query_res.headers.iter().join(", ") );
            let rows = query_res
                .rows
                .iter()
                .map(|r| r.iter().join(", "))
                .join("\n");
            info!(target:"file_hashes", "rows:\n {}", rows);
        }
        // WARN: Half-assed implementation, this should be a recursive function instead of simple
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
        info!(
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
            trace!(
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

        trace!("Nodes in union set:");
        let printable_union_items = printable_nodes(&merged, filtered_union.iter());
        trace!("prinable_union_items:\n{}", printable_union_items);
        // filter relations
        merged.graph.relations.retain(|r| {
            filtered_union.contains(&r.source()) || filtered_union.contains(&r.target())
        });
        // filter nodes
        merged.retain_all(filtered_union);

        transform_parsed_graph(&state.db, merged, &tree).inspect_err(|e| {
            error!("Error transforming partial graph into database:\n{e}");
        })?;

        for file_id in module_uuids {
            for node_ty in NodeType::primary_nodes() {
                info!("Retracting type: {}", node_ty.relation_str());
                let query_res = state
                    .db
                    .retract_embedded_files(file_id, node_ty)
                    .inspect_err(|e| error!("Error in retract_embed_files: {e}"))?;
                trace!("Raw return of retract_embedded_files:\n{:?}", query_res);
                let to_print = query_res
                    .rows
                    .iter()
                    .map(|r| r.iter().join(" | "))
                    .join("\n");
                info!("Return of retract_embedded_files:\n{}", to_print);
            }
        }

        trace!("Finishing scanning, sending message to reindex workspace");
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
        .inspect_err(|e| error!("{e}"));
    info!(target: "write_query", "testing query result\n{:#?}", result);
    if let Ok(named_rows) = result {
        let mut output = String::new();
        let (header, rows) = (named_rows.headers, named_rows.rows);
        let cols_num = header.len();
        let display_header = header.into_iter().map(|h| h.to_string()).join("|");
        info!(target: "write_query", "\n{display_header}");
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
                    .map(|c| c.to_string())
                    .join("|")
            })
            .for_each(|r| {
                info!(target: "write_query", "\n{}", r);
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
                error!(target: "write_query", "Error writing query output to file {e}")
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

    #[cfg(feature = "multi_embedding")]
    fn embedding_set_for_runtime(
        cfg: &crate::app_state::core::RuntimeConfig,
        shape: EmbeddingShape,
    ) -> EmbeddingSetId {
        match &cfg.embedding {
            EmbeddingConfig {
                local: Some(local_cfg),
                ..
            } => EmbeddingSetId::new(
                EmbeddingProviderSlug::new_from_str("local-transformers"),
                EmbeddingModelId::new_from_str(local_cfg.model_id.clone()),
                shape,
            ),
            EmbeddingConfig {
                hugging_face: Some(hf_cfg),
                ..
            } => EmbeddingSetId::new(
                EmbeddingProviderSlug::new_from_str("huggingface"),
                EmbeddingModelId::new_from_str(hf_cfg.model.clone()),
                shape,
            ),
            EmbeddingConfig {
                openai: Some(openai_cfg),
                ..
            } => EmbeddingSetId::new(
                EmbeddingProviderSlug::new_from_str("openai"),
                EmbeddingModelId::new_from_str(openai_cfg.model.clone()),
                shape,
            ),
            EmbeddingConfig {
                cozo: Some(_cozo_cfg),
                ..
            } => EmbeddingSetId::new(
                EmbeddingProviderSlug::new_from_str("cozo"),
                EmbeddingModelId::new_from_str("cozo"),
                shape,
            ),
            _ => EmbeddingSetId::new(
                EmbeddingProviderSlug::new_from_str("local-transformers"),
                EmbeddingModelId::new_from_str("sentence-transformers/all-MiniLM-L6-v2"),
                shape,
            ),
        }
    }

    #[cfg(feature = "multi_embedding")]
    let maybe_embedding_set: Option<EmbeddingSetId> = if state.db.multi_embedding_db_enabled() {
        let runtime_cfg = {
            let guard = state.config.read().await;
            guard.clone()
        };
        let shape = state.embedder.shape();
        Some(embedding_set_for_runtime(&runtime_cfg, shape))
    } else {
        None
    };

    for (prompt_idx, prompt_item) in prompt_data.into_iter().enumerate() {
        let query_params: QueryParams = (&prompt_item).into();
        let PromptData {
            prompt,
            k,
            ef,
            max_hits,
            radius,
        } = prompt_item;
        info!("Processing prompt {}: {}", prompt_idx, prompt);

        let embeddings = state
            .embedder
            .generate_embeddings(vec![prompt.clone()])
            .await?;

        if let Some(embedding) = embeddings.into_iter().next() {
            for ty in NodeType::primary_nodes() {
                let ef_range = 1..=101;

                #[cfg(feature = "multi_embedding")]
                let EmbedDataVerbose { typed_data, dist } = if let Some(set_id) = &maybe_embedding_set {
                    let args = SimilarArgsForSet {
                        db: &state.db,
                        vector_query: &embedding,
                        k,
                        ef,
                        ty,
                        max_hits,
                        radius,
                        set_id,
                    };
                    search_similar_args_for_set(args)?
                } else {
                    let args = SimilarArgs {
                        db: &state.db,
                        vector_query: &embedding,
                        k,
                        ef,
                        ty,
                        max_hits,
                        radius,
                    };
                    search_similar_args(args)?
                };

                #[cfg(not(feature = "multi_embedding"))]
                let EmbedDataVerbose { typed_data, dist } = {
                    let args = SimilarArgs {
                        db: &state.db,
                        vector_query: &embedding,
                        k,
                        ef,
                        ty,
                        max_hits,
                        radius,
                    };
                    search_similar_args(args)?
                };
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

    use std::{ops::Index, path::PathBuf, sync::Once};

    use cozo::DataValue;
    use ploke_db::{create_index_primary, Database, QueryResult};
    use ploke_embed::local::EmbeddingConfig;
    use ploke_rag::RagService;
    use syn_parser::parser::nodes::ToCozoUuid;
    use tracing::{debug, trace};

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
    use crate::test_utils::new_test_harness::TEST_DB_NODES;
    use ploke_error::Error;

    static TEST_TRACING: Once = Once::new();
    fn init_tracing_once() {
        TEST_TRACING.call_once(|| {
            ploke_test_utils::init_test_tracing(tracing::Level::ERROR);
        });
    }

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
        trace!("reading from backup files: {}", backup_file.display());
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

        let config = config::Config::builder()
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

        debug!("Registry prefs loaded: {:#?}", config.registry);
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
            create_proposals: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            rag: Some(Arc::new(rag)),
            budget: TokenBudget::default(), // rag_tx: rag_event_tx.clone()
        });
        {
            let mut system_guard = state.system.write().await;
            let path = workspace_root.join(workspace);
            system_guard.crate_focus = Some(path);
            trace!("system_guard.crate_focus: {:?}", system_guard.crate_focus);
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
        trace!("rows from db:\n{printable_rows}");

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
                    trace!("IndexStatus Running");
                }
                IndexingStatus {
                    status: IndexStatus::Completed,
                    ..
                } => {
                    trace!("IndexStatus Completed, breaking loop");
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
        trace!("rows from db:\n{printable_rows}");

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
        trace!("target_file before pushes:\n{}", target_file.display());
        target_file.push("src");
        target_file.push("main.rs");
        trace!("target_file after pushes:\n{}", target_file.display());

        // ----- start test function ------
        let (scan_tx, scan_rx) = oneshot::channel();
        let result = scan_for_change(&state.clone(), &event_bus.clone(), scan_tx).await;
        trace!("result of scan_for_change: {:?}", result);
        // ----- end test start test ------

        trace!("waiting for scan_rx");

        // ----- await on end of test function `scan_for_change` -----
        match scan_rx.await {
            Ok(_) => trace!("scan_rx received for end of scan_for_change"),
            Err(_) => trace!("error in scan_rx awaiting on end of scan_for_change"),
        };

        // print database output after scan
        let query_result = db_handle.raw_query(script)?;
        let printable_rows = query_result
            .rows
            .iter()
            .map(|r| r.iter().join(", "))
            .join("\n");
        trace!("rows from db:\n{printable_rows}");

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
        trace!("reading file:\n{}", &contents);
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
        trace!("writing changed file:\n{}", &changed);
        std::fs::write(&target_file, changed)?;

        // ----- start second scan -----
        let (scan_tx, scan_rx) = oneshot::channel();
        let result = scan_for_change(&state.clone(), &event_bus.clone(), scan_tx).await;
        trace!("result of after second scan_for_change: {:?}", result);
        // ----- end second scan -----

        // print database output after second scan
        let query_result = db_handle.raw_query(script)?;
        let printable_rows = query_result
            .rows
            .iter()
            .map(|r| r.iter().join(", "))
            .join("\n");
        trace!("rows from db:\n{printable_rows}");

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
                    trace!("IndexStatus Running");
                }
                IndexingStatus {
                    status: IndexStatus::Completed,
                    ..
                } => {
                    trace!("IndexStatus Completed, breaking loop");
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
        debug!("rows from db:\n{printable_rows}");

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

        trace!("changing back:\n{}", target_file.display());
        let contents = std::fs::read_to_string(&target_file)?;
        trace!("reading file:\n{}", &contents);
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
        trace!("writing changed file:\n{}", &changed);
        std::fs::write(&target_file, changed)?;
        Ok(())
    }

    #[cfg(feature = "multi_embedding")]
    #[tokio::test]
    async fn search_for_set_returns_results_for_seeded_set() -> Result<(), Error> {
        use ploke_core::{EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSetId, EmbeddingShape};
        use ploke_db::multi_embedding::schema::vector_dims::vector_dimension_specs;
        use ploke_db::MultiEmbeddingRuntimeConfig;
        use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource};
        use ploke_embed::local::{EmbeddingConfig, LocalEmbedder};

        init_tracing_once();

        // DB with multi-embedding fixtures and runtime config enabled
        let raw_db = ploke_test_utils::setup_db_full("fixture_nodes")?;
        let config = MultiEmbeddingRuntimeConfig::from_env().enable_multi_embedding_db();
        let db = Arc::new(ploke_db::Database::with_multi_embedding_config(raw_db, config));

        // Pick a pending node and seed a runtime embedding for the first dimension spec.
        let batches = db.get_unembedded_node_data(1, 0)?;
        let (node_type, node) = batches
            .into_iter()
            .find_map(|typed| typed.v.into_iter().next().map(|entry| (typed.ty, entry)))
            .expect("at least one pending node");

        let dim_spec = vector_dimension_specs()
            .first()
            .expect("at least one vector dimension spec");
        let vector = vec![0.5_f32; dim_spec.dims() as usize];
        db.update_embeddings_batch(vec![(node.id, vector.clone())]).await?;

        // Ensure HNSW indexes exist for this type (including multi-embedding indexes).
        create_index_primary(&db)?;

        // Embedding processor: use the default local embedder (384 dims) so the query
        // embedding shape matches the first dimension spec.
        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));

        let rag = RagService::new(db.clone(), embedding_processor)?;

        // Build an EmbeddingSetId that matches the seeded dimension spec.
        let shape = EmbeddingShape::f32_raw(dim_spec.dims() as u32);
        let set_id = EmbeddingSetId::new(
            EmbeddingProviderSlug::new_from_str(dim_spec.provider().to_string()),
            EmbeddingModelId::new_from_str(dim_spec.embedding_model().to_string()),
            shape,
        );

        // Use a generic query; we only assert that the seeded node appears somewhere.
        let hits: Vec<(Uuid, f32)> = rag.search_for_set("generic query", 10, &set_id).await?;
        assert!(
            hits.iter().any(|(id, _)| *id == node.id),
            "expected set-aware dense search to return the seeded node"
        );

        Ok(())
    }

    #[cfg(feature = "multi_embedding")]
    #[tokio::test]
    async fn search_for_set_falls_back_when_multi_embedding_disabled() -> Result<(), Error> {
        use ploke_core::{EmbeddingSetId, EmbeddingShape};
        use ploke_core::{EmbeddingModelId, EmbeddingProviderSlug};
        use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource};
        use ploke_embed::local::{EmbeddingConfig, LocalEmbedder};

        init_tracing_once();

        // Legacy-style DB without multi_embedding enabled.
        let db = Arc::new(ploke_db::Database::init_with_schema()?);

        // Use the existing dense search test fixture DB for embeddings.
        let db_nodes = TEST_DB_NODES
            .as_ref()
            .expect("TEST_DB_NODES must initialize")
            .clone();

        // Swap in the pre-populated DB handle so we have real embeddings/HNSW state.
        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db_nodes.clone(), embedding_processor)?;

        // Build a dummy EmbeddingSetId; because multi_embedding is disabled on the
        // Database, search_for_set should transparently fall back to legacy search.
        let shape = EmbeddingShape::f32_raw(384);
        let dummy_set = EmbeddingSetId::new(
            EmbeddingProviderSlug::new_from_str("local-transformers"),
            EmbeddingModelId::new_from_str("sentence-transformers/all-MiniLM-L6-v2"),
            shape,
        );

        let search_term = "use_all_const_static";
        let hits: Vec<(Uuid, f32)> = rag.search_for_set(search_term, 15, &dummy_set).await?;
        assert!(
            !hits.is_empty(),
            "expected search_for_set to fall back and return results when multi_embedding is disabled"
        );

        Ok(())
    }

    #[cfg(feature = "multi_embedding")]
    #[tokio::test]
    async fn batch_prompt_search_uses_set_aware_search_when_enabled(
    ) -> color_eyre::Result<()> {
        use crate::app_state::core::{AppState, ChatState, ConfigState, RuntimeConfig, SystemState};
        use crate::chat_history::ChatHistory;
        use crate::user_config::UserConfig;
        use ploke_core::EmbeddingData;
        use ploke_db::{Database, MultiEmbeddingRuntimeConfig};
        use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource};
        use ploke_embed::local::{EmbeddingConfig as LocalEmbeddingConfig, LocalEmbedder};
        use ploke_io::IoManagerHandle;
        use ploke_rag::TokenBudget;
        use ploke_test_utils::workspace_root;
        use std::fs;
        use tempfile::NamedTempFile;
        use tracing::Level;

        ploke_test_utils::init_test_tracing(Level::ERROR);

        // Multi-embedding DB from shared fixture.
        let raw_db = ploke_test_utils::setup_db_full("fixture_nodes")?;
        let config = MultiEmbeddingRuntimeConfig::from_env().enable_multi_embedding_db();
        let db = Database::with_multi_embedding_config(raw_db, config);

        // Embedding processor configured like the runtime.
        let user_cfg = UserConfig::default();
        let runtime_cfg: crate::app_state::core::RuntimeConfig = user_cfg.clone().into();
        let local_cfg = LocalEmbeddingConfig::default();
        let model = LocalEmbedder::new(local_cfg)?;
        let source = EmbeddingSource::Local(model);
        let embedder = Arc::new(EmbeddingProcessor::new(source));

        let io_handle = IoManagerHandle::new();

        let state = Arc::new(AppState {
            chat: ChatState::new(ChatHistory::new()),
            config: ConfigState::new(runtime_cfg.clone()),
            system: SystemState::default(),
            indexing_state: tokio::sync::RwLock::new(None),
            indexer_task: None,
            indexing_control: Arc::new(tokio::sync::Mutex::new(None)),
            db: Arc::new(db),
            embedder: embedder.clone(),
            io_handle: io_handle.clone(),
            proposals: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            create_proposals: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            rag: None,
            budget: TokenBudget::default(),
        });

        // Prompt file targeting a known symbol in the fixture crate.
        let mut tmp = NamedTempFile::new()?;
        let prompts = serde_json::json!([{
            "prompt": "use_all_const_static",
            "k": 8,
            "ef": 16,
            "max_hits": 8,
            "radius": 10.0
        }]);
        fs::write(tmp.path(), serde_json::to_string(&prompts)?)?;

        let out_file = tmp.path().with_extension("out.json");
        let batches = batch_prompt_search(
            &state,
            tmp.path().to_string_lossy().into_owned(),
            out_file.to_string_lossy().into_owned(),
            Some(8),
            None,
        )
        .await?;

        assert!(
            !batches.is_empty(),
            "expected batch_prompt_search to return at least one batch"
        );
        assert!(
            batches
                .iter()
                .flat_map(|b| &b.snippet_info)
                .any(|info| info.name.contains("use_all_const_static")),
            "expected at least one snippet mentioning the search term"
        );

        Ok(())
    }
}
