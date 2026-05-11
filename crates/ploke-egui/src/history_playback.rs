use std::path::{Path, PathBuf};

use ploke_records::history::ActorRefRecord;
use ploke_records::playback::FineStep;
use serde::Serialize;

use crate::cli::InspectOutputFormat;
use crate::spec::PrepareError;

const SCHEMA_VERSION: &str = "prototype1-history-playback.v1";

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CoarseHistoryPlaybackProjection {
    pub(crate) schema_version: &'static str,
    pub(crate) campaign_id: String,
    pub(crate) manifest_path: PathBuf,
    pub(crate) run_root: PathBuf,
    pub(crate) step_count: usize,
    pub(crate) warning_count: usize,
    pub(crate) spine: CoarseHistorySpine,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct FineHistoryPlaybackProjection {
    pub(crate) schema_version: &'static str,
    pub(crate) campaign_id: String,
    pub(crate) manifest_path: PathBuf,
    pub(crate) run_root: PathBuf,
    pub(crate) step_count: usize,
    pub(crate) steps: Vec<FineStep>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "granularity")]
pub(crate) enum HistoryPlaybackProjection {
    Coarse(CoarseHistoryPlaybackProjection),
    Fine(FineHistoryPlaybackProjection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlaybackGranularity {
    Coarse,
    Fine,
}

pub(crate) fn build_coarse(
    campaign_id: &str,
    manifest_path: &Path,
) -> Result<CoarseHistoryPlaybackProjection, PrepareError> {
    let run_root = prototype_root(manifest_path);
    let store = FsRunStore::new(run_root.clone());
    let blocks = store
        .load_history_blocks()
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "load sealed history blocks",
            detail: source.to_string(),
        })?;
    let spine = build_coarse_history_spine(&blocks);
    Ok(CoarseHistoryPlaybackProjection {
        schema_version: SCHEMA_VERSION,
        campaign_id: campaign_id.to_owned(),
        manifest_path: manifest_path.to_path_buf(),
        run_root,
        step_count: spine.steps.len(),
        warning_count: spine.warnings.len(),
        spine,
    })
}

pub(crate) fn build_fine(
    campaign_id: &str,
    manifest_path: &Path,
) -> Result<FineHistoryPlaybackProjection, PrepareError> {
    let run_root = prototype_root(manifest_path);
    let store = FsRunStore::new(run_root.clone());
    let blocks = store
        .load_history_blocks()
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "load sealed history blocks",
            detail: source.to_string(),
        })?;
    let playback = fine_run_playback_from_sealed_history(&blocks);
    let steps = playback.into_iter().collect::<Vec<_>>();
    Ok(FineHistoryPlaybackProjection {
        schema_version: SCHEMA_VERSION,
        campaign_id: campaign_id.to_owned(),
        manifest_path: manifest_path.to_path_buf(),
        run_root,
        step_count: steps.len(),
        steps,
    })
}

pub(crate) fn build(
    campaign_id: &str,
    manifest_path: &Path,
    granularity: PlaybackGranularity,
) -> Result<HistoryPlaybackProjection, PrepareError> {
    match granularity {
        PlaybackGranularity::Coarse => {
            build_coarse(campaign_id, manifest_path).map(HistoryPlaybackProjection::Coarse)
        }
        PlaybackGranularity::Fine => {
            build_fine(campaign_id, manifest_path).map(HistoryPlaybackProjection::Fine)
        }
    }
}

pub(crate) fn run(
    campaign_id: &str,
    manifest_path: &Path,
    format: InspectOutputFormat,
    granularity: PlaybackGranularity,
) -> Result<(), PrepareError> {
    let projection = build(campaign_id, manifest_path, granularity)?;
    match format {
        InspectOutputFormat::Table => match &projection {
            HistoryPlaybackProjection::Coarse(projection) => {
                print!("{}", render_coarse_table(projection))
            }
            HistoryPlaybackProjection::Fine(projection) => {
                print!("{}", render_fine_table(projection))
            }
        },
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&projection).map_err(PrepareError::Serialize)?
            );
        }
    }
    Ok(())
}

fn render_coarse_table(projection: &CoarseHistoryPlaybackProjection) -> String {
    let mut out = String::new();
    out.push_str("history playback (coarse)\n");
    out.push_str(&format!("campaign: {}\n", projection.campaign_id));
    out.push_str(&format!("run_root: {}\n", projection.run_root.display()));
    out.push_str(&format!(
        "steps: {}  warnings: {}\n",
        projection.step_count, projection.warning_count
    ));

    if projection.spine.steps.is_empty() {
        out.push_str("\n(no sealed history blocks)\n");
        return out;
    }

    out.push_str(
        "\nidx  block_height  block_hash         successor_runtime      selected_candidate  considered\n",
    );
    for (idx, step) in projection.spine.steps.iter().enumerate() {
        let candidate = step.selected_candidate.as_deref().unwrap_or("-");
        let successor_runtime = actor_label(&step.selected_successor.runtime);
        out.push_str(&format!(
            "{idx:>3}  {height:>12}  {hash:<18}  {successor:<21}  {candidate:<18}  {considered:>9}\n",
            height = step.block_height,
            hash = abbreviate_hash(&step.block_hash),
            successor = successor_runtime,
            considered = step.considered_candidate_count,
        ));
    }

    if !projection.spine.warnings.is_empty() {
        out.push_str("\nwarnings:\n");
        for warning in &projection.spine.warnings {
            out.push_str(&format!("- {warning:?}\n"));
        }
    }

    out
}

fn render_fine_table(projection: &FineHistoryPlaybackProjection) -> String {
    let mut out = String::new();
    out.push_str("history playback (fine)\n");
    out.push_str(&format!("campaign: {}\n", projection.campaign_id));
    out.push_str(&format!("run_root: {}\n", projection.run_root.display()));
    out.push_str(&format!("steps: {}\n", projection.step_count));

    if projection.steps.is_empty() {
        out.push_str("\n(no fine history steps)\n");
        return out;
    }

    out.push_str(
        "\nidx  block_height  phase  kind                       evidence          label\n",
    );
    for (idx, step) in projection.steps.iter().enumerate() {
        out.push_str(&format!(
            "{idx:>3}  {height:>12}  {phase:>5}  {kind:<26}  {evidence:<16}  {label}\n",
            height = step.order.block_height,
            phase = step.order.phase_rank,
            kind = format!("{:?}", step.kind),
            evidence = format!("{:?}", step.evidence),
            label = step.label.as_deref().unwrap_or("-"),
        ));
    }

    out
}

fn abbreviate_hash(hash: &str) -> &str {
    if hash.len() <= 16 { hash } else { &hash[..16] }
}

fn actor_label(actor: &ActorRefRecord) -> String {
    match actor {
        ActorRefRecord::Runtime(runtime) => format!("runtime:{runtime}"),
        ActorRefRecord::Human(value) => format!("human:{value}"),
        ActorRefRecord::Process(value) => format!("process:{value}"),
        ActorRefRecord::External(value) => format!("external:{value}"),
        ActorRefRecord::Unknown { reason } => format!("unknown:{reason}"),
    }
}

fn prototype_root(manifest_path: &Path) -> PathBuf {
    manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_render_handles_empty_spine() {
        let projection = CoarseHistoryPlaybackProjection {
            schema_version: SCHEMA_VERSION,
            campaign_id: "campaign-a".to_owned(),
            manifest_path: PathBuf::from("/tmp/campaign.json"),
            run_root: PathBuf::from("/tmp/prototype1"),
            step_count: 0,
            warning_count: 0,
            spine: CoarseHistorySpine {
                steps: Vec::new(),
                warnings: Vec::new(),
            },
        };

        let rendered = render_coarse_table(&projection);
        assert!(rendered.contains("history playback (coarse)"));
        assert!(rendered.contains("(no sealed history blocks)"));
    }

    #[test]
    fn table_render_handles_empty_fine_playback() {
        let projection = FineHistoryPlaybackProjection {
            schema_version: SCHEMA_VERSION,
            campaign_id: "campaign-a".to_owned(),
            manifest_path: PathBuf::from("/tmp/campaign.json"),
            run_root: PathBuf::from("/tmp/prototype1"),
            step_count: 0,
            steps: Vec::<FineStep>::new(),
        };

        let rendered = render_fine_table(&projection);
        assert!(rendered.contains("history playback (fine)"));
        assert!(rendered.contains("(no fine history steps)"));
    }
}
