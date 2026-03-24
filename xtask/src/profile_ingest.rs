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
use tracing::field::{Field, Visit};
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
    pub loops: usize,
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
    let mut loops: usize = 1;

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
            "--loops" => {
                let Some(v) = it.next() else {
                    return Err(XtaskError::new("Missing value for --loops"));
                };
                loops = v
                    .parse()
                    .map_err(|_| XtaskError::new("--loops must be a positive integer"))?;
                if loops == 0 {
                    return Err(XtaskError::new("--loops must be at least 1"));
                }
            }
            "--json" => json_stdout = true,
            other => {
                return Err(XtaskError::new(format!(
                    "Unknown flag '{other}'. Usage: cargo xtask profile-ingest --target <path> [--stages parse,transform,embed] [--verbosity 1|2|3] [--loops N] [--json]"
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
        loops,
    })
}

#[derive(Clone, Copy)]
struct SpanStart(Instant);

#[derive(Clone, Copy)]
struct AllocatedId(u64);

#[derive(Clone, Copy)]
struct ParentAlloc(Option<u64>);

#[derive(Clone)]
struct SpanFields(String);

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
        attrs: &tracing::span::Attributes<'_>,
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
        let mut visitor = SpanFieldVisitor::default();
        attrs.record(&mut visitor);
        if !visitor.fields.is_empty() {
            ext.insert(SpanFields(visitor.fields.join(", ")));
        }
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
        let name = if let Some(fields) = span.extensions().get::<SpanFields>() {
            format!("{} [{}]", span.metadata().name(), fields.0)
        } else {
            span.metadata().name().to_string()
        };
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

#[derive(Default)]
struct SpanFieldVisitor {
    fields: Vec<String>,
}

impl Visit for SpanFieldVisitor {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .push(format!("{}={}", field.name(), value.replace(',', ";")));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields.push(format!("{}={value:?}", field.name()));
    }
}

#[derive(Debug, Clone)]
struct TimedNode {
    name: String,
    elapsed: Duration,
    children: Vec<TimedNode>,
}

#[derive(Debug, Serialize)]
struct JsonNode {
    name: String,
    elapsed_ms: u128,
    pct: f64,
    children: Vec<JsonNode>,
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

/// Time spent in `node` that is not accounted for by its direct children.
fn self_time(node: &TimedNode) -> Duration {
    let children_sum: Duration = node.children.iter().map(|c| c.elapsed).sum();
    node.elapsed.saturating_sub(children_sum)
}

/// Collapse same-named siblings under each parent into a single aggregated node.
///
/// Siblings that share a name are merged: their `elapsed` times are summed and
/// their children lists are concatenated (then recursively merged). A `(×N)`
/// suffix is appended to the name so the multiplicity remains visible.
fn merge_siblings(nodes: Vec<TimedNode>) -> Vec<TimedNode> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<TimedNode>> = HashMap::new();
    for node in nodes {
        if !groups.contains_key(&node.name) {
            order.push(node.name.clone());
        }
        groups.entry(node.name.clone()).or_default().push(node);
    }
    order
        .into_iter()
        .map(|name| {
            let group = groups.remove(&name).unwrap();
            let count = group.len();
            if count == 1 {
                let mut node = group.into_iter().next().unwrap();
                node.children = merge_siblings(node.children);
                node
            } else {
                let elapsed: Duration = group.iter().map(|n| n.elapsed).sum();
                let all_children: Vec<TimedNode> =
                    group.into_iter().flat_map(|n| n.children).collect();
                TimedNode {
                    name: format!("{name} (\u{d7}{count})"),
                    elapsed,
                    children: merge_siblings(all_children),
                }
            }
        })
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

const MIN_NAME_COL: usize = 36;

fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// Compute the widest label (prefix + connector + name) in the tree,
/// respecting the same depth limit used for display.
fn max_tree_label_width(
    node: &TimedNode,
    depth: usize,
    depth_remaining: Option<usize>,
) -> usize {
    let indent = if depth == 0 { 0 } else { 3 * depth };
    let width = indent + node.name.len();

    if depth_remaining == Some(0) {
        return width;
    }
    let next_depth = depth_remaining.map(|d| d.saturating_sub(1));
    node.children
        .iter()
        .map(|ch| max_tree_label_width(ch, depth + 1, next_depth))
        .fold(width, usize::max)
}

fn print_node(
    node: &TimedNode,
    global: Duration,
    prefix: &str,
    connector: &str,
    is_stage: bool,
    depth_remaining: Option<usize>,
    name_col: usize,
) {
    let name = if is_stage {
        capitalize_first(&node.name)
    } else {
        node.name.clone()
    };
    let label = format!("{prefix}{connector}{name}");
    // Show exclusive (self) time in parentheses only when the node has children;
    // a leaf's self-time is trivially equal to its total, so the column is blank.
    let self_part = if !node.children.is_empty() {
        format!("{:>11}", format!("({}ms)", self_time(node).as_millis()))
    } else {
        "           ".to_string() // 11 spaces – same width as "(123456ms)"
    };
    println!(
        "{:<width$} {:>8}ms{} {:>6.1}%",
        label,
        node.elapsed.as_millis(),
        self_part,
        pct(node.elapsed, global),
        width = name_col,
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
        print_node(ch, global, &child_prefix, conn, false, next_depth, name_col);
    }
}

fn print_report(
    forest: &[TimedNode],
    global: Duration,
    verbosity: u8,
    stages_sum: Duration,
    stopwatch: &HashMap<&str, Duration>,
) {
    let max_depth = match verbosity {
        1 => Some(0),
        2 => Some(1),
        _ => None,
    };

    let name_col = forest
        .iter()
        .map(|t| max_tree_label_width(t, 0, max_depth))
        .max()
        .unwrap_or(0)
        .max(MIN_NAME_COL)
        + 2; // gap between name column and numbers
    // " 99999999ms (123456ms) 9999.9%" = 1+8+2+11+1+6+1 = 30
    let display_width = name_col + 30;

    println!("{}", "═".repeat(display_width));
    for tree in forest {
        print_node(tree, global, "", "", true, max_depth, name_col);
    }

    if !stopwatch.is_empty() {
        let stage_map = find_stage_nodes(forest);
        let header = "─── Stopwatch check ";
        println!("{header}{}", "─".repeat(display_width.saturating_sub(header.len())));
        for &key in &["parse", "transform", "embed"] {
            let Some(&sw) = stopwatch.get(key) else {
                continue;
            };
            let span_ms = stage_map
                .get(key)
                .map(|n| n.elapsed.as_millis())
                .unwrap_or(0) as i128;
            let wall_ms = sw.as_millis() as i128;
            let drift = wall_ms - span_ms;
            let sign = if drift >= 0 { "+" } else { "" };
            println!(
                "  {:<12} span: {:>7}ms  wall: {:>7}ms  Δ {}{}ms",
                capitalize_first(key),
                span_ms,
                wall_ms,
                sign,
                drift,
            );
        }
    }

    let overhead = global.saturating_sub(stages_sum);
    println!("{}", "─".repeat(display_width));
    // Footer rows: no self-time column – pad with 11 spaces to preserve alignment.
    println!(
        "{:<width$} {:>8}ms            {:>6.1}%",
        "Stages total",
        stages_sum.as_millis(),
        pct(stages_sum, global),
        width = name_col,
    );
    println!(
        "{:<width$} {:>8}ms            {:>6.1}%",
        "Overhead",
        overhead.as_millis(),
        pct(overhead, global),
        width = name_col,
    );
    println!(
        "{:<width$} {:>8}ms",
        "Wall time",
        global.as_millis(),
        width = name_col,
    );
    println!("{}", "═".repeat(display_width));
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

/// Statistics for a single stage across all iterations
#[derive(Debug, Clone)]
struct StageStats {
    min: Duration,
    max: Duration,
    avg: Duration,
    p50: Duration,
    p90: Duration,
    p95: Duration,
    p99: Duration,
    std_dev: Duration,
}

fn compute_stats(durations: &[Duration]) -> Option<StageStats> {
    if durations.is_empty() {
        return None;
    }
    
    let mut sorted: Vec<Duration> = durations.to_vec();
    sorted.sort();
    
    let min = *sorted.first().unwrap();
    let max = *sorted.last().unwrap();
    
    let sum: Duration = durations.iter().sum();
    let avg = sum / durations.len() as u32;
    
    let percentile = |p: f64| -> Duration {
        let idx = ((durations.len() as f64 - 1.0) * p / 100.0) as usize;
        sorted[idx.min(sorted.len() - 1)]
    };
    
    let p50 = percentile(50.0);
    let p90 = percentile(90.0);
    let p95 = percentile(95.0);
    let p99 = percentile(99.0);
    
    // Standard deviation
    let avg_secs = avg.as_secs_f64();
    let variance: f64 = durations
        .iter()
        .map(|d| {
            let diff = d.as_secs_f64() - avg_secs;
            diff * diff
        })
        .sum::<f64>() / durations.len() as f64;
    let std_dev_secs = variance.sqrt();
    let std_dev = Duration::from_secs_f64(std_dev_secs);
    
    Some(StageStats {
        min,
        max,
        avg,
        p50,
        p90,
        p95,
        p99,
        std_dev,
    })
}

fn print_stats_report(
    stage_stats: &HashMap<&'static str, StageStats>,
    global_stats: &StageStats,
    iterations: usize,
    target_name: &str,
    rs_count: usize,
    member_count: usize,
) {
    let display_width = 90;
    
    println!("{}", "═".repeat(display_width));
    println!("  Statistics Report: {} iterations", iterations);
    println!("  Target: {} ({} .rs files, {} {})", 
        target_name, 
        rs_count, 
        member_count,
        if member_count == 1 { "member" } else { "members" }
    );
    println!("{}", "═".repeat(display_width));
    
    // Global timing stats
    println!("\n  Global Wall Time (ms):");
    println!("    min: {:>8}  avg: {:>8}  max: {:>8}  std: {:>8}",
        global_stats.min.as_millis(),
        global_stats.avg.as_millis(),
        global_stats.max.as_millis(),
        global_stats.std_dev.as_millis(),
    );
    println!("    p50: {:>8}  p90: {:>8}  p95: {:>8}  p99: {:>8}",
        global_stats.p50.as_millis(),
        global_stats.p90.as_millis(),
        global_stats.p95.as_millis(),
        global_stats.p99.as_millis(),
    );
    
    // Per-stage stats
    for &stage_name in &["parse", "transform", "embed"] {
        let Some(stats) = stage_stats.get(stage_name) else {
            continue;
        };
        
        println!("\n  {} Stage (ms):", capitalize_first(stage_name));
        println!("    min: {:>8}  avg: {:>8}  max: {:>8}  std: {:>8}",
            stats.min.as_millis(),
            stats.avg.as_millis(),
            stats.max.as_millis(),
            stats.std_dev.as_millis(),
        );
        println!("    p50: {:>8}  p90: {:>8}  p95: {:>8}  p99: {:>8}",
            stats.p50.as_millis(),
            stats.p90.as_millis(),
            stats.p95.as_millis(),
            stats.p99.as_millis(),
        );
    }
    
    println!("{}", "═".repeat(display_width));
}

fn format_stage_list(stages: &[Stage]) -> String {
    stages
        .iter()
        .map(|stage| match stage {
            Stage::Parse => "parse",
            Stage::Transform => "transform",
            Stage::Embed => "embed",
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Runs a single profiling iteration. Returns the global elapsed time and stage stopwatch times.
fn run_single_iteration<'a>(
    resolved: &'a ResolvedProfileTarget,
    cfg: &'a ProfileConfig,
    _timing_layer: &'a TimingCollector,
) -> Result<(Duration, HashMap<&'a str, Duration>), XtaskError> {
    let run_parse = cfg.stages.contains(&Stage::Parse);
    let run_transform = cfg.stages.contains(&Stage::Transform);
    let run_embed = cfg.stages.contains(&Stage::Embed);

    let mut parsed_workspace: Option<ParsedWorkspace> = None;
    let mut parsed_crate: Option<ParserOutput> = None;
    let mut db: Option<Arc<Database>> = None;
    let mut stopwatch: HashMap<&str, Duration> = HashMap::new();

    let global_start = Instant::now();

    if run_parse {
        let sw = Instant::now();
        {
            let _span = info_span!("parse").entered();
            match resolved {
                ResolvedProfileTarget::Workspace { workspace_root, .. } => {
                    let target_rel = cfg.target.display().to_string();
                    let _span = info_span!("parse_workspace", workspace = %target_rel).entered();
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
        stopwatch.insert("parse", sw.elapsed());
    }

    if run_transform {
        let sw = Instant::now();
        {
            let _span = info_span!("transform").entered();
            let d = {
                let _db_span = info_span!("init_db").entered();
                Arc::new(
                    Database::init_with_schema()
                        .map_err(|e| XtaskError::new(format!("init db: {e}")))?,
                )
            };
            match resolved {
                ResolvedProfileTarget::Workspace { .. } => {
                    let pw = parsed_workspace.take().ok_or_else(|| {
                        XtaskError::new(
                            "internal: missing parsed workspace for transform".to_string(),
                        )
                    })?;
                    transform_parsed_workspace(&d, pw)
                        .map_err(|e| XtaskError::new(format!("transform_parsed_workspace: {e}")))?;
                }
                ResolvedProfileTarget::Crate { .. } => {
                    let mut po = parsed_crate.take().ok_or_else(|| {
                        XtaskError::new(
                            "internal: missing parsed crate for transform".to_string(),
                        )
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
        stopwatch.insert("transform", sw.elapsed());
    }

    if run_embed {
        let sw = Instant::now();
        {
            let _span = info_span!("embed").entered();
            let d = db.as_ref().ok_or_else(|| {
                XtaskError::new("embed stage requires transform stage".to_string())
            })?;
            let or_cfg = OpenRouterConfig {
                model: OPENROUTER_MODEL.to_string(),
                dimensions: Some(OPENROUTER_DIMS as usize),
                request_dimensions: None,
                ..Default::default()
            };
            let backend_init = OpenRouterBackend::new(&or_cfg)
                .map_err(|e| XtaskError::new(format!("OpenRouter backend: {e}")))?;
            let processor_init =
                EmbeddingProcessor::new(EmbeddingSource::OpenRouter(backend_init));
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
        stopwatch.insert("embed", sw.elapsed());
    }

    let global_elapsed = global_start.elapsed();
    Ok((global_elapsed, stopwatch))
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

    let target_arg = cfg.target.display();
    let stages_arg = format_stage_list(&cfg.stages);
    eprintln!(
        "Repro: cargo run --package xtask -- profile-ingest --target {target_arg} --stages {stages_arg} --verbosity {} --loops {} --json",
        cfg.verbosity, cfg.loops
    );

    // Initialize tracing subscriber once
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

    // Run iterations and collect timing data
    let mut iteration_results: Vec<(Duration, HashMap<&str, Duration>)> = Vec::new();
    
    for i in 0..cfg.loops {
        if cfg.loops > 1 {
            eprintln!("Running iteration {}/{}...", i + 1, cfg.loops);
        }
        
        let result = run_single_iteration(&resolved, &cfg, &timing_layer)?;
        iteration_results.push(result);
        
        // Clear timing data between iterations, but keep the last one for detailed reporting
        if i < cfg.loops - 1 {
            let _ = timing_layer.take_finished();
        }
    }
    
    // Get the timing data from the last iteration for detailed breakdown
    let last_iteration_timing_data = timing_layer.take_finished();

    // Extract stage timings from all iterations for statistics
    let mut global_times: Vec<Duration> = Vec::new();
    let mut parse_times: Vec<Duration> = Vec::new();
    let mut transform_times: Vec<Duration> = Vec::new();
    let mut embed_times: Vec<Duration> = Vec::new();

    for (global, stopwatch) in &iteration_results {
        global_times.push(*global);
        if let Some(&t) = stopwatch.get("parse") {
            parse_times.push(t);
        }
        if let Some(&t) = stopwatch.get("transform") {
            transform_times.push(t);
        }
        if let Some(&t) = stopwatch.get("embed") {
            embed_times.push(t);
        }
    }

    // Compute statistics
    let global_stats = compute_stats(&global_times).unwrap();
    
    let mut stage_stats: HashMap<&str, StageStats> = HashMap::new();
    if !parse_times.is_empty() {
        stage_stats.insert("parse", compute_stats(&parse_times).unwrap());
    }
    if !transform_times.is_empty() {
        stage_stats.insert("transform", compute_stats(&transform_times).unwrap());
    }
    if !embed_times.is_empty() {
        stage_stats.insert("embed", compute_stats(&embed_times).unwrap());
    }

    // Build the tree from the last iteration's timing data for detailed reporting
    let timing_forest = if !last_iteration_timing_data.is_empty() {
        build_tree(last_iteration_timing_data)
    } else {
        Vec::new()
    };
    
    // Merge same-named siblings for cleaner terminal display
    let display_forest = merge_siblings(timing_forest.clone());
    
    // For single iteration, show the detailed report
    if cfg.loops == 1 {
        let (global_elapsed, ref stopwatch) = iteration_results[0];
        
        // Calculate stages sum for overhead calculation
        let stages_sum: Duration = stopwatch.values().copied().sum();
        
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
        
        // If we have detailed timing data and verbosity is enabled, show the tree breakdown
        if !display_forest.is_empty() && cfg.verbosity > 0 {
            print_report(&display_forest, global_elapsed, cfg.verbosity, stages_sum, stopwatch);
        } else {
            // Simple summary output for single iteration
            let display_width = 70;
            println!("{}", "═".repeat(display_width));
            println!("  Stage Timings:");
            for &stage in &["parse", "transform", "embed"] {
                if let Some(&t) = stopwatch.get(stage) {
                    println!("    {:12} {:>8}ms", capitalize_first(stage), t.as_millis());
                }
            }
            println!("{}", "─".repeat(display_width));
            println!("    {:12} {:>8}ms", "Total", global_elapsed.as_millis());
            println!("{}", "═".repeat(display_width));
        }
    } else {
        // Multiple iterations: show statistics report
        print_stats_report(
            &stage_stats,
            &global_stats,
            cfg.loops,
            &target_name,
            rs_count,
            member_count,
        );
        
        // Also show detailed breakdown from last iteration if verbosity is high
        if !display_forest.is_empty() && cfg.verbosity > 0 {
            let (last_global, last_stopwatch) = &iteration_results[iteration_results.len() - 1];
            let stages_sum: Duration = last_stopwatch.values().copied().sum();
            println!("\n  Detailed breakdown (last iteration):");
            print_report(&display_forest, *last_global, cfg.verbosity, stages_sum, last_stopwatch);
        }
    }

    // Save JSON report for last iteration (or aggregated stats if multiple iterations)
    let out_dir = root.join("xtask/profiling_output");
    fs::create_dir_all(&out_dir)
        .map_err(|e| XtaskError::new(format!("create profiling_output dir: {e}")))?;
    let ts_slug = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let git_slug = target_git.as_deref().unwrap_or("nogit");
    let loops_suffix = if cfg.loops > 1 {
        format!("_x{}", cfg.loops)
    } else {
        String::new()
    };
    let fname = format!("{}_{}_{}{}.json", target_name, git_slug, ts_slug, loops_suffix);
    let out_path = out_dir.join(fname);
    
    // Create a report with statistics
    let mut stages_map = serde_json::Map::new();
    for (name, stats) in &stage_stats {
        let mut stage_obj = serde_json::Map::new();
        stage_obj.insert("min_ms".to_string(), (stats.min.as_millis() as u64).into());
        stage_obj.insert("avg_ms".to_string(), (stats.avg.as_millis() as u64).into());
        stage_obj.insert("max_ms".to_string(), (stats.max.as_millis() as u64).into());
        stage_obj.insert("p50_ms".to_string(), (stats.p50.as_millis() as u64).into());
        stage_obj.insert("p90_ms".to_string(), (stats.p90.as_millis() as u64).into());
        stage_obj.insert("p95_ms".to_string(), (stats.p95.as_millis() as u64).into());
        stage_obj.insert("p99_ms".to_string(), (stats.p99.as_millis() as u64).into());
        stage_obj.insert("std_dev_ms".to_string(), (stats.std_dev.as_millis() as u64).into());
        stages_map.insert(name.to_string(), serde_json::Value::Object(stage_obj));
    }
    
    let raw_times: Vec<serde_json::Value> = iteration_results.iter().map(|(global, stopwatch)| {
        let mut stages = serde_json::Map::new();
        for (k, v) in stopwatch {
            stages.insert(k.to_string(), (v.as_millis() as u64).into());
        }
        serde_json::json!({
            "global_ms": global.as_millis(),
            "stages": stages,
        })
    }).collect();
    
    // Get last iteration's global time for the timing tree percentages
    let (last_global, _) = &iteration_results[iteration_results.len() - 1];
    
    // Generate full timing tree for offline analysis (unmerged to preserve full detail)
    let full_forest: Vec<JsonNode> = timing_forest
        .iter()
        .map(|t| to_json_node(t, *last_global))
        .collect();
    
    // Generate stage tree (top-level stages with children)
    let stage_forest: Vec<JsonNode> = timing_forest
        .iter()
        .filter(|t| matches!(t.name.as_str(), "parse" | "transform" | "embed"))
        .map(|t| to_json_node(t, *last_global))
        .collect();
    
    let report = serde_json::json!({
        "target": target_name,
        "target_path": target_path.display().to_string(),
        "target_git_sha": target_git,
        "ploke_git_sha": ploke_git,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "rs_file_count": rs_count,
        "member_count": member_count,
        "iterations": cfg.loops,
        "statistics": {
            "global": {
                "min_ms": global_stats.min.as_millis() as u64,
                "avg_ms": global_stats.avg.as_millis() as u64,
                "max_ms": global_stats.max.as_millis() as u64,
                "p50_ms": global_stats.p50.as_millis() as u64,
                "p90_ms": global_stats.p90.as_millis() as u64,
                "p95_ms": global_stats.p95.as_millis() as u64,
                "p99_ms": global_stats.p99.as_millis() as u64,
                "std_dev_ms": global_stats.std_dev.as_millis() as u64,
            },
            "stages": stages_map,
        },
        "raw_times": raw_times,
        "timing_tree": {
            "stages": stage_forest,
            "full_forest": full_forest,
        },
    });
    
    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| XtaskError::new(format!("serialize report: {e}")))?;
    fs::write(&out_path, &json)
        .map_err(|e| XtaskError::new(format!("write {}: {e}", out_path.display())))?;

    println!("Wrote {}", out_path.display());

    if cfg.json_stdout {
        println!("{}", json);
    }

    Ok(())
}
