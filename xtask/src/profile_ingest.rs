//! Cold-start ingest profiling (`cargo xtask profile-ingest`).

use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use ploke_core::embeddings::{
    EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
};
use ploke_db::{Database, create_index_primary};
use ploke_embed::{
    cancel_token::CancellationToken,
    config::OpenRouterConfig,
    indexer::{EmbeddingProcessor, EmbeddingSource, IndexerTask},
    providers::openrouter::OpenRouterBackend,
    runtime::EmbeddingRuntime,
};
use ploke_io::IoManagerHandle;
use ploke_transform::transform::{transform_parsed_graph, transform_parsed_workspace};
use serde::Serialize;
use syn_parser::{
    ParsedWorkspace, ParserOutput,
    discovery::workspace::{locate_workspace_manifest, try_parse_manifest},
    parse_workspace, try_run_phases_and_merge,
};
use tokio::sync::{broadcast, mpsc};
use tracing::info_span;
use tracing_subscriber::{
    EnvFilter, Layer, layer::SubscriberExt, prelude::*, registry::LookupSpan,
};
use walkdir::WalkDir;

use crate::{XtaskError, workspace_root};

const OPENROUTER_MODEL: &str = "mistralai/codestral-embed-2505";
const OPENROUTER_DIMS: u32 = 1536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Parse,
    Transform,
    Embed,
}

#[derive(Debug, Clone)]
pub struct ProfileConfig {
    pub target: PathBuf,
    pub stages: Vec<Stage>,
    pub verbosity: u8,
    pub json_stdout: bool,
}

fn parse_stages(s: &str) -> Result<Vec<Stage>, XtaskError> {
    let mut out = Vec::new();
    for part in s.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        match p {
            "parse" => out.push(Stage::Parse),
            "transform" => out.push(Stage::Transform),
            "embed" => out.push(Stage::Embed),
            other => {
                return Err(XtaskError::new(format!(
                    "Unknown stage '{other}'. Use parse, transform, embed (comma-separated)."
                )));
            }
        }
    }
    Ok(out)
}

pub fn parse_profile_ingest_args(args: Vec<String>) -> Result<ProfileConfig, XtaskError> {
    let mut target: Option<PathBuf> = None;
    let mut stages_arg: Option<String> = None;
    let mut verbosity: u8 = 2;
    let mut json_stdout = false;

    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--target" => {
                let Some(v) = it.next() else {
                    return Err(XtaskError::new("Missing value for --target"));
                };
                target = Some(PathBuf::from(v));
            }
            "--stages" => {
                let Some(v) = it.next() else {
                    return Err(XtaskError::new("Missing value for --stages"));
                };
                stages_arg = Some(v);
            }
            "--verbosity" => {
                let Some(v) = it.next() else {
                    return Err(XtaskError::new("Missing value for --verbosity"));
                };
                verbosity = v
                    .parse()
                    .map_err(|_| XtaskError::new("--verbosity must be 1, 2, or 3"))?;
                if !(1..=3).contains(&verbosity) {
                    return Err(XtaskError::new("--verbosity must be 1, 2, or 3"));
                }
            }
            "--json" => json_stdout = true,
            other => {
                return Err(XtaskError::new(format!(
                    "Unknown flag '{other}'. Usage: cargo xtask profile-ingest --target <path> [--stages parse,transform,embed] [--verbosity 1|2|3] [--json]"
                )));
            }
        }
    }

    let Some(target) = target else {
        return Err(XtaskError::new(
            "Missing --target <path>. Example: tests/fixture_crates/fixture_nodes",
        ));
    };

    let key_ok = env::var("OPENROUTER_API_KEY")
        .ok()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    let mut stages = if let Some(ref s) = stages_arg {
        parse_stages(s)?
    } else {
        vec![Stage::Parse, Stage::Transform]
    };

    if stages_arg.is_none() && key_ok && !stages.contains(&Stage::Embed) {
        stages.push(Stage::Embed);
    }

    if stages.contains(&Stage::Embed) && !stages.contains(&Stage::Transform) {
        return Err(XtaskError::new(
            "Embed stage requires transform. Use --stages parse,transform,embed",
        ));
    }
    if stages.contains(&Stage::Transform) && !stages.contains(&Stage::Parse) {
        stages.insert(0, Stage::Parse);
    }

    if stages.contains(&Stage::Embed) && !key_ok {
        return Err(XtaskError::new(
            "Embed stage requested but OPENROUTER_API_KEY is not set (or empty).",
        ));
    }

    Ok(ProfileConfig {
        target,
        stages,
        verbosity,
        json_stdout,
    })
}

#[derive(Clone, Copy)]
struct SpanStart(Instant);

#[derive(Clone, Copy)]
struct AllocatedId(u64);

#[derive(Clone, Copy)]
struct ParentAlloc(Option<u64>);

static NEXT_SPAN_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
struct FinishedRecord {
    id: u64,
    parent_id: Option<u64>,
    name: String,
    elapsed: Duration,
}

#[derive(Clone)]
struct TimingCollector {
    finished: Arc<Mutex<Vec<FinishedRecord>>>,
}

impl TimingCollector {
    fn new() -> Self {
        Self {
            finished: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn take_finished(&self) -> Vec<FinishedRecord> {
        self.finished
            .lock()
            .map(|mut g| std::mem::take(&mut *g))
            .unwrap_or_default()
    }
}

impl<S> Layer<S> for TimingCollector
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        _attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        let my_id = NEXT_SPAN_ID.fetch_add(1, Ordering::Relaxed);
        let parent_id = span
            .parent()
            .and_then(|p| p.extensions().get::<AllocatedId>().map(|a| a.0));
        let mut ext = span.extensions_mut();
        ext.insert(SpanStart(Instant::now()));
        ext.insert(AllocatedId(my_id));
        ext.insert(ParentAlloc(parent_id));
    }

    fn on_close(&self, id: tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let Some(span) = ctx.span(&id) else {
            return;
        };
        let start = span.extensions().get::<SpanStart>().map(|s| s.0);
        let my_id = span.extensions().get::<AllocatedId>().map(|a| a.0);
        let parent_id = span.extensions().get::<ParentAlloc>().and_then(|p| p.0);
        let name = span.metadata().name().to_string();
        if let (Some(start), Some(my_id)) = (start, my_id) {
            let elapsed = start.elapsed();
            if let Ok(mut g) = self.finished.lock() {
                g.push(FinishedRecord {
                    id: my_id,
                    parent_id,
                    name,
                    elapsed,
                });
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct JsonNode {
    name: String,
    elapsed_ms: u128,
    pct: f64,
    children: Vec<JsonNode>,
}

#[derive(Debug)]
struct TimedNode {
    name: String,
    elapsed: Duration,
    children: Vec<TimedNode>,
}

fn build_tree(flat: Vec<FinishedRecord>) -> Vec<TimedNode> {
    if flat.is_empty() {
        return Vec::new();
    }
    let mut by_id: HashMap<u64, (String, Duration, Option<u64>)> = HashMap::new();
    for r in &flat {
        by_id.insert(r.id, (r.name.clone(), r.elapsed, r.parent_id));
    }
    let mut children_map: HashMap<u64, Vec<u64>> = HashMap::new();
    let mut roots: Vec<u64> = Vec::new();
    for r in &flat {
        if let Some(pid) = r.parent_id {
            if by_id.contains_key(&pid) {
                children_map.entry(pid).or_default().push(r.id);
            } else {
                roots.push(r.id);
            }
        } else {
            roots.push(r.id);
        }
    }
    fn build(
        id: u64,
        by_id: &HashMap<u64, (String, Duration, Option<u64>)>,
        cmap: &HashMap<u64, Vec<u64>>,
    ) -> TimedNode {
        let (name, elapsed, _) = by_id
            .get(&id)
            .cloned()
            .unwrap_or_else(|| ("?".to_string(), Duration::ZERO, None));
        let child_ids = cmap.get(&id).cloned().unwrap_or_default();
        let children = child_ids
            .into_iter()
            .map(|cid| build(cid, by_id, cmap))
            .collect();
        TimedNode {
            name,
            elapsed,
            children,
        }
    }
    roots.sort_unstable();
    roots.dedup();
    roots
        .into_iter()
        .map(|id| build(id, &by_id, &children_map))
        .collect()
}

fn collect_stage_nodes<'a>(node: &'a TimedNode, out: &mut HashMap<&'static str, &'a TimedNode>) {
    match node.name.as_str() {
        "parse" => {
            out.entry("parse").or_insert(node);
        }
        "transform" => {
            out.entry("transform").or_insert(node);
        }
        "embed" => {
            out.entry("embed").or_insert(node);
        }
        _ => {}
    }
    for ch in &node.children {
        collect_stage_nodes(ch, out);
    }
}

fn find_stage_nodes(forest: &[TimedNode]) -> HashMap<&'static str, &TimedNode> {
    let mut m = HashMap::new();
    for tree in forest {
        collect_stage_nodes(tree, &mut m);
    }
    m
}

fn pct(elapsed: Duration, global: Duration) -> f64 {
    if global.is_zero() {
        return 0.0;
    }
    elapsed.as_secs_f64() / global.as_secs_f64() * 100.0
}

fn to_json_node(n: &TimedNode, global: Duration) -> JsonNode {
    JsonNode {
        name: n.name.clone(),
        elapsed_ms: n.elapsed.as_millis(),
        pct: pct(n.elapsed, global),
        children: n.children.iter().map(|c| to_json_node(c, global)).collect(),
    }
}

const DISPLAY_WIDTH: usize = 56;
const NAME_COL: usize = 36;

fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn print_node(
    node: &TimedNode,
    global: Duration,
    prefix: &str,
    connector: &str,
    is_stage: bool,
    depth_remaining: Option<usize>,
) {
    let name = if is_stage {
        capitalize_first(&node.name)
    } else {
        node.name.clone()
    };
    let label = format!("{prefix}{connector}{name}");
    println!(
        "{:<width$} {:>8}ms {:>6.1}%",
        label,
        node.elapsed.as_millis(),
        pct(node.elapsed, global),
        width = NAME_COL,
    );

    if depth_remaining == Some(0) {
        return;
    }
    let next_depth = depth_remaining.map(|d| d.saturating_sub(1));
    let child_prefix = if connector.is_empty() {
        prefix.to_string()
    } else if connector.starts_with('└') {
        format!("{prefix}   ")
    } else {
        format!("{prefix}│  ")
    };
    let count = node.children.len();
    for (i, ch) in node.children.iter().enumerate() {
        let conn = if i == count - 1 { "└─ " } else { "├─ " };
        print_node(ch, global, &child_prefix, conn, false, next_depth);
    }
}

fn print_report(forest: &[TimedNode], global: Duration, verbosity: u8, stages_sum: Duration) {
    let max_depth = match verbosity {
        1 => Some(0),
        2 => Some(1),
        _ => None,
    };

    println!("{}", "═".repeat(DISPLAY_WIDTH));
    for tree in forest {
        print_node(tree, global, "", "", true, max_depth);
    }

    let overhead = global.saturating_sub(stages_sum);
    println!("{}", "─".repeat(DISPLAY_WIDTH));
    println!(
        "{:<width$} {:>8}ms {:>6.1}%",
        "Stages total",
        stages_sum.as_millis(),
        pct(stages_sum, global),
        width = NAME_COL,
    );
    println!(
        "{:<width$} {:>8}ms {:>6.1}%",
        "Overhead",
        overhead.as_millis(),
        pct(overhead, global),
        width = NAME_COL,
    );
    println!(
        "{:<width$} {:>8}ms",
        "Wall time",
        global.as_millis(),
        width = NAME_COL,
    );
    println!("{}", "═".repeat(DISPLAY_WIDTH));
}

#[derive(Debug, Clone)]
enum ResolvedProfileTarget {
    Workspace {
        workspace_root: PathBuf,
        member_count: usize,
    },
    Crate {
        crate_root: PathBuf,
    },
}

fn resolve_profile_target(requested: &Path) -> Result<ResolvedProfileTarget, XtaskError> {
    let local_manifest = requested.join("Cargo.toml");
    if local_manifest.is_file() {
        let manifest = try_parse_manifest(requested).map_err(|e| {
            XtaskError::new(format!(
                "Failed to read manifest at {}: {e}",
                local_manifest.display()
            ))
        })?;
        if let Some(ws) = manifest.workspace {
            return Ok(ResolvedProfileTarget::Workspace {
                member_count: ws.members.len(),
                workspace_root: ws.path.clone(),
            });
        }
        return Ok(ResolvedProfileTarget::Crate {
            crate_root: requested.to_path_buf(),
        });
    }
    match locate_workspace_manifest(requested) {
        Ok((_p, metadata)) => {
            let ws = metadata.workspace.ok_or_else(|| {
                XtaskError::new("Workspace manifest missing [workspace] section".to_string())
            })?;
            Ok(ResolvedProfileTarget::Workspace {
                member_count: ws.members.len(),
                workspace_root: ws.path.clone(),
            })
        }
        Err(syn_parser::discovery::DiscoveryError::WorkspaceManifestNotFound { .. }) => {
            Err(XtaskError::new(format!(
                "No Cargo.toml or workspace at {}",
                requested.display()
            )))
        }
        Err(e) => Err(XtaskError::new(format!("Workspace discovery failed: {e}"))),
    }
}

fn count_rs_files(root: &Path) -> usize {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|x| x == "rs"))
        .count()
}

fn git_head_short(dir: &Path) -> Option<String> {
    let dir_s = dir.to_str()?;
    let out = std::process::Command::new("git")
        .args(["-C", dir_s, "rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[derive(Serialize)]
struct ProfileReport {
    target: String,
    target_path: String,
    target_git_sha: Option<String>,
    ploke_git_sha: Option<String>,
    timestamp: String,
    rs_file_count: usize,
    member_count: usize,
    global_elapsed_ms: u128,
    stages_sum_ms: u128,
    unaccounted_ms: u128,
    stages: Vec<JsonNode>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    full_forest: Vec<JsonNode>,
}

pub fn run_profile_ingest(cfg: ProfileConfig) -> Result<(), XtaskError> {
    let root = workspace_root();
    let target_path = if cfg.target.is_absolute() {
        cfg.target.clone()
    } else {
        root.join(&cfg.target)
    };
    let target_path = fs::canonicalize(&target_path).map_err(|e| {
        XtaskError::new(format!(
            "Could not resolve target path {}: {e}",
            target_path.display()
        ))
    })?;

    let resolved = resolve_profile_target(&target_path)?;
    let member_count = match &resolved {
        ResolvedProfileTarget::Workspace { member_count, .. } => *member_count,
        ResolvedProfileTarget::Crate { .. } => 1,
    };

    let rs_count = count_rs_files(&target_path);
    let target_name = target_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("target")
        .to_string();
    let target_git = git_head_short(&target_path);
    let ploke_git = git_head_short(&root);

    let timing_layer = TimingCollector::new();

    let registry = tracing_subscriber::registry::Registry::default().with(timing_layer.clone());

    if env::var("PLOKE_PROFILE_LOG").is_ok() {
        let fmt = tracing_subscriber::fmt::layer()
            .with_target(false)
            .with_filter(EnvFilter::from_default_env());
        registry.with(fmt).init();
    } else {
        registry.init();
    }

    let global_start = Instant::now();

    let run_parse = cfg.stages.contains(&Stage::Parse);
    let run_transform = cfg.stages.contains(&Stage::Transform);
    let run_embed = cfg.stages.contains(&Stage::Embed);

    let mut parsed_workspace: Option<ParsedWorkspace> = None;
    let mut parsed_crate: Option<ParserOutput> = None;
    let mut db: Option<Arc<Database>> = None;

    if run_parse {
        let _span = info_span!("parse").entered();
        match &resolved {
            ResolvedProfileTarget::Workspace { workspace_root, .. } => {
                let pw = parse_workspace(workspace_root, None)
                    .map_err(|e| XtaskError::new(format!("parse_workspace: {e}")))?;
                parsed_workspace = Some(pw);
            }
            ResolvedProfileTarget::Crate { crate_root } => {
                let po = try_run_phases_and_merge(crate_root)
                    .map_err(|e| XtaskError::new(format!("try_run_phases_and_merge: {e}")))?;
                parsed_crate = Some(po);
            }
        }
    }

    if run_transform {
        let _span = info_span!("transform").entered();
        let d = Arc::new(
            Database::init_with_schema().map_err(|e| XtaskError::new(format!("init db: {e}")))?,
        );
        match &resolved {
            ResolvedProfileTarget::Workspace { .. } => {
                let pw = parsed_workspace.take().ok_or_else(|| {
                    XtaskError::new("internal: missing parsed workspace for transform".to_string())
                })?;
                transform_parsed_workspace(&d, pw)
                    .map_err(|e| XtaskError::new(format!("transform_parsed_workspace: {e}")))?;
            }
            ResolvedProfileTarget::Crate { .. } => {
                let mut po = parsed_crate.take().ok_or_else(|| {
                    XtaskError::new("internal: missing parsed crate for transform".to_string())
                })?;
                let merged = po.extract_merged_graph().ok_or_else(|| {
                    XtaskError::new("missing merged graph after parse".to_string())
                })?;
                let tree = po.extract_module_tree().ok_or_else(|| {
                    XtaskError::new("missing module tree after parse".to_string())
                })?;
                transform_parsed_graph(&d, merged, &tree)
                    .map_err(|e| XtaskError::new(format!("transform_parsed_graph: {e}")))?;
            }
        }
        db = Some(d);
    }

    if run_embed {
        let _span = info_span!("embed").entered();
        let d = db
            .as_ref()
            .ok_or_else(|| XtaskError::new("embed stage requires transform stage".to_string()))?;
        let or_cfg = OpenRouterConfig {
            model: OPENROUTER_MODEL.to_string(),
            dimensions: Some(OPENROUTER_DIMS as usize),
            request_dimensions: None,
            ..Default::default()
        };
        let backend_init = OpenRouterBackend::new(&or_cfg)
            .map_err(|e| XtaskError::new(format!("OpenRouter backend: {e}")))?;
        let processor_init = EmbeddingProcessor::new(EmbeddingSource::OpenRouter(backend_init));
        let backend_active = OpenRouterBackend::new(&or_cfg)
            .map_err(|e| XtaskError::new(format!("OpenRouter backend (activate): {e}")))?;
        let proc_arc = Arc::new(EmbeddingProcessor::new(EmbeddingSource::OpenRouter(
            backend_active,
        )));
        let embedding_set = EmbeddingSet::new(
            EmbeddingProviderSlug::new_from_str("openrouter"),
            EmbeddingModelId::new_from_str(OPENROUTER_MODEL),
            EmbeddingShape::new_dims_default(OPENROUTER_DIMS),
        );
        let runtime = Arc::new(EmbeddingRuntime::from_shared_set(
            Arc::clone(&d.active_embedding_set),
            processor_init,
        ));
        runtime
            .activate(d.as_ref(), embedding_set, Arc::clone(&proc_arc))
            .map_err(|e| XtaskError::new(format!("activate embedding runtime: {e}")))?;

        create_index_primary(d.as_ref())
            .map_err(|e| XtaskError::new(format!("create_index_primary: {e}")))?;

        let (cancellation_token, cancel_handle) = CancellationToken::new();
        let indexer = IndexerTask::new(
            Arc::clone(d),
            IoManagerHandle::new(),
            runtime,
            cancellation_token,
            cancel_handle,
            None,
        );
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| XtaskError::new(format!("tokio runtime: {e}")))?;

        rt.block_on(async move {
            let (progress_tx, _progress_rx) = broadcast::channel(32);
            let (_control_tx, control_rx) = mpsc::channel(4);
            let task = Arc::new(indexer);
            task.run(Arc::new(progress_tx), control_rx)
                .await
                .map_err(|e| XtaskError::new(format!("indexer run: {e}")))
        })?;
    }

    let global_elapsed = global_start.elapsed();
    let flat = timing_layer.take_finished();
    let forest = build_tree(flat);

    let stage_map = find_stage_nodes(&forest);
    let stages_sum = stage_map
        .get("parse")
        .map(|n| n.elapsed)
        .unwrap_or_default()
        + stage_map
            .get("transform")
            .map(|n| n.elapsed)
            .unwrap_or_default()
        + stage_map
            .get("embed")
            .map(|n| n.elapsed)
            .unwrap_or_default();

    let stage_json: Vec<JsonNode> = ["parse", "transform", "embed"]
        .into_iter()
        .filter_map(|k| stage_map.get(k).map(|n| to_json_node(n, global_elapsed)))
        .collect();

    // Full span tree always persisted for offline analysis; verbosity only affects stdout.
    let full_forest: Vec<JsonNode> = forest
        .iter()
        .map(|t| to_json_node(t, global_elapsed))
        .collect();

    let report = ProfileReport {
        target: target_name.clone(),
        target_path: target_path.display().to_string(),
        target_git_sha: target_git.clone(),
        ploke_git_sha: ploke_git.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        rs_file_count: rs_count,
        member_count,
        global_elapsed_ms: global_elapsed.as_millis(),
        stages_sum_ms: stages_sum.as_millis(),
        unaccounted_ms: global_elapsed.saturating_sub(stages_sum).as_millis(),
        stages: stage_json,
        full_forest,
    };

    let out_dir = root.join("xtask/profiling_output");
    fs::create_dir_all(&out_dir)
        .map_err(|e| XtaskError::new(format!("create profiling_output dir: {e}")))?;
    let ts_slug = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let git_slug = target_git.as_deref().unwrap_or("nogit");
    let fname = format!("{}_{}_{}.json", target_name, git_slug, ts_slug);
    let out_path = out_dir.join(fname);
    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| XtaskError::new(format!("serialize report: {e}")))?;
    fs::write(&out_path, &json)
        .map_err(|e| XtaskError::new(format!("write {}: {e}", out_path.display())))?;

    let member_label = if member_count == 1 { "member" } else { "members" };
    println!(
        "Target: {} @ {} ({} .rs files, {} {})",
        target_name,
        target_git.as_deref().unwrap_or("n/a"),
        rs_count,
        member_count,
        member_label,
    );
    println!("Ploke:  {}", ploke_git.as_deref().unwrap_or("n/a"));
    println!("Wrote {}", out_path.display());

    print_report(&forest, global_elapsed, cfg.verbosity, stages_sum);

    if cfg.json_stdout {
        println!("{}", json);
    }

    Ok(())
}
