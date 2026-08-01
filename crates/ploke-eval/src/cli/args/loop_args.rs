use std::path::PathBuf;

use ploke_records::ids::CampaignId;

use clap::{ArgAction, Parser, Subcommand};
use ploke_llm::request::models::ModelRouteSource;
use serde::{Deserialize, Serialize};

use super::common::{InspectOutputFormat, parse_embedding_route, parse_model_route_source};
use crate::campaign::EmbeddingRoute;
use crate::cli::prototype1_state::walk::{phase::WalkPhase, protocol::OperationId};
pub use crate::walk_client::WalkEvidenceQuery;

#[derive(Debug, Parser)]
#[command(about = "Run higher-level loop wrappers over eval, protocol, and intervention stages")]
pub struct LoopCommand {
    #[command(subcommand)]
    pub command: LoopSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum LoopSubcommand {
    /// Run Prototype 1 through eval configuration, baseline arm, synthesis, treatment, and compare.
    Prototype1(Prototype1LoopCommand),
    /// Create a Prototype 1 campaign and admit the current checkout as Parent(0).
    Prototype1Setup(Prototype1SetupCommand),
    /// Diagnose the active Prototype 1 parent checkout and print the next exact commands.
    Prototype1Doctor(Prototype1DoctorCommand),
    /// Print the broad-harness prompt for the active Prototype 1 parent checkout.
    Prototype1Prompt(Prototype1PromptCommand),
    /// Resume the active Prototype 1 parent checkout until the current turn completes or hands off.
    Prototype1Continue(Prototype1AdvanceCommand),
    /// Advance exactly one diagnosed Prototype 1 parent phase.
    Prototype1Step(Prototype1AdvanceCommand),
    /// Drive the typed Prototype 1 parent runtime path.
    Prototype1State(Prototype1StateAdvanceCommand),
    // ANCHOR: prototype1_walk_command_safety_help
    /// Operate the live Prototype 1 Step-mode controller through its local server.
    #[command(
        name = "walk",
        about = "Operate Prototype 1 typestate transitions through the local walk service",
        long_about = "Operate Prototype 1 typestate transitions through the local walk service.\n\nThe walk server owns mutation admission for the attached Step-mode controller session; CLI and UI commands are clients of that authority. It is not a durable Continuous-mode scheduler. By default the client uses the active walk context if one was set with `walk use`, otherwise the current directory, and a repo-hashed socket under the runtime directory.\n\nLive start/step requests are submitted as one supervised server job. A second live start/step request returns the active job instead of starting a duplicate attempt. Use `walk status` to query the server while a job is running, and `walk step --watch` to follow the accepted step job until it finishes.",
        after_help = "Common workflows:\n  Set context:       ploke-eval loop walk use /path/to/parent-worktree\n  Start live walk:   ploke-eval loop walk start\n  Inspect server:    ploke-eval loop walk status\n  Inspect progress:  ploke-eval loop walk summary -v\n  Replay history:    ploke-eval loop walk replay --index 0\n  Query eval DB:     ploke-eval loop walk db_query --view progress\n  Move replay:       ploke-eval loop walk forward --steps 10 --tail 20\n  Live step:         ploke-eval loop walk step --until r6 --allow-live-api\n\nSafety notes:\n  replay/back/forward and db_query are read-only inspection commands.\n  Named db_query views are owner-DB evidence projections, never live mutation authority.\n  step submits a typestate job; --watch only follows that job and never admits provider calls.\n  Provider-backed edges require --allow-live-api.\n  R12 -> R13b successor handoff mutates checkout state and requires --allow git-changes.\n  stop shuts down only an idle server. It never aborts an admitted effectful edge; wait for its receipt, then recover or abandon explicitly if needed.\n  branch-live writes only explicit provenance and requires --allow provenance-record."
    )]
    Prototype1StateWalk(Prototype1StateWalkCommand),
    // ANCHOR_END: prototype1_walk_command_safety_help
    /// Inspect or execute one staged Prototype 1 runner invocation.
    #[command(hide = true)]
    Prototype1Runner(Prototype1RunnerCommand),
    /// Replay published broad headless-TUI harness requests outside the full parent loop.
    #[command(hide = true)]
    Prototype1Harness(Prototype1HarnessCommand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1StateStopAfter {
    Materialize,
    Build,
    Spawn,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1SuccessorSelection {
    GenerationLocal,
    HistoryFrontierMax,
    HistoryScoreChildProp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1TraversalMetrics {
    Operational,
    OperationalAndProtocol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1CandidateGenerator {
    Legacy,
    BroadHarnessRequest,
    DeterministicTuiTools,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1EditSurface {
    PlokeTuiTools,
    WorkspaceExceptPlokeEval,
}

#[derive(Debug, Parser)]
#[command(about = "Run the typed Prototype 1 state transitions for the active parent checkout")]
pub struct Prototype1StateCommand {
    /// Campaign id. Defaults to parent identity, then active `select campaign`.
    #[arg(long)]
    pub campaign: Option<CampaignId>,

    /// Legacy inspection target. Live controller runs reject this field in
    /// favor of the admitted profile and durable child-plan authority.
    #[arg(long)]
    pub node_id: Option<String>,

    /// Parent checkout root. Defaults to the current directory.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    /// Bootstrap the active checkout by writing and committing parent identity.
    #[arg(long)]
    pub init_parent_identity: bool,

    /// Branch to create or switch to before writing initial parent identity.
    #[arg(long, value_name = "BRANCH", requires = "init_parent_identity")]
    pub identity_branch: Option<String>,

    /// Prepared instance id for the generation-0 parent identity.
    #[arg(long, value_name = "INSTANCE", requires = "init_parent_identity")]
    pub identity_instance: Option<String>,

    /// Successor handoff token written by the previous parent runtime.
    #[arg(long, value_name = "PATH")]
    pub handoff_invocation: Option<PathBuf>,

    /// Assert the admitted profile's debug cut. Omit to use profile authority.
    #[arg(long, value_enum)]
    pub stop_after: Option<Prototype1StateStopAfter>,

    /// Successor-selection strategy. Active selection defaults to History traversal with current-generation candidates appended before scoring.
    #[arg(long, value_enum)]
    pub successor_selection: Option<Prototype1SuccessorSelection>,

    /// Replay seed committed by History-backed traversal selection.
    #[arg(long)]
    pub successor_selection_seed: Option<u64>,

    /// Metric-bearing states used by History-backed traversal scoring.
    #[arg(long, value_enum)]
    pub successor_selection_metrics: Option<Prototype1TraversalMetrics>,

    /// Candidate generator used before publishing the child plan.
    #[arg(long, value_enum)]
    pub candidate_generator: Option<Prototype1CandidateGenerator>,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

/// CLI admission envelope around the unchanged typed Prototype 1 command.
#[derive(Debug, Parser)]
pub struct Prototype1StateAdvanceCommand {
    #[command(flatten)]
    pub state: Prototype1StateCommand,

    #[command(flatten)]
    pub capabilities: Prototype1MutationCapabilities,
}

#[derive(Debug, Parser)]
#[command(about = "Operate Prototype 1 typestate transitions through the local walk service")]
pub struct Prototype1StateWalkCommand {
    #[command(subcommand)]
    pub command: Prototype1StateWalkSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum Prototype1StateWalkSubcommand {
    /// Run the local walk server on a Unix socket.
    Serve(Prototype1StateWalkServeCommand),
    /// Save the active parent checkout for later walk commands.
    Use(Prototype1StateWalkUseCommand),
    /// Attach to the setup-derived controller session, defaulting to R3.
    Start(Prototype1StateWalkStartCommand),
    /// Submit one live typestate job by one step or until a target phase.
    Step(Prototype1StateWalkStepCommand),
    /// Reset the server projection without changing durable session history or stopping the server.
    Reset(Prototype1StateWalkResetCommand),
    /// Inspect or explicitly resolve one durable controller recovery cause.
    Recover(Prototype1StateWalkRecoverCommand),
    /// Print tracked output files for the current walk.
    Files(Prototype1StateWalkControlCommand),
    /// Show the admitted campaign, run profile, and effective controller configuration.
    Config(Prototype1StateWalkControlCommand),
    /// List or inspect canonical completed evaluation traces.
    Trace(Prototype1StateWalkTraceCommand),
    /// Show the reconstructed controller view or the last successful Step delta.
    Show(Prototype1StateWalkShowCommand),
    /// Show the ordered durable controller-session journal projection.
    SessionHistory(Prototype1StateWalkControlCommand),
    /// Audit file/database persistence surfaces for a walk transition.
    Audit(Prototype1StateWalkAuditCommand),
    /// Run an immutable CozoScript query against the active loop run eval DB.
    #[command(name = "db_query", visible_alias = "db-query")]
    DbQuery(Prototype1StateWalkDbQueryCommand),
    /// Inspect nested LLM/tool-loop debugger checkpoints.
    Llm(Prototype1StateWalkLlmCommand),
    /// Summarize durable campaign progress without contacting the walk server.
    Summary(Prototype1StateWalkSummaryCommand),
    /// Show or jump within the read-only historical replay cursor.
    Replay(Prototype1StateWalkReplayCommand),
    /// Move the read-only historical replay cursor backward without undoing side effects.
    Back(Prototype1StateWalkReplayMoveCommand),
    /// Move the read-only historical replay cursor forward.
    Forward(Prototype1StateWalkReplayMoveCommand),
    /// Record explicit provenance before branching from replay toward live work.
    BranchLive(Prototype1StateWalkBranchLiveCommand),
    /// Query server liveness, phase, and active or most recent job.
    Status(Prototype1StateWalkControlCommand),
    /// Stop an idle server; active effectful jobs must reach a durable boundary first.
    Stop(Prototype1StateWalkControlCommand),
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkServeCommand {
    /// Parent checkout root. Defaults to active walk context, then current directory.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    /// Explicit Unix socket path. Defaults to a repo-hashed path under the runtime directory.
    #[arg(long, value_name = "PATH")]
    pub socket: Option<PathBuf>,

    /// Idle seconds before the server exits. Defaults to 1800 seconds.
    #[arg(long, value_name = "SECS", conflicts_with = "no_ttl")]
    pub ttl_secs: Option<u64>,

    /// Keep the server alive until an explicit stop request.
    #[arg(long)]
    pub no_ttl: bool,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Remember the parent checkout/socket used by later walk commands",
    after_help = "Examples:\n  ploke-eval loop walk use /path/to/parent-worktree\n  ploke-eval loop walk use /path/to/parent-worktree --socket /tmp/ploke-walk.sock\n\nLater walk commands default to this context when --repo-root/--socket are omitted."
)]
pub struct Prototype1StateWalkUseCommand {
    /// Parent checkout root to remember. Defaults to the current directory.
    #[arg(value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    /// Explicit Unix socket path to remember for this walk context.
    #[arg(long, value_name = "PATH")]
    pub socket: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkControlCommand {
    /// Parent checkout root. Defaults to active walk context, then current directory.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    /// Explicit Unix socket path. Defaults to a repo-hashed path under the runtime directory.
    #[arg(long, value_name = "PATH")]
    pub socket: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,

    /// Include protocol and transition-graph versions in table output.
    #[arg(long)]
    pub with_version: bool,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Inspect canonical evaluation records and protocol reviews",
    after_help = "Examples:\n  ploke-eval loop walk trace list\n  ploke-eval loop walk trace show --instance BurntSushi__ripgrep-2209 --run-id run-...\n\nCompleted traces are loaded through the global run registry and preserve record, model-response, and protocol-artifact source hashes. Non-completed runs return lifecycle data only; mutable turn traces are not read."
)]
pub struct Prototype1StateWalkTraceCommand {
    #[command(flatten)]
    pub control: Prototype1StateWalkControlCommand,

    #[command(subcommand)]
    pub command: Prototype1StateWalkTraceSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Prototype1StateWalkTraceSubcommand {
    /// List completed registered runs within the admitted campaign root.
    List,
    /// Load one exact registered run by stable campaign-local coordinate.
    Show(Prototype1StateWalkTraceShowCommand),
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkTraceShowCommand {
    /// Owning campaign from `trace list`; defaults to the admitted root campaign.
    #[arg(long)]
    pub campaign: Option<String>,

    /// Benchmark/task instance recorded by the run registration.
    #[arg(long)]
    pub instance: String,

    /// Globally registered concrete run identifier.
    #[arg(long)]
    pub run_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1StateWalkAuditScope {
    /// Audit R0 preconditions plus expected file/DB persistence for R0 -> R1.
    R0ToR1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1StateWalkAuditTransition {
    R0ToR1,
    R1ToR2a,
    R1ToR3,
    R2aToR3,
    R3ToR4a,
    R4aToR4b,
    R4aToR4c,
    R4bToR4c,
    R4cToR5,
    R5ToR6,
    R6ToR7,
    R7ToR8,
    R8ToR9,
    R9ToR10,
    R10ToR11a,
    R10ToR11,
    R11aToR12,
    R11ToR12,
    R12ToR13a,
    R12ToR13b,
    R12ToR13c,
    R13aToR14a,
    R13bToR14b,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Audit file/database persistence surfaces for a walk transition",
    after_help = "Examples:\n  ploke-eval loop walk audit --repo-root .\n  ploke-eval loop walk audit --repo-root . --transition r10-to-r11\n  ploke-eval loop walk audit --repo-root . --format json\n  ploke-eval loop walk audit --repo-root . --verbose\n  ploke-eval loop walk audit --repo-root . --with-note\n  ploke-eval loop walk audit --repo-root . --verify\n\nThe first audit scope is r0-to-r1: it checks R0 preconditions, the documents r0_to_r1 expects, and whether that transition is expected to write files or DB rows. By default this command skips durable reconstruction and surface re-hashing; use --verify to reconstruct and verify the latest typestate first. Use --transition to show only one transition's checklist items. Use --verbose for per-item paths, DB relations, counts, and legacy document/DB details. Use --with-note to include the checklist note column. This command is read-only and uses the local walk server like show/replay."
)]
pub struct Prototype1StateWalkAuditCommand {
    #[command(flatten)]
    pub control: Prototype1StateWalkControlCommand,

    /// Campaign id. Defaults to parent identity in the repo root.
    #[arg(long)]
    pub campaign: Option<CampaignId>,

    /// Audit scope to run.
    #[arg(long, value_enum, default_value_t = Prototype1StateWalkAuditScope::R0ToR1)]
    pub scope: Prototype1StateWalkAuditScope,

    /// Show only one transition's persistence checklist items.
    #[arg(long, value_enum)]
    pub transition: Option<Prototype1StateWalkAuditTransition>,

    /// Reconstruct and verify durable walk state before auditing. This may hash large checkout surfaces.
    #[arg(long)]
    pub verify: bool,

    /// Show paths, relations, counts, and per-item details in table output.
    #[arg(long)]
    pub verbose: bool,

    /// Include the note column in the checklist table.
    #[arg(long)]
    pub with_note: bool,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Run an immutable CozoScript query against the active loop run eval DB",
    after_help = "Examples:\n  ploke-eval loop walk db_query --view relations\n  ploke-eval loop walk db_query --view progress\n  ploke-eval loop walk db_query --view handoff-evidence --format json\n  ploke-eval loop walk db_query --script '?[campaign_id, dataset_sources] := *eval_campaign { campaign_id, dataset_sources }'\n\nNamed views are closed, owner-DB evidence projections. `relations` is the complete installed-relation inventory. `counts` is a curated core operator projection, including critical typed-trace relations, not a complete schema count. None reports controller liveness or mutation authority; use walk status for that. Raw --script remains an expert escape hatch.\n\nEvery query is read-only: the walk service reads one exact owner DB snapshot, reports its content revision, restores it in isolation, and executes the script with Cozo ScriptMutability::Immutable. The response samples server phase/session only after the owner-DB query completes; those observations are non-atomic."
)]
pub struct Prototype1StateWalkDbQueryCommand {
    /// Parent checkout root. Defaults to active walk context, then current directory.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    /// Campaign id. Defaults to parent identity in the repo root.
    #[arg(long)]
    pub campaign: Option<CampaignId>,

    /// Closed owner-DB evidence view. This never reports live controller authority.
    #[arg(
        long,
        value_enum,
        conflicts_with = "script",
        required_unless_present = "script"
    )]
    pub view: Option<WalkEvidenceQuery>,

    /// Expert CozoScript to execute immutably against the loop run owner eval DB.
    #[arg(
        long,
        value_name = "COZOSCRIPT",
        conflicts_with = "view",
        required_unless_present = "view"
    )]
    pub script: Option<String>,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Show the reconstructed controller view or the last successful Step delta",
    after_help = "Examples:\n  ploke-eval loop walk show\n  ploke-eval loop walk show --with-version\n  ploke-eval loop walk show delta --verbose\n\nUse summary/replay for durable historical inspection after a run has completed."
)]
pub struct Prototype1StateWalkShowCommand {
    #[command(flatten)]
    pub control: Prototype1StateWalkControlCommand,

    #[command(subcommand)]
    pub command: Option<Prototype1StateWalkShowSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Prototype1StateWalkShowSubcommand {
    /// Show only the last successful step delta.
    Delta(Prototype1StateWalkShowDeltaCommand),
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkShowDeltaCommand {
    /// Include changed axis values plus added/removed nested type structures.
    #[arg(long)]
    pub verbose: bool,

    /// Disable ANSI colors in table output.
    #[arg(long)]
    pub no_color: bool,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Inspect nested LLM/tool-loop debugger checkpoints",
    after_help = "Examples:\n  ploke-eval loop walk llm sessions\n  ploke-eval loop walk llm observe --session-id SESSION\n  ploke-eval loop walk llm --format json observe --session-id SESSION --step 2\n  ploke-eval loop walk llm lanes\n  ploke-eval loop walk llm focus node-...-r2\n  ploke-eval loop walk llm timeline\n  ploke-eval loop walk llm prompt\n  ploke-eval loop walk llm protocol\n  ploke-eval loop walk llm show --lane node-...-r2 --head\n  ploke-eval loop walk llm tool --step 11 --json\n  ploke-eval loop walk llm back --lane node-...-r2 --steps 3\n\n`sessions` and `observe` use the typed exact-session read model shared with ploke-walk-ui; malformed siblings remain visible instead of aborting the inventory. This surface is read-only unless step/finish says otherwise. Current sessions/observe/lane/cursor/timeline/show/prompt/protocol/tool commands do not call providers, execute tools, mutate the checkout, or advance the outer typestate walk."
)]
pub struct Prototype1StateWalkLlmCommand {
    #[command(flatten)]
    pub control: Prototype1StateWalkControlCommand,

    #[command(subcommand)]
    pub command: Prototype1StateWalkLlmSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Prototype1StateWalkLlmSubcommand {
    /// List every exact debugger session and any unreadable or invalid manifests.
    Sessions,
    /// Observe one exact debugger session and optional published response step.
    Observe(Prototype1StateWalkLlmObserveCommand),
    /// List known fanout lanes and their latest checkpoint heads.
    Lanes(Prototype1StateWalkLlmLanesCommand),
    /// Set the default lane for subsequent LLM checkpoint commands on this server.
    Focus(Prototype1StateWalkLlmFocusCommand),
    /// Show the latest or selected LLM/tool-loop checkpoint.
    Show(Prototype1StateWalkLlmShowCommand),
    /// Show a compact chronological summary of recorded LLM/tool-loop steps.
    Timeline(Prototype1StateWalkLlmTimelineCommand),
    /// Inspect persisted request messages sent to the LLM/tool-loop.
    Prompt(Prototype1StateWalkLlmPromptCommand),
    /// Inspect persisted protocol review artifacts for the selected LLM/tool-loop.
    Protocol(Prototype1StateWalkLlmProtocolCommand),
    /// Inspect the tool definition and arguments for a selected LLM tool call.
    Tool(Prototype1StateWalkLlmToolCommand),
    /// Execute one historical or live provider response step through current tools.
    Step(Prototype1StateWalkLlmStepCommand),
    /// Continue live provider response steps until terminal or max steps.
    Finish(Prototype1StateWalkLlmFinishCommand),
    /// Move the focused/read-only lane cursor backward.
    Back(Prototype1StateWalkLlmMoveCommand),
    /// Move the focused/read-only lane cursor forward.
    Forward(Prototype1StateWalkLlmMoveCommand),
    /// Jump the focused/read-only lane cursor to the latest checkpoint head.
    Head(Prototype1StateWalkLlmLaneCommand),
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkLlmObserveCommand {
    /// Exact tool-loop session id from `llm sessions`.
    #[arg(long)]
    pub session_id: String,

    /// Exact published provider-response checkpoint. Defaults to the resume head.
    #[arg(long)]
    pub step: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1StateWalkLlmStepSource {
    /// Replay one recorded checkpoint response through current tools.
    Historical,
    /// Continue from checkpoint request state with one live provider response.
    Live,
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkLlmLanesCommand {
    /// Include workspace and session ids for each lane.
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkLlmFocusCommand {
    /// Lane id to focus, usually the candidate workspace basename.
    pub lane: String,
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkLlmLaneCommand {
    /// Lane id. Defaults to the current focus, then the latest lane.
    #[arg(long)]
    pub lane: Option<String>,
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkLlmMoveCommand {
    #[command(flatten)]
    pub lane: Prototype1StateWalkLlmLaneCommand,

    /// Number of recorded response steps to move.
    #[arg(long, default_value_t = 1)]
    pub steps: usize,
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkLlmShowCommand {
    /// Specific tool-loop session id to inspect. Defaults to selected lane/latest session.
    #[arg(long)]
    pub session_id: Option<String>,

    /// Lane id, usually the candidate workspace basename. Defaults to current focus.
    #[arg(long)]
    pub lane: Option<String>,

    /// Inspect the selected lane's latest recorded head instead of its read-only cursor.
    #[arg(long)]
    pub head: bool,

    /// Specific response step to inspect. Defaults to the lane cursor or latest recorded step.
    #[arg(long)]
    pub step: Option<usize>,
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkLlmTimelineCommand {
    /// Specific tool-loop session id to summarize. Defaults to selected lane/latest session.
    #[arg(long)]
    pub session_id: Option<String>,

    /// Lane id, usually the candidate workspace basename. Defaults to current focus.
    #[arg(long)]
    pub lane: Option<String>,
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkLlmPromptCommand {
    /// Specific tool-loop session id to inspect. Defaults to selected lane/latest session.
    #[arg(long)]
    pub session_id: Option<String>,

    /// Lane id, usually the candidate workspace basename. Defaults to current focus.
    #[arg(long)]
    pub lane: Option<String>,

    /// Response step whose request messages should be inspected. Defaults to the initial step 0.
    #[arg(long)]
    pub step: Option<usize>,

    /// Filter to one message role: system, user, assistant, or tool.
    #[arg(long, value_parser = ["system", "user", "assistant", "tool"])]
    pub role: Option<String>,

    /// Zero-based request message index to show.
    #[arg(long)]
    pub message: Option<usize>,

    /// Show complete message content instead of a bounded preview.
    #[arg(long)]
    pub full: bool,

    /// Print persisted request messages as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkLlmProtocolCommand {
    /// Specific tool-loop session id to inspect. Defaults to selected lane/latest session.
    #[arg(long)]
    pub session_id: Option<String>,

    /// Lane id, usually the candidate workspace basename. Defaults to current focus.
    #[arg(long)]
    pub lane: Option<String>,

    /// Print protocol summary as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkLlmToolCommand {
    /// Specific tool-loop session id to inspect. Defaults to selected lane/latest session.
    #[arg(long)]
    pub session_id: Option<String>,

    /// Lane id, usually the candidate workspace basename. Defaults to current focus.
    #[arg(long)]
    pub lane: Option<String>,

    /// Inspect the selected lane's latest recorded head instead of its read-only cursor.
    #[arg(long)]
    pub head: bool,

    /// Specific response step to inspect. Defaults to the lane cursor or latest recorded step.
    #[arg(long)]
    pub step: Option<usize>,

    /// One-based tool call index within the selected step. Defaults to 1.
    #[arg(long)]
    pub call: Option<usize>,

    /// Tool name to inspect. If no call matches, shows the current definition without historical arguments.
    #[arg(long)]
    pub name: Option<String>,

    /// Print the raw JSON tool definition and historical argument payload.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkLlmStepCommand {
    /// Reuse a prior semantic operation identity to attach to the exact same request.
    #[arg(long, value_name = "UUID")]
    pub(crate) operation_id: Option<OperationId>,

    /// Lane id, usually the candidate workspace basename. Defaults to current focus.
    #[arg(long)]
    pub lane: Option<String>,

    /// Specific tool-loop session id to step from. Defaults to selected lane/latest session.
    #[arg(long)]
    pub session_id: Option<String>,

    /// Response step index. Historical mode replays this response; live mode continues after this step.
    #[arg(long)]
    pub step: Option<usize>,

    /// Step source: historical replays a recorded response; live calls the provider once.
    #[arg(long, value_enum, default_value_t = Prototype1StateWalkLlmStepSource::Historical)]
    pub source: Prototype1StateWalkLlmStepSource,

    /// Wait for the live provider response. Required when --source live.
    #[arg(long)]
    pub watch: bool,

    /// Admit workspace mutation by current TUI tools during the stepped response.
    #[arg(long = "allow", value_name = "CAPABILITY", value_parser = ["workspace-mutation"])]
    pub allow: Vec<String>,

    /// Override model id for live steps. Defaults to the checkpoint session model.
    #[arg(long, value_name = "MODEL")]
    pub model_id: Option<String>,

    /// Provider slug for the selected model. Requires --model-id.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Maximum attempts for the one-step headless runtime.
    #[arg(long, default_value_t = 1)]
    pub max_attempts: u32,

    /// Timeout seconds for the one-step headless runtime.
    #[arg(long, default_value_t = 300)]
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Parser)]
pub struct Prototype1StateWalkLlmFinishCommand {
    /// Reuse a prior semantic operation identity to attach to the exact same request.
    #[arg(long, value_name = "UUID")]
    pub(crate) operation_id: Option<OperationId>,

    /// Lane id, usually the candidate workspace basename. Defaults to current focus.
    #[arg(long)]
    pub lane: Option<String>,

    /// Specific tool-loop session id to continue. Defaults to selected lane/latest session.
    #[arg(long)]
    pub session_id: Option<String>,

    /// Response step to continue after. Defaults to lane cursor/head.
    #[arg(long)]
    pub step: Option<usize>,

    /// Wait for live provider responses. Required for finish.
    #[arg(long)]
    pub watch: bool,

    /// Admit workspace mutation by current TUI tools during live response steps.
    #[arg(long = "allow", value_name = "CAPABILITY", value_parser = ["workspace-mutation"])]
    pub allow: Vec<String>,

    /// Override model id for live steps. Defaults to the checkpoint session model.
    #[arg(long, value_name = "MODEL")]
    pub model_id: Option<String>,

    /// Provider slug for the selected model. Requires --model-id.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Maximum live response steps before stopping.
    #[arg(long, default_value_t = 8)]
    pub max_steps: usize,

    /// Maximum attempts for each one-step headless runtime.
    #[arg(long, default_value_t = 1)]
    pub max_attempts: u32,

    /// Timeout seconds for each one-step headless runtime.
    #[arg(long, default_value_t = 300)]
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Summarize durable campaign progress without contacting the walk server",
    after_help = "Field guide:\n  branch: selected branch disposition when the final report recorded one.\n  decision: candidate-local selection outcome; decision=Stop does not necessarily mean the campaign stopped.\n  handoff: whether the selected successor handoff was acknowledged, skipped, or absent.\n\nUse --verbose/-v for run-specific notes and next inspection commands."
)]
pub struct Prototype1StateWalkSummaryCommand {
    /// Parent checkout root. Defaults to active walk context, then current directory.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,

    /// Include field meanings and next typed inspection commands in table output.
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Show or jump within the read-only historical replay cursor",
    after_help = "Examples:\n  ploke-eval loop walk replay\n  ploke-eval loop walk replay --index 0\n  ploke-eval loop walk replay --index 120 --tail 20\n\nThe recent-entry window shows the latest journal entries by default. Use --tail N to show more entries. This command does not undo or replay durable side effects."
)]
pub struct Prototype1StateWalkReplayCommand {
    #[command(flatten)]
    pub control: Prototype1StateWalkControlCommand,

    /// Jump to this zero-based durable journal index before rendering.
    #[arg(long)]
    pub index: Option<usize>,

    /// Number of trailing journal entries to render.
    #[arg(long, default_value_t = 3)]
    pub tail: usize,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Move the read-only historical replay cursor backward or forward",
    after_help = "Examples:\n  ploke-eval loop walk forward\n  ploke-eval loop walk forward --steps 10 --tail 20\n  ploke-eval loop walk back --steps 1\n\nCursor movement is read-only. It changes only the server's in-memory replay cursor and never mutates the parent checkout or journal. Use --tail N to expand the recent-entry window."
)]
pub struct Prototype1StateWalkReplayMoveCommand {
    #[command(flatten)]
    pub control: Prototype1StateWalkControlCommand,

    /// Number of replay entries to move.
    #[arg(long, default_value_t = 1)]
    pub steps: usize,

    /// Number of trailing journal entries to render.
    #[arg(long, default_value_t = 3)]
    pub tail: usize,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Record explicit provenance before leaving replay toward live work",
    after_help = "Example:\n  ploke-eval loop walk branch-live --reason \"investigate cursor 42\" --allow provenance-record\n\nThis command records provenance only. It does not materialize a new live branch, run providers, or mutate checkout content."
)]
pub struct Prototype1StateWalkBranchLiveCommand {
    #[command(flatten)]
    pub control: Prototype1StateWalkControlCommand,

    /// Reuse a prior semantic operation identity to attach to the exact same request.
    #[arg(long, value_name = "UUID")]
    pub(crate) operation_id: Option<OperationId>,

    /// Operator reason for leaving read-only historical replay.
    #[arg(long)]
    pub reason: String,

    /// Explicitly admit writing a replay-to-live provenance record.
    #[arg(long = "allow", value_name = "CAPABILITY", value_parser = ["provenance-record"])]
    pub allow: Vec<String>,
}

// ANCHOR: prototype1_walk_step_live_edge_admission
#[derive(Debug, Clone, Parser)]
#[command(
    about = "Submit one live typestate step job to the walk server",
    after_help = "Examples:\n  ploke-eval loop walk step\n  ploke-eval loop walk step --until r6 --allow-live-api\n  ploke-eval loop walk step --until r8 --allow-live-api --watch\n  ploke-eval loop walk step --watch --allow git-changes\n\nUse replay/back/forward for read-only historical inspection. Use step only when you intend to drive live typestate edges. Without --watch the command returns after the server accepts the job; with --watch it follows status until the job finishes. The follow flag never admits provider calls; provider-backed edges require --allow-live-api. Checkout-mutating successor handoff requires --allow git-changes. A bounded --until target must post-dominate every possible graph path from the current phase; use one bare Step and inspect its typed receipt at a branch such as R12."
)]
pub struct Prototype1StateWalkStepCommand {
    /// Parent checkout root. Defaults to active walk context, then current directory.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    /// Explicit Unix socket path. Defaults to a repo-hashed path under the runtime directory.
    #[arg(long, value_name = "PATH")]
    pub socket: Option<PathBuf>,

    /// Advance to a graph post-dominator instead of taking exactly one Step.
    #[arg(long, value_enum)]
    pub until: Option<WalkPhase>,

    /// Follow the accepted server job until it reaches a terminal state.
    #[arg(long)]
    pub watch: bool,

    /// Admit typestate edges that call a configured live provider.
    #[arg(long)]
    pub allow_live_api: bool,

    /// Reuse a prior semantic operation identity to attach to the exact same request.
    #[arg(long, value_name = "UUID")]
    pub(crate) operation_id: Option<OperationId>,

    /// Admit typed edges that intentionally install the selected successor into
    /// the active checkout. Required for R12 -> R13b handoff.
    #[arg(long = "allow", value_name = "CAPABILITY", value_parser = ["git-changes"])]
    pub allow: Vec<String>,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,

    /// Include protocol and transition-graph versions in table output.
    #[arg(long)]
    pub with_version: bool,
}
// ANCHOR_END: prototype1_walk_step_live_edge_admission

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Attach to the setup-derived controller session, defaulting to R3",
    after_help = "Examples:\n  ploke-eval loop walk start\n  ploke-eval loop walk start --until r6 --allow-live-api\n  ploke-eval loop walk start --no-ttl\n\nStart creates or contacts the local walk server for the selected parent checkout, attaches to its completed setup authority, submits one start job, and returns the accepted job. Provider-backed edges require --allow-live-api. A bounded --until target must post-dominate every possible graph path from the attached phase. Use `walk status` to inspect an active or completed server job. Use summary/replay when you only need to inspect a completed historical run."
)]
pub struct Prototype1StateWalkStartCommand {
    /// Campaign id. Defaults to parent identity, then active `select campaign`.
    #[arg(long)]
    pub campaign: Option<CampaignId>,

    /// Parent checkout root. Defaults to active walk context, then current directory.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    /// Explicit Unix socket path. Defaults to a repo-hashed path under the runtime directory.
    #[arg(long, value_name = "PATH")]
    pub socket: Option<PathBuf>,

    /// Idle seconds before an auto-started server exits. Defaults to 1800 seconds.
    #[arg(long, value_name = "SECS", conflicts_with = "no_ttl")]
    pub ttl_secs: Option<u64>,

    /// Keep an auto-started server alive until an explicit stop request.
    #[arg(long)]
    pub no_ttl: bool,

    /// Stop after this admitted typestate phase. Defaults to R3.
    #[arg(
        long,
        value_enum,
        default_value_t = crate::walk_client::DEFAULT_START_PHASE
    )]
    pub until: WalkPhase,

    /// Admit typestate edges that call a configured live provider.
    #[arg(long)]
    pub allow_live_api: bool,

    /// Reuse a prior semantic operation identity to attach to the exact same request.
    #[arg(long, value_name = "UUID")]
    pub(crate) operation_id: Option<OperationId>,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,

    /// Include protocol and transition-graph versions in table output.
    #[arg(long)]
    pub with_version: bool,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Reset the server projection without changing durable session history or stopping the server"
)]
pub struct Prototype1StateWalkResetCommand {
    #[command(flatten)]
    pub control: Prototype1StateWalkControlCommand,

    /// Reuse a prior semantic operation identity to attach to the exact same request.
    #[arg(long, value_name = "UUID")]
    pub(crate) operation_id: Option<OperationId>,
}

#[derive(Debug, Clone, Parser)]
#[command(
    about = "Inspect or explicitly resolve one durable controller recovery cause",
    after_help = "Examples:\n  ploke-eval loop walk recover\n  ploke-eval loop walk recover --abandon-job 11111111-1111-4111-8111-111111111111\n  ploke-eval loop walk recover --abandon-owner\n  ploke-eval loop walk recover --abandon-session\n  ploke-eval loop walk recover --admit-epoch\n\nWith no resolution flag this command is read-only. Job abandonment preserves an indeterminate operation as evidence while releasing its mutation blocker; it does not claim that the operation had no effects. Owner abandonment is admitted only for an exact lost-owner cause. Session abandonment permanently terminalizes unresolved pending or indeterminate authority without making the session runnable again; preserve that run as evidence and start fresh. Epoch admission records the exact prior and current source/binary epochs."
)]
pub struct Prototype1StateWalkRecoverCommand {
    #[command(flatten)]
    pub control: Prototype1StateWalkControlCommand,

    /// Resolve an exact journal owner whose kernel lock and process incarnation are gone.
    #[arg(long, conflicts_with_all = ["abandon_job", "abandon_session", "admit_epoch"])]
    pub abandon_owner: bool,

    /// Permanently terminalize an unresolved session without restoring mutation authority.
    #[arg(long, conflicts_with_all = ["abandon_job", "abandon_owner", "admit_epoch"])]
    pub abandon_session: bool,

    /// Admit the exact current source/binary epoch over the journal's prior epoch.
    #[arg(long, conflicts_with_all = ["abandon_job", "abandon_owner", "abandon_session"])]
    pub admit_epoch: bool,

    /// Preserve one indeterminate operation as abandoned and release its mutation blocker.
    #[arg(
        long,
        value_name = "UUID",
        conflicts_with_all = ["abandon_owner", "abandon_session", "admit_epoch", "operation_id"]
    )]
    pub abandon_job: Option<OperationId>,

    /// Reuse a prior semantic operation identity for the same recovery resolution.
    #[arg(long, value_name = "UUID", conflicts_with = "abandon_job")]
    pub(crate) operation_id: Option<OperationId>,
}

#[derive(Debug, Clone, Parser)]
#[command(about = "Diagnose or control the active Prototype 1 parent checkout")]
pub struct Prototype1ControlCommand {
    /// Active parent checkout root. Defaults to the current directory.
    ///
    /// Use this to diagnose or control a parent checkout from another cwd. The
    /// path must contain `.ploke/prototype1/parent_identity.json`.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

/// Explicit mutation capabilities shared by the direct step/continue clients.
#[derive(Debug, Clone, Parser)]
pub struct Prototype1AdvanceCommand {
    #[command(flatten)]
    pub control: Prototype1ControlCommand,

    #[command(flatten)]
    pub capabilities: Prototype1MutationCapabilities,
}

/// Explicit effect capabilities shared by every direct mutation client.
#[derive(Debug, Clone, Parser)]
pub struct Prototype1MutationCapabilities {
    /// Admit typed edges that call a configured live provider.
    #[arg(long)]
    pub allow_live_api: bool,

    /// Admit typed edges that install a successor into the active checkout.
    #[arg(long = "allow", value_name = "CAPABILITY", value_parser = ["git-changes"])]
    pub allow: Vec<String>,
}

impl Prototype1MutationCapabilities {
    pub(crate) fn allow_git_changes(&self) -> bool {
        self.allow.iter().any(|value| value == "git-changes")
    }
}

#[derive(Debug, Clone, Parser)]
#[command(about = "Diagnose the active Prototype 1 parent checkout")]
pub struct Prototype1DoctorCommand {
    #[command(flatten)]
    pub control: Prototype1ControlCommand,

    /// Run a tiny live protocol JSON request using the admitted model/provider/reasoning policy.
    #[arg(long)]
    pub live_protocol_preflight: bool,

    /// Run the production embedding selection and a tiny live embedding request using the admitted model/provider preference.
    ///
    /// This may refresh the shared embedding-model registry cache, but it does
    /// not write campaign closure, run, batch, or typestate evidence.
    #[arg(long)]
    pub live_embedding_preflight: bool,

    /// Extra setup check: initialize the headless TUI sparse/BM25 runtime for this parent checkout without making model calls.
    #[arg(long)]
    pub headless_tui_setup_preflight: bool,
}

#[derive(Debug, Clone, Parser)]
#[command(about = "Print the broad-harness prompt for the active Prototype 1 parent checkout")]
pub struct Prototype1PromptCommand {
    /// Active parent checkout root. Defaults to the current directory.
    ///
    /// Use this to print the prompt for a parent checkout from another cwd. The
    /// path must contain `.ploke/prototype1/parent_identity.json`.
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,
}

#[derive(Debug, Parser)]
#[command(
    about = "Inspect one Prototype 1 runner invocation or execute one persisted child runtime"
)]
pub struct Prototype1RunnerCommand {
    #[arg(long)]
    pub campaign: Option<CampaignId>,

    #[arg(long)]
    pub node_id: Option<String>,

    #[arg(long, value_name = "PATH")]
    pub invocation: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    pub execute: bool,

    #[arg(long, action = ArgAction::Set, default_value_t = false)]
    pub stop_on_error: bool,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
#[command(about = "Run published Prototype 1 broad harness request slots")]
pub struct Prototype1HarnessCommand {
    #[command(subcommand)]
    pub command: Prototype1HarnessSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum Prototype1HarnessSubcommand {
    /// Run one published broad harness request slot.
    Attempt(Prototype1HarnessAttemptCommand),
    /// Run multiple distinct published broad harness request slots concurrently.
    Sweep(Prototype1HarnessSweepCommand),
}

#[derive(Debug, Parser)]
pub struct Prototype1HarnessAttemptCommand {
    /// Path to one published broad harness request JSON.
    #[arg(long, value_name = "PATH")]
    pub request: PathBuf,

    /// Model id for this attempt. Defaults to the headless TUI harness default.
    #[arg(long, value_name = "MODEL")]
    pub model_id: Option<String>,

    /// Provider slug to pin for the selected model.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Override the published attempt count for fast live probes.
    #[arg(long)]
    pub max_attempts: Option<u32>,

    /// Override the published turn timeout for fast live probes.
    #[arg(long)]
    pub timeout_secs: Option<u64>,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Parser)]
pub struct Prototype1HarnessSweepCommand {
    /// Published broad harness request JSON. Repeat for multiple lanes.
    #[arg(long = "request", value_name = "PATH", action = ArgAction::Append)]
    pub requests: Vec<PathBuf>,

    /// Directory containing published broad harness request JSON files.
    #[arg(long, value_name = "DIR")]
    pub requests_dir: Option<PathBuf>,

    /// Model id for lanes. Pass one model for all requests, or one per request.
    #[arg(long = "model-id", value_name = "MODEL", action = ArgAction::Append)]
    pub model_ids: Vec<String>,

    /// Provider slug to pin for every selected model.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Override the published attempt count for fast live probes.
    #[arg(long)]
    pub max_attempts: Option<u32>,

    /// Override the published turn timeout for fast live probes.
    #[arg(long)]
    pub timeout_secs: Option<u64>,

    /// Maximum concurrent lanes.
    #[arg(long, default_value_t = 2)]
    pub parallel: usize,

    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1LoopStopAfter {
    BaselineEval,
    BaselineProtocol,
    TargetSelection,
    InterventionApply,
    Compare,
}

#[derive(Debug, Parser)]
#[command(
    about = "Plan or admit a Prototype 1 Parent(0) setup",
    long_about = "Plan or admit a Prototype 1 Parent(0) setup.\n\nSetup requires an explicit run profile. The run profile owns search, generation, selection, execution, storage, and control; legacy prototype1 search and --stop-after flags are not setup authority. With an existing batch, that batch owns its cohort and eval budget, while --instance may select one member as the primary Parent(0) identity. Use --preview first, then --expect-plan-sha256 to bind admission to the reviewed plan."
)]
pub struct Prototype1SetupCommand {
    #[command(flatten)]
    pub input: Prototype1LoopCommand,

    /// Build a read-only configuration plan without performing admission-time DB, Git, identity, or provider-readiness checks.
    #[arg(long)]
    pub preview: bool,

    /// Admit only if a fresh read-only plan matches this SHA-256 from an earlier --preview.
    #[arg(long, value_name = "SHA256", conflicts_with = "preview")]
    pub expect_plan_sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Prototype1ChildScheduleMode {
    FullBatch,
    AdaptiveBatch,
}

pub(crate) mod setup_defaults {
    use super::{Prototype1ChildScheduleMode, Prototype1LoopStopAfter};

    pub const MAX_TURNS: u32 = 40;
    pub const MAX_TOOL_CALLS: u32 = 200;
    pub const WALL_CLOCK_SECS: u32 = 1800;
    pub const EVAL_MAX_TOKENS: u32 = crate::campaign::DEFAULT_EVAL_MAX_TOKENS;
    pub const MAX_GENERATIONS: u32 = 1;
    pub const MAX_TOTAL_NODES: u32 = 32;
    pub const MIN_CHILDREN: u32 = 2;
    pub const MAX_CHILDREN: u32 = 6;
    pub const CHILD_SCHEDULE: Prototype1ChildScheduleMode = Prototype1ChildScheduleMode::FullBatch;
    pub const REQUIRE_KEEP: bool = true;
    pub const EXPLORE_REJECTED: bool = true;
    pub const STOP_AFTER: Prototype1LoopStopAfter = Prototype1LoopStopAfter::InterventionApply;
}

#[derive(Debug, Parser)]
#[command(
    about = "Run the Prototype 1 loop through baseline, treatment, and compare",
    after_help = "\
Flow:
  eval configuration
    -> baseline arm (= eval run -> protocol run)
    -> target selection
    -> intervention apply
    -> treatment arm
    -> compare

Use either an existing prepared batch (--batch/--batch-id) or define the slice
inline with dataset selectors (--dataset or --dataset-key plus --instance/--specific/--all).
"
)]
pub struct Prototype1LoopCommand {
    /// Path to a prepared batch manifest. Defaults to ~/.ploke-eval/batches/<batch-id>/batch.json.
    #[arg(long, value_name = "PATH", conflicts_with_all = ["batch_id", "dataset", "dataset_key"])]
    pub batch: Option<PathBuf>,

    /// Batch id, used to resolve ~/.ploke-eval/batches/<batch-id>/batch.json.
    #[arg(long, conflicts_with_all = ["batch", "dataset", "dataset_key"])]
    pub batch_id: Option<String>,

    /// Multi-SWE-bench dataset JSONL file.
    #[arg(long, value_name = "PATH", conflicts_with = "dataset_key")]
    pub dataset: Option<PathBuf>,

    /// Built-in dataset registry key, for example ripgrep.
    #[arg(long, conflicts_with = "dataset")]
    pub dataset_key: Option<String>,

    /// Prepare every instance in the dataset.
    #[arg(long, conflicts_with_all = ["instance", "specific"])]
    pub all: bool,

    /// Exact benchmark instance id to include. Repeat for multiple instances.
    #[arg(long)]
    pub instance: Vec<String>,

    /// Substring selector matched against task id, benchmark id, org, repo, and title.
    #[arg(long)]
    pub specific: Vec<String>,

    /// Stop selecting after this many matched instances.
    #[arg(long)]
    pub limit: Option<usize>,

    /// Stable identifier for the batch manifest directory when preparing inline.
    #[arg(long)]
    pub prepare_batch_id: Option<String>,

    /// Stable human campaign/profile id for Prototype 1 state.
    #[arg(long)]
    pub campaign: Option<CampaignId>,

    /// Prototype 1 run profile name or TOML path. Profile admission is owned by setup.
    #[arg(long, value_name = "NAME_OR_PATH")]
    pub profile: Option<String>,

    /// Root directory containing repo checkouts at <repo-cache>/<org>/<repo>.
    #[arg(long, value_name = "PATH")]
    pub repo_cache: Option<PathBuf>,

    /// Root directory where ploke-eval should create per-run directories.
    #[arg(long = "instances-root", alias = "runs-root", value_name = "PATH")]
    pub instances_root: Option<PathBuf>,

    /// Root directory where ploke-eval should create batch manifests and summaries.
    #[arg(long, value_name = "PATH")]
    pub batches_root: Option<PathBuf>,

    #[arg(long, default_value_t = setup_defaults::MAX_TURNS)]
    pub max_turns: u32,

    #[arg(long, default_value_t = setup_defaults::MAX_TOOL_CALLS)]
    pub max_tool_calls: u32,

    #[arg(long, default_value_t = setup_defaults::WALL_CLOCK_SECS)]
    pub wall_clock_secs: u32,

    /// Maximum completion tokens for each baseline/treatment agent turn.
    #[arg(long, default_value_t = setup_defaults::EVAL_MAX_TOKENS)]
    pub eval_max_tokens: u32,

    /// Disable eval-only DB checkpoint/failure snapshots during indexing.
    #[arg(long = "no-index-debug-snapshots", action = ArgAction::SetFalse, default_value_t = true)]
    pub index_debug_snapshots: bool,

    /// Use the default model instead of the persisted active model selection.
    #[arg(long)]
    pub use_default_model: bool,

    /// Explicit model id to use for the baseline eval batch.
    #[arg(long)]
    pub model_id: Option<String>,

    /// Explicit provider slug to pin for the selected eval model.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Explicit route source for the selected eval model.
    #[arg(long, value_name = "ROUTE", value_parser = parse_model_route_source)]
    pub route_source: Option<ModelRouteSource>,

    /// Explicit embedding model id to use for eval indexing/retrieval.
    #[arg(long)]
    pub embedding_model_id: Option<String>,

    /// Backend route for eval embeddings.
    #[arg(
        long,
        value_name = "ROUTE",
        value_parser = parse_embedding_route
    )]
    pub embedding_route: Option<EmbeddingRoute>,

    /// Explicit provider slug to pin for the embedding model.
    #[arg(long, value_name = "PROVIDER")]
    pub embedding_provider: Option<String>,

    /// Stop the baseline batch after the first per-instance runner failure.
    #[arg(long)]
    pub stop_on_error: bool,

    /// Override the model id used for baseline protocol review.
    #[arg(long)]
    pub protocol_model_id: Option<String>,

    /// Override the provider slug used for baseline protocol review.
    #[arg(long, value_name = "PROVIDER")]
    pub protocol_provider: Option<String>,

    /// Override the route source used for baseline protocol review.
    #[arg(long, value_name = "ROUTE", value_parser = parse_model_route_source)]
    pub protocol_route_source: Option<ModelRouteSource>,

    /// Continue the loop from a previously synthesized/applied branch in another Prototype 1 campaign.
    #[arg(long, requires = "source_branch_id")]
    pub source_campaign: Option<CampaignId>,

    /// Branch id to materialize as the starting source content state for this loop generation.
    #[arg(long, requires = "source_campaign")]
    pub source_branch_id: Option<String>,

    /// Maximum generation index the search controller is allowed to continue to.
    #[arg(long, default_value_t = setup_defaults::MAX_GENERATIONS)]
    pub max_generations: u32,

    /// Maximum total staged nodes the search controller may create for this campaign.
    #[arg(long, default_value_t = setup_defaults::MAX_TOTAL_NODES)]
    pub max_total_nodes: u32,

    /// Minimum direct child candidates to evaluate before generation-level fallback selection.
    #[arg(long, default_value_t = setup_defaults::MIN_CHILDREN)]
    pub min_children: u32,

    /// Maximum direct child candidates to evaluate for one parent generation.
    #[arg(long, default_value_t = setup_defaults::MAX_CHILDREN)]
    pub max_children: u32,

    /// Child scheduling mode: run all planned children (`full-batch`) or run in `min-children` batches (`adaptive-batch`).
    #[arg(long, value_enum, default_value_t = setup_defaults::CHILD_SCHEDULE)]
    pub child_schedule_mode: Prototype1ChildScheduleMode,

    /// Stop search continuation once a keep-worthy branch is found.
    #[arg(long)]
    pub stop_on_first_keep: bool,

    /// Require the selected next branch to have overall disposition=keep before continuation.
    #[arg(long, action = ArgAction::Set, default_value_t = setup_defaults::REQUIRE_KEEP)]
    pub require_keep_for_continuation: bool,

    /// Permit the best rejected child to become the next exploration parent when no child is acceptable.
    #[arg(long, action = ArgAction::Set, default_value_t = setup_defaults::EXPLORE_REJECTED)]
    pub explore_from_rejected: bool,

    /// Stop the wrapper after the selected implemented stage.
    #[arg(long, value_enum, default_value_t = setup_defaults::STOP_AFTER)]
    pub stop_after: Prototype1LoopStopAfter,

    /// Synthesize/select the intervention candidate, but do not overwrite the target file.
    #[arg(long)]
    pub dry_run: bool,

    /// Output format: table (default) or json.
    #[arg(long, value_enum, default_value_t = InspectOutputFormat::Table)]
    pub format: InspectOutputFormat,
}

pub(crate) fn setup_command(
    request: &crate::setup_client::RunSetupRequest,
) -> Result<Prototype1LoopCommand, crate::spec::PrepareError> {
    use crate::setup_client::RunSetupBatch;

    let (batch, batch_id) = match &request.batch {
        RunSetupBatch::Manifest(path) => (Some(path.clone()), None),
        RunSetupBatch::Id(id) => (None, Some(id.clone())),
    };
    let profile = crate::setup_client::profile_selector(&request.profile)?;

    Ok(Prototype1LoopCommand {
        batch,
        batch_id,
        dataset: None,
        dataset_key: None,
        all: false,
        instance: request.primary_instance.clone().into_iter().collect(),
        specific: Vec::new(),
        limit: None,
        prepare_batch_id: None,
        campaign: Some(request.campaign.clone()),
        profile: Some(profile),
        repo_cache: None,
        instances_root: None,
        batches_root: None,
        max_turns: setup_defaults::MAX_TURNS,
        max_tool_calls: setup_defaults::MAX_TOOL_CALLS,
        wall_clock_secs: setup_defaults::WALL_CLOCK_SECS,
        eval_max_tokens: request
            .model
            .max_tokens
            .unwrap_or(setup_defaults::EVAL_MAX_TOKENS),
        index_debug_snapshots: true,
        use_default_model: request.model.use_default,
        model_id: request.model.id.clone(),
        provider: request.model.provider.clone(),
        route_source: request.model.route,
        embedding_model_id: request.embedding.id.clone(),
        embedding_route: request.embedding.route,
        embedding_provider: request.embedding.provider.clone(),
        stop_on_error: false,
        protocol_model_id: request.protocol.id.clone(),
        protocol_provider: request.protocol.provider.clone(),
        protocol_route_source: request.protocol.route,
        source_campaign: None,
        source_branch_id: None,
        max_generations: setup_defaults::MAX_GENERATIONS,
        max_total_nodes: setup_defaults::MAX_TOTAL_NODES,
        min_children: setup_defaults::MIN_CHILDREN,
        max_children: setup_defaults::MAX_CHILDREN,
        child_schedule_mode: setup_defaults::CHILD_SCHEDULE,
        stop_on_first_keep: false,
        require_keep_for_continuation: setup_defaults::REQUIRE_KEEP,
        explore_from_rejected: setup_defaults::EXPLORE_REJECTED,
        stop_after: setup_defaults::STOP_AFTER,
        dry_run: false,
        format: InspectOutputFormat::Table,
    })
}
