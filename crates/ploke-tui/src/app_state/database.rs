use std::{
    collections::{HashMap, HashSet, VecDeque},
    ops::ControlFlow,
};

use itertools::Itertools;
use ploke_core::{EmbeddingData, FileData, NodeId};
use ploke_db::{
    EmbedDataVerbose, NodeType, RestoredEmbeddingSet, SimilarArgs,
    multi_embedding::schema::EmbeddingSetExt, search_similar_args,
};
use ploke_embed::config::OpenRouterConfig;
use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource};
use ploke_embed::providers::openrouter::OpenRouterBackend;
use ploke_embed::runtime::EmbeddingRuntime;
use ploke_transform::transform::transform_parsed_graph;
use serde::{Deserialize, Serialize};
use syn_parser::{
    ModuleTree, TestIds,
    parser::{
        nodes::{AnyNodeId, AsAnyNodeId as _, ModuleNodeId, PrimaryNodeId},
        relations::SyntacticRelation,
    },
    resolve::{RelationIndexer, TreeRelation},
};
use tokio::sync::oneshot;
use tracing::{debug, error, info, trace, warn};

use crate::{
    app_state::helpers::{print_module_set, printable_nodes},
    parser::{ParserOutput, run_parse_no_transform},
    tracing_setup::SCAN_CHANGE,
    utils::helper::find_file_by_prefix,
};

use super::*;

pub const TUI_DB_TARGET: &str = "tracing_db_target";
pub const TUI_SCAN_TARGET: &str = "scan-for-change";

/// Attempt to construct or reuse an embedder that matches the restored embedding set so the runtime
/// stays aligned with the database after a load.
fn build_embedder_for_restored_set(
    state: &Arc<AppState>,
    set: &ploke_core::embeddings::EmbeddingSet,
) -> Result<Option<Arc<EmbeddingProcessor>>, ploke_error::Error> {
    let target_dims = set.dims() as usize;

    // Fast path: reuse the current embedder if it already matches the restored dimensions.
    if let (Ok(curr_dim), Ok(curr_proc)) = (
        state.embedder.dimensions(),
        state.embedder.current_processor(),
    ) && curr_dim == target_dims
    {
        return Ok(Some(curr_proc));
    }

    let provider = set.provider.as_ref();
    match provider {
        "openrouter" => {
            let cfg = OpenRouterConfig {
                model: set.model.to_string(),
                dimensions: Some(target_dims),
                ..Default::default()
            };
            match OpenRouterBackend::new(&cfg) {
                Ok(backend) => Ok(Some(Arc::new(EmbeddingProcessor::new(
                    EmbeddingSource::OpenRouter(backend),
                )))),
                Err(e) => Err(ploke_error::Error::from(
                    ploke_error::WarningError::PlokeDb(format!(
                        "Failed to build OpenRouter embedder for {}: {}",
                        set.model, e
                    )),
                )),
            }
        }
        _ => {
            // Unknown provider; caller will surface a warning.
            Ok(None)
        }
    }
}

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
            Ok(db_result) => db_result,
            Err(e) => {
                e.emit_warning();
                let err_msg = format!("Error loading crate: {}", e);
                handlers::chat::add_msg_immediate(
                    state,
                    event_bus,
                    Uuid::new_v4(),
                    err_msg,
                    chat_history::MessageKind::SysInfo,
                )
                .await;
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

        // Persist the active embedding set selection so it survives backup/restore.
        if let Ok(active_set) = state.db.with_active_set(|set| set.clone()) {
            if let Err(e) = state
                .db
                .put_active_embedding_set_meta(crate_focus, &active_set)
            {
                error!("Failed to persist active embedding set for backup: {e}");
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

    state
        .db
        .import_backup_with_embeddings(&valid_file)
        .map_err(ploke_db::DbError::from)
        .map_err(ploke_error::Error::from)?;

    // Attempt to restore the active embedding set from metadata in the backup, falling back to the
    // first populated set.
    let selection = state
        .db
        .restore_embedding_set(&crate_name)
        .map_err(ploke_error::Error::from)?;
    if let Some((set, reason)) = selection {
        let reason_text = match reason {
            RestoredEmbeddingSet::FromMetadata => {
                format!(
                    "Restored embedding set '{}' from backup metadata.",
                    set.rel_name()
                )
            }
            RestoredEmbeddingSet::FirstPopulated => format!(
                "No metadata found; using first populated embedding set '{}' from backup.",
                set.rel_name()
            ),
        };
        handlers::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            reason_text.clone(),
            chat_history::MessageKind::SysInfo,
        )
        .await;
        info!("{reason_text}");

        // Build or reuse an embedder that matches the restored set and activate it so the runtime
        // and DB stay in sync.
        match build_embedder_for_restored_set(state, &set) {
            Ok(Some(new_embedder)) => {
                if let Err(e) =
                    state
                        .embedder
                        .activate(&state.db, set.clone(), Arc::clone(&new_embedder))
                {
                    let msg = format!(
                        "Restored embedding set '{}' but failed to activate runtime: {}. Code search may fail until you reselect an embedding model.",
                        set.rel_name(),
                        e
                    );
                    warn!("{msg}");
                    handlers::chat::add_msg_immediate(
                        state,
                        event_bus,
                        Uuid::new_v4(),
                        msg,
                        chat_history::MessageKind::SysInfo,
                    )
                    .await;
                } else {
                    let msg = format!(
                        "Switched embedding model to '{}' from backup (dims {}). Code search should work. Use `/embedding search <model>` to reindex with a different model.",
                        set.rel_name(),
                        set.dims()
                    );
                    handlers::chat::add_msg_immediate(
                        state,
                        event_bus,
                        Uuid::new_v4(),
                        msg.clone(),
                        chat_history::MessageKind::SysInfo,
                    )
                    .await;
                    info!("{msg}");
                }
            }
            Ok(None) => {
                let msg = format!(
                    "Restored embedding set '{}' but could not build a matching embedder (provider '{}', dims {}). Code search may fail until you reselect an embedding model.",
                    set.rel_name(),
                    set.provider.as_ref(),
                    set.dims()
                );
                warn!("{msg}");
                handlers::chat::add_msg_immediate(
                    state,
                    event_bus,
                    Uuid::new_v4(),
                    msg,
                    chat_history::MessageKind::SysInfo,
                )
                .await;
            }
            Err(e) => {
                let msg = format!(
                    "Restored embedding set '{}' but hit an error building embedder: {}. Code search may fail until you reselect an embedding model.",
                    set.rel_name(),
                    e
                );
                warn!("{msg}");
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

        ploke_db::create_index_for_set(&state.db, &set)?;
    } else {
        let msg = "No populated embedding set found after restore; embedding searches will be unavailable";
        warn!("{msg}");
        handlers::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            chat_history::MessageKind::SysInfo,
        )
        .await;
        return Err(ploke_error::WarningError::PlokeDb(msg.to_string()).into());
    }

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
    let (file_data, removed_file_data): (Vec<_>, Vec<_>) =
        file_data.into_iter().partition(|f| f.file_path.exists());

    // 3. scan the files, returning a Vec<Option<FileData>>, where None indicates the file has not
    //    changed.
    //  - Note that this does not do anything for those files which may have been added, which will
    //  be handled in parsing during the IndexFiles event process mentioned in step 5 below.
    let result = state
        .io_handle
        .scan_changes_batch(file_data)
        .await
        .inspect_err(|e| {
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

        // get the changed (altered or removed) filenames to send through the oneshot
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
                mod_id, printable_nodes
            );
        }
        fn mods_in_file(
            current: ModuleNodeId,
            mut mods: HashSet<AnyNodeId>,
            tree: &ModuleTree,
        ) -> HashSet<AnyNodeId> {
            // start_len is probably unneeded, try running tests and removing
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

        use SyntacticRelation::*;
        let is_parent_filter = |tr: &TreeRelation| {
            let r = tr.rel();
            matches!(
                r,
                Contains { .. }
                    | ModuleImports { .. }
                    | ReExports { .. }
                    | StructField { .. }
                    | UnionField { .. }
                    | VariantField { .. }
                    | EnumVariant { .. }
                    | ImplAssociatedItem { .. }
                    | TraitAssociatedItem { .. }
            )
        };

        fn nodes_in_file(
            current: ModuleNodeId,
            mut mods: HashSet<AnyNodeId>,
            tree: &ModuleTree,
        ) -> HashSet<AnyNodeId> {
            // start_len is probably unneeded, try running tests and removing
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

        // Goal: List all nodes (primary, secondary, all kinds) that have changed.
        //
        // 1. For nodes that have a tracking_hash (all primary_node types of relevance for vector
        // embedding) this means checking the tracking_hash of the recently parsed (present state)
        // against the last known tracking_hash stored in the db (past state).
        //  - A) if past state != present state,
        //      -> then the node has changed (and needs to be embedded)
        //  - B) if past state DNE,
        //      -> then the item is new (and needs to be embedded)
        //  - C) if past state exists, but present state DNE,
        //      -> then the item needs to be pruned from the database
        //
        // For case 1.A, we need to remove the previous item and insert the new item (see notes on
        // using synthetic node ids in note on 2.A below)
        //
        // 2. For nodes that do not have a tracking_hash:
        //  - A) if parent past state != present state,
        //      -> then the node parent has changed (and the link pointing to the parent can stay)
        //  - B) if past state DNE,
        //      -> then the item is new (and will have been linked already)
        //  - C) if past state exists, but present state DNE,
        //      -> then the item needs to be pruned from the database
        //
        // NOTE: For case 2.C, because there is not a tracking_hash on the node that can be used to
        // determine whether or not the item has changed, we need to rely on traversing the
        // relations that indicate a parent-child relationship to find the 2.C items.
        //
        // NOTE: For case 2.A, since we do not know whether the item itself has changed or not, we
        // cannot determine whether or not it needs to be replaced (at least based on the
        // tracking_hash). Ideally, we would be able to use the NodeId as a heuristic to tell if
        // the node has changed sufficiently to remove the previous node (and all the previous
        // node's relations) and add a new one, or to update the previous node (leaving the
        // identifier and other relations unchanged).
        // At the time of writing (2025-12-07), we use synthetic nodes everywhere, and they are
        // constructed from file_path, relative module path, item name, and span start/end.
        // Therefore even if an item has not changed, but a newline has been added above that item,
        // then the NodeId will have changed, and so the item must first be removed from the
        // database before being added and linked again.

        // initial set of node ids to iterate over
        //
        // module_set is the set of modules node ids that own the files which have changed.
        let mut new_item_set_deq: VecDeque<AnyNodeId> =
            module_set.iter().map(|m_id| m_id.as_any()).collect();
        // hashset to hold unique ids of all node ids in the changed/removed files
        let mut items_in_file: HashSet<AnyNodeId> = HashSet::new();

        // recursively iterate over all items that may have changed in the target files by
        // traversing all relations which indicate a parent (source) -> child (target) relation
        // where the nodes in the changed set are the parent/source.
        while let Some(source_id) = new_item_set_deq.pop_front() {
            let is_unique = items_in_file.insert(source_id);
            if !is_unique {
                tracing::warn!("Non-unique node id: {source_id}");
            }
            let next_items = tree
                .get_relations_from(&source_id, is_parent_filter)
                .into_iter()
                .flat_map(|v| v.into_iter())
                .map(|tr| tr.rel().target());
            new_item_set_deq.extend(next_items);
        }

        // 1. get all node ids in the database that are in a changed file
        //  - call them db_nodes
        // 2. compare db_node ids (previous state) to parsed node ids (present state)
        //  -> if match, then compare to tracking_hash (aka TH)
        //      -> if match, then
        //          => no embedding, no update
        //      -> if no TH match, then
        //          => new embedding, update old node_id (unlikely given byte spans used in synth node id)
        //  -> if no match, add node_id to a list to be removed from database
        // 3.
        for item in items_in_file {

            // let is_db_tracking_hash = state.db;
        }
        // let new_item_set: HashSet<AnyNodeId> = module_set
        //     .iter()
        //     .filter_map();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use cozo::{DataValue, ScriptMutability, UuidWrapper};
    use ploke_core::embeddings::{EmbeddingModelId, EmbeddingProviderSlug, EmbeddingShape};
    use ploke_db::multi_embedding::debug::DebugAll;
    use ploke_embed::indexer::EmbeddingProcessor;
    use ploke_transform::schema::crate_node::CrateContextSchema;
    use tempfile::TempDir;

    const HNSW_SUFFIX: &str = ":hnsw_idx";

    fn build_state(
        db: Arc<ploke_db::Database>,
        embedder: Arc<ploke_embed::runtime::EmbeddingRuntime>,
    ) -> Arc<AppState> {
        Arc::new(AppState {
            chat: ChatState::new(chat_history::ChatHistory::new()),
            config: ConfigState::new(RuntimeConfig::from(
                crate::user_config::UserConfig::default(),
            )),
            system: SystemState::default(),
            indexing_state: tokio::sync::RwLock::new(None),
            indexer_task: None,
            indexing_control: Arc::new(tokio::sync::Mutex::new(None)),
            db,
            embedder,
            io_handle: ploke_io::IoManagerHandle::new(),
            rag: None,
            budget: ploke_rag::TokenBudget::default(),
            proposals: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            create_proposals: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        })
    }

    fn custom_set() -> ploke_core::embeddings::EmbeddingSet {
        ploke_core::embeddings::EmbeddingSet::new(
            EmbeddingProviderSlug::new_from_str("openrouter"),
            EmbeddingModelId::new_from_str("mistralai/codestral-embed-2505"),
            EmbeddingShape::new_dims_default(3),
        )
    }

    #[tokio::test]
    async fn load_db_restores_saved_embedding_set_and_index() {
        let tmp_config = TempDir::new().expect("temp config dir");
        let old_xdg = std::env::var("XDG_CONFIG_HOME").ok();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", tmp_config.path());
        }

        let crate_name = "fixture_crate";
        let crate_root = tmp_config.path().join(crate_name);
        std::fs::create_dir_all(&crate_root).expect("crate root dir");

        // Session 1: choose non-default embedding set, persist data, and back up.
        let db = Arc::new(ploke_db::Database::init_with_schema().expect("db init"));
        db.setup_multi_embedding().expect("multi embed setup");
        let embedder = Arc::new(ploke_embed::runtime::EmbeddingRuntime::from_shared_set(
            Arc::clone(&db.active_embedding_set),
            EmbeddingProcessor::new_mock(),
        ));
        let state = build_state(Arc::clone(&db), Arc::clone(&embedder));
        state
            .system
            .set_crate_focus_for_test(crate_root.clone())
            .await;

        // Insert crate context so backup naming and crate_focus restore work.
        let ns = uuid::Uuid::new_v4();
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::Uuid(UuidWrapper(ns)));
        params.insert("name".to_string(), DataValue::from(crate_name));
        params.insert("version".to_string(), DataValue::from("0.1.0"));
        params.insert("namespace".to_string(), DataValue::Uuid(UuidWrapper(ns)));
        params.insert(
            "root_path".to_string(),
            DataValue::from(crate_root.display().to_string()),
        );
        params.insert("files".to_string(), DataValue::List(vec![]));
        let script = CrateContextSchema::SCHEMA.script_put(&params);
        db.run_script(&script, params, ScriptMutability::Mutable)
            .expect("crate_context put");

        let target_set = custom_set();
        embedder
            .activate(
                &db,
                target_set.clone(),
                Arc::new(EmbeddingProcessor::new_mock()),
            )
            .expect("activate custom set");

        // Insert one embedding and create the index so restore sees a populated set.
        let node_id = uuid::Uuid::new_v4();
        db.update_embeddings_batch(vec![(node_id, vec![0.1, 0.2, 0.3])])
            .expect("update embeddings");
        assert_eq!(
            db.count_embeddings_for_set(&target_set)
                .expect("count before backup"),
            1,
            "pre-backup embedding count"
        );
        ploke_db::create_index_for_set(&db, &target_set).expect("create index for set");
        db.put_active_embedding_set_meta(crate_name, &target_set)
            .expect("persist active set metadata");

        let bus = Arc::new(EventBus::new(EventBusCaps::default()));
        save_db(&state, &bus).await;

        // Session 2: start with default set/runtime and load from backup.
        let fresh_db = Arc::new(ploke_db::Database::init_with_schema().expect("db init"));
        fresh_db.setup_multi_embedding().expect("multi embed setup");
        let fresh_embedder = Arc::new(ploke_embed::runtime::EmbeddingRuntime::from_shared_set(
            Arc::clone(&fresh_db.active_embedding_set),
            EmbeddingProcessor::new_mock(),
        ));
        let fresh_state = build_state(Arc::clone(&fresh_db), Arc::clone(&fresh_embedder));
        let bus2 = Arc::new(EventBus::new(EventBusCaps::default()));

        if let Err(err) = load_db(&fresh_state, &bus2, crate_name.to_string()).await {
            eprintln!("load_db error: {err:?}");
            let sets = fresh_state.db.list_embedding_sets().expect("list sets");
            eprintln!("sets after import: {:?}", sets);
            for set in &sets {
                let cnt = fresh_state
                    .db
                    .count_embeddings_for_set(set)
                    .expect("count after import");
                eprintln!("set {:?} count {}", set.rel_name(), cnt);
                let db_info: String = fresh_state
                    .db
                    .is_embedding_info_all(set)
                    .expect("show info idempotent")
                    .tracing_string_all();
                eprintln!("set {:?} is_embedding_info_all: {}", set, db_info);
            }
            panic!("load db failed");
        }

        let restored = fresh_state
            .db
            .with_active_set(|s| s.clone())
            .expect("active set");
        assert_eq!(restored, target_set);
        assert_eq!(
            fresh_state
                .db
                .count_embeddings_for_set(&target_set)
                .expect("count embeddings"),
            1
        );

        let hnsw_rel = format!("{}{}", target_set.rel_name(), HNSW_SUFFIX);
        let rels = fresh_state.db.relations_vec().expect("relations");
        assert!(
            rels.iter().any(|r| r == &hnsw_rel),
            "HNSW index for restored set should exist"
        );

        // Runtime embedder should now reflect the restored set dimensions.
        let runtime_dims = fresh_state.embedder.dimensions().expect("runtime dims");
        assert_eq!(runtime_dims, target_set.dims() as usize);

        let focus = fresh_state.system.crate_focus_for_test().await;
        assert_eq!(
            focus.as_deref(),
            Some(crate_root.as_path()),
            "crate focus should be restored from backup"
        );

        if let Some(old) = old_xdg {
            unsafe {
                std::env::set_var("XDG_CONFIG_HOME", old);
            }
        } else {
            unsafe {
                std::env::remove_var("XDG_CONFIG_HOME");
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
        info!("Processing prompt {}: {}", prompt_idx, prompt);

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
    use ploke_core::embeddings::EmbeddingSet;
    use ploke_db::{Database, QueryResult, multi_embedding::debug::DebugAll};
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
    use ploke_test_utils::{
        init_test_tracing, init_test_tracing_with_target, setup_db_full, setup_db_full_crate,
        workspace_root,
    };
    use thiserror::Error;

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

    fn is_id_embed_null(
        db_handle: &Database,
        ty: NodeType,
        id: AnyNodeId,
        embedding_set: EmbeddingSet,
    ) -> Result<bool> {
        let rel_name = ty.relation_str();
        let cozo_id = id.to_cozo_uuid();
        let vec_rel = embedding_set.rel_name;
        let one_script = format!(
            "?[name, item_id] := *{rel_name}{{ name, id: item_id @ 'NOW' }},
                *{vec_rel}{{ node_id: item_id @ 'NOW'}},
                item_id = {cozo_id}"
        );
        let query = db_handle.raw_query(&one_script)?;
        let count = query.rows.len();

        if count != 1 {
            let mut msg = format!("Expect node_id to be unique for item id: {id:?}");
            for (i, row) in query.rows.clone().into_iter().enumerate() {
                msg.push('\n');
                let i_num = format!("{i}: ");
                msg.push_str(&i_num);
                let mut cols = row.into_iter();
                let name_borrowed = cols.next().expect("cell not found");
                let name = name_borrowed
                    .get_str()
                    .expect("col name should be type &str");
                msg.push_str(name);
                let id_raw_cozo = cols.next().expect("cell not found");
                let id = ploke_db::to_uuid(&id_raw_cozo)?;
                let id_string_dbg = format!("{id:?}");
                msg.push_str(&id_string_dbg);
                tracing::error!(msg);
            }
        }
        assert_eq!(1, count, "expect node_id to be unique");
        let is_embedding_null_now = iter_col(&query, "name")
            .expect("column not found")
            .next()
            .is_some();
        // .expect("row not found")
        // .get_str()
        // .expect("cell not expected datatype (&str)");
        Ok(is_embedding_null_now)
    }

    fn is_name_embed_null(
        db_handle: &Database,
        ty: NodeType,
        name: &str,
        embedding_set: &EmbeddingSet,
    ) -> Result<bool> {
        let rel_name = ty.relation_str();
        let vec_rel = embedding_set.rel_name.clone();
        // Checks if the target relation with the specified name has a corresponding vector
        // embedding in the vector relation for the given embedding set.
        let one_script = format!(
            "?[item_name, id] := *{rel_name}{{ name: item_name, id @ 'NOW' }},
                *{vec_rel}{{ node_id: id @ 'NOW'}},
                item_name = \"{name}\""
        );
        tracing::debug!(%one_script);
        let query = db_handle.raw_query(&one_script)?;
        let count = query.rows.len();

        if count >= 1 {
            let mut msg = format!("Expect node_id to be unique for item name: {name:?}");
            for (i, row) in query.rows.clone().into_iter().enumerate() {
                msg.push('\n');
                let i_num = format!("{i}: ");
                msg.push_str(&i_num);
                let mut cols = row.into_iter();
                let name_borrowed = cols.next().expect("cell not found");
                let name = name_borrowed
                    .get_str()
                    .expect("col name should be type &str");
                msg.push_str(name);
                let id_raw_cozo = cols.next().expect("cell not found");
                let id = ploke_db::to_uuid(&id_raw_cozo)?;
                let id_string_dbg = format!("{id:?}");
                msg.push_str(&id_string_dbg);
                tracing::info!(msg);
            }
            assert_eq!(1, count, "expect node_id to be unique");
            let is_embedding_set_row_present_now = iter_col(&query, "item_name")
                .expect("column not found")
                .next()
                .is_some();
            Ok(!is_embedding_set_row_present_now)
        } else {
            tracing::info!(
                "No embedding found for \nname: {name}\nrelation node: {rel_name}\nvec_rel: {vec_rel}"
            );
            Ok(true)
        }
    }

    #[tokio::test]
    async fn test_update_embed() -> color_eyre::Result<()> {
        // if std::env::var("PLOKE_RUN_UPDATE_EMBED").ok().as_deref() != Some("1") {
        //     eprintln!("Skipping: PLOKE_RUN_UPDATE_EMBED!=1");
        //     return Ok(());
        // }
        init_test_tracing_with_target(TUI_SCAN_TARGET, tracing::Level::ERROR);
        let workspace_root = workspace_root();
        let target_crate = "fixture_update_embed";
        let workspace = "tests/fixture_crates/fixture_update_embed";

        // ensure file begins in same state by using backup
        let backup_file = PathBuf::from(format!(
            "{}/{}/src/backup_main.bak",
            workspace_root.display(),
            workspace
        ));
        trace!(target: TUI_SCAN_TARGET, "reading from backup files: {}", backup_file.display());
        let backup_contents = std::fs::read(&backup_file)?;
        let target_main = backup_file.with_file_name("main.rs");
        std::fs::write(&target_main, backup_contents)?;

        let cozo_db = if target_crate.starts_with("fixture") {
            ploke_test_utils::setup_db_full_multi_embedding(target_crate)
            // setup_db_full(target_crate)
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
        let processor = config.load_embedding_processor()?;
        let embedding_runtime = Arc::new(ploke_embed::runtime::EmbeddingRuntime::with_default_set(
            processor,
        ));

        let new_db =
            ploke_db::Database::new_with_active_set(cozo_db, embedding_runtime.active_set_handle());
        let db_handle = Arc::new(new_db);

        // Initial parse is now optional - user can run indexing on demand
        // run_parse(Arc::clone(&db_handle), Some(TARGET_DIR_FIXTURE.into()))?;

        // TODO: Change IoManagerHandle so it doesn't spawn its own thread, then use similar pattern to
        // spawning state meager below.
        let io_handle = ploke_io::IoManagerHandle::new();

        // TODO: These numbers should be tested for performance under different circumstances.
        let event_bus_caps = EventBusCaps::default();
        let event_bus = Arc::new(EventBus::new(event_bus_caps));

        // TODO:
        // 1 Implement the cancellation token propagation in IndexerTask
        // 2 Add error handling for embedder initialization failures
        let (index_cancellation_token, index_cancel_handle) = CancellationToken::new();
        let indexer_task = IndexerTask::new(
            db_handle.clone(),
            io_handle.clone(),
            Arc::clone(&embedding_runtime), // Use configured processor
            index_cancellation_token,
            index_cancel_handle,
            8,
        );

        let rag = RagService::new(Arc::clone(&db_handle), Arc::clone(&embedding_runtime))?;
        let state = Arc::new(AppState {
            chat: ChatState::new(ChatHistory::new()),
            config: ConfigState::default(),
            system: SystemState::default(),
            indexing_state: RwLock::new(None),
            indexer_task: Some(Arc::new(indexer_task)),
            indexing_control: Arc::new(Mutex::new(None)),
            db: db_handle.clone(),
            embedder: Arc::clone(&embedding_runtime),
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
            trace!(target: TUI_SCAN_TARGET, "system_guard.crate_focus: {:?}", system_guard.crate_focus);
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

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let active_embedding_set = db_handle.with_active_set(|set| set.clone())?;
        let embedding_set = active_embedding_set.clone();
        let vec_rel = embedding_set.rel_name.clone();
        // let script = r#"?[name, id, embedding] := *function{name, id, embedding @ 'NOW' }"#;
        let script = format!(
            r#"?[name, time, is_assert, maybe_null, id] := *function{{ id, at, name }}
                                or *struct{{ id, at, name }} 
                                or *module{{ id, at, name }} 
                                or *static{{ id, at, name }} 
                                or *const{{ id, at, name }}, 
                                  time = format_timestamp(at),
                                  *{vec_rel} {{ node_id @ 'NOW' }},
                                  maybe_null = ( node_id == id ),
                                  is_assert = to_bool(at)
        "#
        );
        let query_result = db_handle.raw_query(&script)?;
        let printable_rows = query_result
            .rows
            .iter()
            .map(|r| r.iter().join(", "))
            .join("\n");
        trace!(target: TUI_SCAN_TARGET, "rows from db:\n{printable_rows}");

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
                    trace!(target: TUI_SCAN_TARGET, "IndexStatus Running");
                }
                IndexingStatus {
                    status: IndexStatus::Completed,
                    ..
                } => {
                    trace!(target: TUI_SCAN_TARGET, "IndexStatus Completed, breaking loop");
                    break;
                }
                _ => {}
            }
        }

        // print database output after indexing
        // or *struct{name, id, embedding & 'NOW'}
        let query_result = db_handle.raw_query(&script)?;
        let printable_rows = query_result
            .rows
            .iter()
            .map(|r| r.iter().join(", "))
            .join("\n");
        trace!(target: TUI_SCAN_TARGET, "rows from db:\n{printable_rows}");

        // These items are defined in the backup file as:

        // --- items in as-yet unchanged file, ---
        // expect to be embedded initially
        // (before scan sets them to null again)

        // is in backup at
        // crate::inner_test_mod::double_inner_mod
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Module,
            "double_inner_mod",
            &embedding_set
        )?);
        // is in backup at
        // crate::inner_test_mod::NUMBER_ONE
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Const,
            "NUMBER_ONE",
            &embedding_set
        )?);
        // is in backup at
        // crate::inner_test_mod::double_inner_mod::STR_TWO
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Static,
            "STR_TWO",
            &embedding_set,
        )?);
        // is in backup at
        // crate::TestStruct
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Struct,
            "TestStruct",
            &embedding_set,
        )?);
        // is in backup at
        // crate::inner_test_mod
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Module,
            "inner_test_mod",
            &embedding_set,
        )?);
        // is in backup at
        // crate::func_with_params
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "func_with_params",
            &embedding_set,
        )?);
        // is in backup at
        // crate::main
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "main",
            &embedding_set
        )?);

        // ---

        // --- items not in changed file, expect to be remain embedded ---

        // is in backup at
        // crate::other_mod::simple_four
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "simple_four",
            &embedding_set,
        )?);
        // is in backup at
        // crate::other_mod::OtherStruct
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Struct,
            "OtherStruct",
            &embedding_set,
        )?);

        // ---

        let mut target_file = {
            let mut system_guard = state.system.write().await;
            system_guard.crate_focus = Some(workspace_root.join(workspace));
            system_guard
                .crate_focus
                .clone()
                .expect("Crate focus not set")
        };
        trace!(target: TUI_SCAN_TARGET, "target_file before pushes:\n{}", target_file.display());
        target_file.push("src");
        target_file.push("main.rs");
        trace!(target: TUI_SCAN_TARGET, "target_file after pushes:\n{}", target_file.display());

        // ----- start test function ------
        let (scan_tx, scan_rx) = oneshot::channel();
        let result = scan_for_change(&state.clone(), &event_bus.clone(), scan_tx).await;
        trace!(target: TUI_SCAN_TARGET, "result of scan_for_change: {:?}", result);
        // ----- end test start test ------

        trace!(target: TUI_SCAN_TARGET, "waiting for scan_rx");

        // ----- await on end of test function `scan_for_change` -----
        match scan_rx.await {
            Ok(_) => trace!(target: TUI_SCAN_TARGET, "scan_rx received for end of scan_for_change"),
            Err(_) => {
                trace!(target: TUI_SCAN_TARGET, "error in scan_rx awaiting on end of scan_for_change")
            }
        };

        // print database output after scan
        let query_result = db_handle.raw_query(&script)?;
        let printable_rows = query_result
            .rows
            .iter()
            .map(|r| r.iter().join(", "))
            .join("\n");
        trace!(target: TUI_SCAN_TARGET, "rows from db:\n{printable_rows}");

        // Nothing should have changed after running scan on the target when the target has not
        // changed.
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Const,
            "NUMBER_ONE",
            &embedding_set,
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Static,
            "STR_TWO",
            &embedding_set,
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Struct,
            "TestStruct",
            &embedding_set,
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Module,
            "double_inner_mod",
            &embedding_set,
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Module,
            "inner_test_mod",
            &embedding_set,
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "func_with_params",
            &embedding_set,
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "main",
            &embedding_set
        )?);
        // Same here
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "simple_four",
            &embedding_set,
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Struct,
            "OtherStruct",
            &embedding_set,
        )?);

        // ----- make change to target file -----
        let contents = std::fs::read_to_string(&target_file)?;
        trace!(target: TUI_SCAN_TARGET, "reading file:\n{}", &contents);
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
        trace!(target: TUI_SCAN_TARGET, "writing changed file:\n{}", &changed);
        std::fs::write(&target_file, changed)?;

        // ----- start second scan -----
        let (scan_tx, scan_rx) = oneshot::channel();
        let result = scan_for_change(&state.clone(), &event_bus.clone(), scan_tx).await;
        trace!(target: TUI_SCAN_TARGET, "result of after second scan_for_change: {:?}", result);
        // ----- end second scan -----

        // print database output after second scan
        let query_result = db_handle.raw_query(&script)?;
        let printable_rows = query_result
            .rows
            .iter()
            .map(|r| r.iter().join(", "))
            .join("\n");
        trace!(target: TUI_SCAN_TARGET, "rows from db:\n{printable_rows}");

        // items in changed file, expect to have null embeddings after scan
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Module,
            "double_inner_mod",
            &embedding_set
        )?);
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Const,
            "NUMBER_ONE",
            &embedding_set
        )?);
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Static,
            "STR_TWO",
            &embedding_set
        )?);
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Struct,
            "TestStruct",
            &embedding_set
        )?);
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Module,
            "inner_test_mod",
            &embedding_set
        )?);
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "func_with_params",
            &embedding_set
        )?);
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "main",
            &embedding_set
        )?);
        // items not in changed file, expect to be remain embedded
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "simple_four",
            &embedding_set,
        )?);
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Struct,
            "OtherStruct",
            &embedding_set,
        )?);

        // -- simulating sending response from app back to index --
        // At the end of `scan_for_change`, an `AppEvent` is sent, which is processed inside the
        // app event loop (not running here), which should print a message and then send another
        // message to index the unembedded items in the database, which should currently only be
        // the items detected as having changed through `scan_for_change`.

        let is_embedding_info_before = db_handle.is_embedding_info_all(&embedding_set)?;
        use tracing::Level;
        is_embedding_info_before.tracing_print_all(Level::DEBUG);
        db_handle.debug_print_counts_active();
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
                    trace!(target: TUI_SCAN_TARGET, "IndexStatus Running");
                }
                IndexingStatus {
                    status: IndexStatus::Completed,
                    ..
                } => {
                    trace!(target: TUI_SCAN_TARGET, "IndexStatus Completed, breaking loop");
                    break;
                }
                _ => {}
            }
        }
        let is_embedding_info_after = db_handle.is_embedding_info_all(&embedding_set)?;
        is_embedding_info_after.tracing_print_all(Level::DEBUG);

        // print database output after reindex following the second scan
        let query_result = db_handle.raw_query(&script)?;
        let printable_headers = query_result.headers.join(", ");
        let printable_rows = query_result
            .rows
            .iter()
            .map(|r| r.iter().join(", "))
            .join("\n");
        debug!("rows from db:\n{printable_headers}\n{printable_rows}");

        // (possibly old info) items in changed file, expect to have embeddings again after scan

        // item not itself changed, only in changed file
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Const,
            "NUMBER_ONE",
            &embedding_set,
        )?);
        // item not itself changed, only in changed file
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Static,
            "STR_TWO",
            &embedding_set,
        )?);
        // item has changed,
        // and in changed file
        // before:  pub struct TestStruct(pub i32);
        // after:   struct TestStruct(pub i32);
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Struct,
            "TestStruct",
            &embedding_set,
        )?);
        // item not itself changed, only in changed file
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Module,
            "double_inner_mod",
            &embedding_set,
        )?);
        // item not itself changed, only in changed file
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Module,
            "inner_test_mod",
            &embedding_set,
        )?);
        // item not itself changed, only in changed file
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "func_with_params",
            &embedding_set,
        )?);
        // item not itself changed, only in changed file
        assert!(is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "main",
            &embedding_set
        )?);
        // neither file nor item changed
        // - expect to remain embedded
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Function,
            "simple_four",
            &embedding_set,
        )?);
        // neither file nor item changed
        // - expect to remain embedded
        assert!(!is_name_embed_null(
            &db_handle,
            NodeType::Struct,
            "OtherStruct",
            &embedding_set,
        )?);

        trace!(target: TUI_SCAN_TARGET, "changing back:\n{}", target_file.display());
        let contents = std::fs::read_to_string(&target_file)?;
        trace!(target: TUI_SCAN_TARGET, "reading file:\n{}", &contents);
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
        trace!(target: TUI_SCAN_TARGET, "writing changed file:\n{}", &changed);
        std::fs::write(&target_file, changed)?;
        Ok(())
    }
}
