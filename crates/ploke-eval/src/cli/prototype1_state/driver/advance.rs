//! CLI adapters over the sealed one-edge controller operation.

use std::{fmt::Debug, path::Path, time::Duration};

use ploke_core::EXECUTION_DEBUG_TARGET;
use ploke_records::ids::CampaignId;

use crate::{
    campaign_manifest_path,
    cli::{
        InspectOutputFormat, Prototype1StateCommand,
        prototype1_process::{build_ready_receipt, record_prototype1_successor_ready},
        prototype1_state::{
            control_evidence::SuccessorOrigin,
            driver::control::{
                ControlAdvance, ControlFailure, advance_controlled, claim_controller,
                claim_successor, persisted_successor_ready, recover_successor_handoff,
                successor_transfer_release,
            },
            identity::{ParentIdentity, load_parent_identity_optional},
            invocation::{
                self, InvocationAuthority, SuccessorInvocation, process_incarnation,
                successor_publication_idle,
            },
            journal::{JournalEntry, PrototypeJournal, prototype1_transition_journal_path},
            profile::{self, RunMode, RunProfileCommitment},
            session::{AttemptReceipt, AttemptResult, Failure, Finished, Idle, Lease},
            successor::ReadyReceipt,
            walk::{
                endpoint::{self, ServerEndpoint},
                paths,
                phase::WalkPhase,
                server,
            },
        },
    },
    spec::PrepareError,
};

const TRANSFER_TIMEOUT: Duration = Duration::from_secs(30);
const TRANSFER_POLL: Duration = Duration::from_millis(50);
const PUBLICATION_TIMEOUT: Duration = Duration::from_secs(5);
const PUBLICATION_POLL: Duration = Duration::from_millis(10);

// ANCHOR: prototype1_run_to_terminal
/// Run one complete parent turn under a continuous controller-session lease.
pub(crate) async fn run_to_terminal(
    command: Prototype1StateCommand,
    allow_live_api: bool,
    allow_git_changes: bool,
) -> Result<(), PrepareError> {
    let repo_root = paths::resolve_use_repo_root(command.repo_root.as_deref())?;
    let admitted = validate_command(&command, &repo_root)?;
    if let Some(path) = command.handoff_invocation.as_deref() {
        await_successor_publication(&repo_root, path, &admitted.commitment).await?;
    }
    match command.handoff_invocation.as_deref() {
        Some(path) => match admitted.profile.control.mode {
            RunMode::Continuous => {
                run_continuous(
                    &repo_root,
                    command.campaign.as_ref(),
                    Some(path),
                    allow_live_api,
                    allow_git_changes,
                )
                .await
            }
            RunMode::Step => serve_successor(&repo_root, path).await,
        },
        None => {
            run_continuous(
                &repo_root,
                command.campaign.as_ref(),
                None,
                allow_live_api,
                allow_git_changes,
            )
            .await
        }
    }
}

/// Resume a profile-admitted continuous session without rebuilding an R0
/// command or invoking the legacy diagnosis-driven mutator.
pub(crate) async fn continue_session(
    repo_root: Option<&Path>,
    allow_live_api: bool,
    allow_git_changes: bool,
) -> Result<(), PrepareError> {
    let repo_root = paths::resolve_use_repo_root(repo_root)?;
    run_continuous(&repo_root, None, None, allow_live_api, allow_git_changes).await
}

/// Submit one Step-mode typed edge through the same durable authority used by
/// walk. Live model work is admitted; checkout mutation remains an explicit
/// walk capability until the shared command contract carries confirmations.
pub(crate) async fn step_session(
    repo_root: Option<&Path>,
    allow_live_api: bool,
    allow_git_changes: bool,
) -> Result<Option<AttemptReceipt>, PrepareError> {
    let repo_root = paths::resolve_use_repo_root(repo_root)?;
    let lease = claim_controller(&repo_root, RunMode::Step)?;
    if matches!(lease.cursor().phase, WalkPhase::R14a | WalkPhase::R14b) {
        release(lease)?;
        return Ok(None);
    }
    let intent = match lease.intent_with_live_api(allow_live_api, allow_git_changes) {
        Ok(intent) => intent,
        Err(source) => {
            let release = release(lease).err();
            return Err(attach_release(session_error(source), release));
        }
    };
    let (lease, receipt) = match advance_controlled(lease, intent).await {
        Ok(ControlAdvance::Existing { lease, receipt }) => (lease, receipt),
        Ok(ControlAdvance::Finished(Finished::Terminal { lease, receipt, .. })) => (lease, receipt),
        Ok(ControlAdvance::Finished(Finished::Uncertain { receipt, .. })) => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "controlled transition became indeterminate: {}",
                    result_detail(&receipt.result)
                ),
            });
        }
        Err(failure) => return Err(control_error(failure)),
    };
    let result = receipt.result.clone();
    release(lease)?;
    match result {
        AttemptResult::Committed { .. } => Ok(Some(receipt)),
        AttemptResult::Rejected { detail, .. } | AttemptResult::Cancelled { detail, .. } => {
            Err(PrepareError::InvalidBatchSelection { detail })
        }
        AttemptResult::Indeterminate { detail, .. } => {
            Err(PrepareError::InvalidBatchSelection { detail })
        }
    }
}

async fn run_continuous(
    repo_root: &Path,
    requested: Option<&CampaignId>,
    handoff: Option<&Path>,
    allow_live_api: bool,
    allow_git_changes: bool,
) -> Result<(), PrepareError> {
    let invocation = handoff.map(load_successor).transpose()?;
    if let Some(invocation) = invocation.as_ref() {
        invocation.validate_capabilities(allow_live_api, allow_git_changes)?;
    }
    if let Some(path) = handoff
        && let Some(ready) = persisted_successor_ready(repo_root, path)?
    {
        if successor_transfer_release(repo_root, RunMode::Continuous, path)?.is_none() {
            if ready_owned_by_current(&ready)? {
                wait_for_predecessor(repo_root, RunMode::Continuous, path).await?;
            } else {
                recover_successor_handoff(repo_root, RunMode::Continuous, path)?;
            }
        }
    }
    let mut lease = match handoff {
        Some(path) => claim_successor(repo_root, RunMode::Continuous, path)?,
        None => claim_controller(repo_root, RunMode::Continuous)?,
    };
    if requested.is_some_and(|campaign| campaign != lease.campaign_id()) {
        let active = lease.campaign_id().clone();
        release(lease)?;
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "requested campaign '{}' does not match active parent campaign '{active}'",
                requested.expect("checked campaign")
            ),
        });
    }
    if let Some(path) = handoff
        && matches!(lease.cursor().phase, WalkPhase::R3 | WalkPhase::R4a)
    {
        let invocation = invocation
            .as_ref()
            .expect("handoff path was loaded before successor claim");
        lease = bootstrap_successor(lease).await?;
        let runtime = invocation.runtime_id();
        let commit = lease.prepare_ready(runtime).map_err(session_error)?;
        let receipt = build_ready_receipt(invocation, commit, None, None)?;
        release_ready(lease, &receipt)?;
        record_prototype1_successor_ready(invocation, receipt)?;
        wait_for_predecessor(repo_root, RunMode::Continuous, path).await?;
        lease = claim_successor(repo_root, RunMode::Continuous, path)?;
    }
    let span_campaign_id = lease.campaign_id().clone();
    let turn_span = tracing::info_span!(
        target: EXECUTION_DEBUG_TARGET,
        "prototype1.parent.turn",
        role = "parent",
        phase = "parent_turn",
        campaign = %span_campaign_id,
    );
    let _turn_entered = turn_span.enter();

    let mut guard = 0_u8;
    loop {
        match lease.cursor().phase {
            WalkPhase::R14a | WalkPhase::R14b => return release(lease),
            WalkPhase::R13c => {
                release(lease)?;
                return Err(PrepareError::InvalidBatchSelection {
                    detail: "controller reached R13c with an incomplete successor handoff; explicit recovery is required"
                        .to_string(),
                });
            }
            _ => {}
        }
        guard = guard.saturating_add(1);
        if guard > 24 {
            release(lease)?;
            return Err(PrepareError::InvalidBatchSelection {
                detail: "controlled Prototype 1 batch exceeded the R3-R14 step bound".to_string(),
            });
        }
        let intent = match lease.intent_with_live_api(allow_live_api, allow_git_changes) {
            Ok(intent) => intent,
            Err(source) => {
                let release = release(lease).err();
                return Err(attach_release(session_error(source), release));
            }
        };
        lease = match advance_controlled(lease, intent).await {
            Ok(ControlAdvance::Existing { lease, .. }) => lease,
            Ok(ControlAdvance::Finished(Finished::Terminal { lease, .. })) => lease,
            Ok(ControlAdvance::Finished(Finished::Uncertain { receipt, .. })) => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "controlled transition became indeterminate: {}",
                        result_detail(&receipt.result)
                    ),
                });
            }
            Err(failure) => return Err(control_error(failure)),
        };
    }
}

/// Admit one successor bootstrap through its persisted transfer, advance only
/// to the predecessor-ready boundary, then expose the shared walk service.
async fn serve_successor(repo_root: &Path, handoff: &Path) -> Result<(), PrepareError> {
    let invocation = load_successor(handoff)?;
    let persisted = persisted_successor_ready(repo_root, handoff)?;
    if let Some(receipt) = persisted.as_ref() {
        let endpoint = receipt
            .endpoint()
            .expect("validated Step Ready receipt has an endpoint");
        let expected = paths::successor_socket(repo_root, invocation.runtime_id())?;
        if endpoint.socket() != expected {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "persisted successor endpoint '{}' does not match runtime slot '{}'",
                    endpoint.socket().display(),
                    expected.display()
                ),
            });
        }
        if server::endpoint_reachable(endpoint)? {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor walk endpoint '{}' is already owned by pid {}; attach to that server instead of starting a second controller",
                    endpoint.socket().display(),
                    endpoint.pid()
                ),
            });
        }
    }
    let predecessor = match persisted.as_ref() {
        Some(receipt) => receipt.predecessor().cloned(),
        None => endpoint::load(repo_root)?,
    };
    let prior = match persisted.as_ref() {
        Some(ready) => match successor_transfer_release(repo_root, RunMode::Step, handoff)? {
            Some(release) => Some(release),
            None if ready_owned_by_current(ready)? => None,
            None => Some(recover_successor_handoff(
                repo_root,
                RunMode::Step,
                handoff,
            )?),
        },
        None => None,
    };
    let prepared = server::prepare_successor(repo_root, invocation.runtime_id())?;
    let endpoint = prepared.endpoint().clone();
    let gate = server::MutationGate::closed();
    let server_gate = gate.clone();
    let handle = tokio::spawn(async move { server::serve_prepared(prepared, server_gate).await });

    let transfer = if persisted.is_some() {
        let transfer = match prior {
            Some(transfer) => transfer,
            None => match wait_for_predecessor(repo_root, RunMode::Step, handoff).await {
                Ok(transfer) => transfer,
                Err(error) => return Err(stop_server(handle, &endpoint, error).await),
            },
        };
        let lease = match claim_successor(repo_root, RunMode::Step, handoff) {
            Ok(lease) => lease,
            Err(error) => return Err(stop_server(handle, &endpoint, error).await),
        };
        if let Err(error) = release(lease) {
            return Err(stop_server(handle, &endpoint, error).await);
        }
        transfer
    } else {
        let mut lease = match claim_successor(repo_root, RunMode::Step, handoff) {
            Ok(lease) => lease,
            Err(error) => return Err(stop_server(handle, &endpoint, error).await),
        };
        lease = match bootstrap_successor(lease).await {
            Ok(lease) => lease,
            Err(error) => return Err(stop_server(handle, &endpoint, error).await),
        };
        let commit = match lease.prepare_ready(invocation.runtime_id()) {
            Ok(commit) => commit,
            Err(error) => {
                let error = session_error(error);
                return Err(stop_server(handle, &endpoint, error).await);
            }
        };
        let receipt = match build_ready_receipt(
            &invocation,
            commit,
            Some(endpoint.clone()),
            predecessor.clone(),
        ) {
            Ok(receipt) => receipt,
            Err(error) => return Err(stop_server(handle, &endpoint, error).await),
        };
        if let Err(error) = release_ready(lease, &receipt) {
            return Err(stop_server(handle, &endpoint, error).await);
        }
        if let Err(error) = record_prototype1_successor_ready(&invocation, receipt) {
            return Err(stop_server(handle, &endpoint, error).await);
        }
        match wait_for_predecessor(repo_root, RunMode::Step, handoff).await {
            Ok(transfer) => transfer,
            Err(error) => return Err(stop_server(handle, &endpoint, error).await),
        }
    };
    if let Err(error) = server::activate_successor(&endpoint, predecessor.as_ref(), &gate, transfer)
    {
        return Err(stop_server(handle, &endpoint, error).await);
    }
    match handle.await {
        Ok(result) => result,
        Err(source) => Err(PrepareError::DatabaseSetup {
            phase: "prototype1_successor_walk_join",
            detail: source.to_string(),
        }),
    }
}

fn ready_owned_by_current(
    ready: &super::super::successor::ReadyReceipt,
) -> Result<bool, PrepareError> {
    ready
        .owned_by_current()
        .map_err(|detail| PrepareError::InvalidBatchSelection { detail })
}

async fn bootstrap_successor(mut lease: Lease<Idle>) -> Result<Lease<Idle>, PrepareError> {
    let mut guard = 0_u8;
    while matches!(lease.cursor().phase, WalkPhase::R3 | WalkPhase::R4a) {
        guard = guard.saturating_add(1);
        if guard > 2 {
            let phase = lease.cursor().phase;
            release(lease)?;
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!("successor bootstrap exceeded the R3-R4c edge bound at {phase}"),
            });
        }
        let intent = match lease.intent_with_live_api(false, false) {
            Ok(intent) => intent,
            Err(source) => {
                let release = release(lease).err();
                return Err(attach_release(session_error(source), release));
            }
        };
        lease = match advance_controlled(lease, intent).await {
            Ok(ControlAdvance::Existing { lease, .. }) => lease,
            Ok(ControlAdvance::Finished(Finished::Terminal { lease, .. })) => lease,
            Ok(ControlAdvance::Finished(Finished::Uncertain { receipt, .. })) => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "successor bootstrap became indeterminate: {}",
                        result_detail(&receipt.result)
                    ),
                });
            }
            Err(failure) => return Err(control_error(failure)),
        };
    }
    if lease.cursor().phase != WalkPhase::R4c {
        let phase = lease.cursor().phase;
        release(lease)?;
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "successor runtime must enter its post-bootstrap boundary at R4c, found {phase}"
            ),
        });
    }
    Ok(lease)
}

fn load_successor(path: &Path) -> Result<SuccessorInvocation, PrepareError> {
    match invocation::load_executable(path)? {
        InvocationAuthority::Successor(invocation) => Ok(invocation),
        InvocationAuthority::Child(_) => Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "handoff invocation '{}' grants child authority, not successor control",
                path.display()
            ),
        }),
    }
}

/// Wait until the predecessor's synced Spawned publication is both unlocked
/// and reconstructible as the exact typed successor origin.
async fn await_successor_publication(
    repo_root: &Path,
    invocation_path: &Path,
    profile: &RunProfileCommitment,
) -> Result<(), PrepareError> {
    let invocation = load_successor(invocation_path)?;
    let parent = load_parent_identity_optional(repo_root)?.ok_or_else(|| {
        PrepareError::InvalidBatchSelection {
            detail: "successor publication cannot resolve the installed parent identity"
                .to_string(),
        }
    })?;
    let manifest = campaign_manifest_path(parent.campaign_id())?;
    let journal_path = prototype1_transition_journal_path(&manifest);
    if !same_path(invocation.journal_path(), &journal_path) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "successor invocation journal '{}' does not match campaign journal '{}'",
                invocation.journal_path().display(),
                journal_path.display()
            ),
        });
    }
    await_successor_publication_at(
        repo_root,
        invocation_path,
        &invocation,
        &parent,
        profile,
        PUBLICATION_TIMEOUT,
    )
    .await
}

async fn await_successor_publication_at(
    repo_root: &Path,
    invocation_path: &Path,
    invocation: &SuccessorInvocation,
    parent: &ParentIdentity,
    profile: &RunProfileCommitment,
    timeout: Duration,
) -> Result<(), PrepareError> {
    let started = tokio::time::Instant::now();
    loop {
        if successor_publication_idle(invocation_path)?
            && published_successor_origin(repo_root, invocation_path, invocation, parent, profile)?
                .is_some()
        {
            return Ok(());
        }
        if started.elapsed() >= timeout {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor Spawned publication for runtime {} was not durably visible within {}s",
                    invocation.runtime_id(),
                    timeout.as_secs()
                ),
            });
        }
        tokio::time::sleep(PUBLICATION_POLL).await;
    }
}

fn published_successor_origin(
    repo_root: &Path,
    invocation_path: &Path,
    invocation: &SuccessorInvocation,
    parent: &ParentIdentity,
    profile: &RunProfileCommitment,
) -> Result<Option<SuccessorOrigin>, PrepareError> {
    let entries = PrototypeJournal::new(invocation.journal_path().to_path_buf())
        .load_entries()
        .map_err(|source| PrepareError::InvalidBatchSelection {
            detail: format!("cannot inspect successor publication journal: {source}"),
        })?;
    let runtime = invocation.runtime_id();
    let mut spawned = None;
    let mut runtime_seen = false;
    for (index, entry) in entries.iter().enumerate() {
        let JournalEntry::Successor(record) = entry else {
            continue;
        };
        if record.runtime_id != Some(runtime) {
            continue;
        }
        runtime_seen = true;
        if record.campaign_id != *invocation.campaign_id() || record.node_id != invocation.node_id()
        {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor runtime {runtime} was published under different campaign or node coordinates"
                ),
            });
        }
        if !matches!(record.state, super::super::successor::State::Spawned { .. }) {
            continue;
        }
        if spawned.is_some() {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor runtime {runtime} has more than one Spawned publication"
                ),
            });
        }
        spawned = Some((index, record.clone()));
    }
    let Some((spawn_index, spawned)) = spawned else {
        if runtime_seen {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor runtime {runtime} has lifecycle evidence without a preceding Spawned publication"
                ),
            });
        }
        return Ok(None);
    };
    let checkout = entries
        .iter()
        .take(spawn_index)
        .rev()
        .find_map(|entry| match entry {
            JournalEntry::ActiveCheckoutAdvanced(entry)
                if entry.campaign_id == *parent.campaign_id()
                    && entry.selected_parent_identity == *parent
                    && same_path(&entry.active_parent_root, repo_root) =>
            {
                Some(entry.clone())
            }
            _ => None,
        })
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: "successor publication is missing its preceding installed checkout record"
                .to_string(),
        })?;
    let path = invocation_path
        .canonicalize()
        .map_err(|source| PrepareError::ReadManifest {
            path: invocation_path.to_path_buf(),
            source,
        })?;
    let origin = SuccessorOrigin::new(
        path,
        invocation.as_invocation().clone(),
        checkout,
        spawned,
        parent,
        profile,
        repo_root,
    )
    .map_err(|source| PrepareError::InvalidBatchSelection {
        detail: format!("invalid successor Spawned publication: {source}"),
    })?;
    let incarnation =
        origin
            .spawned_incarnation()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "successor Spawned publication has no exact process incarnation"
                    .to_string(),
            })?;
    if incarnation.boot_id.is_nil() || incarnation.start_ticks == 0 {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "successor Spawned publication has an incomplete process incarnation"
                .to_string(),
        });
    }
    let later = entries
        .iter()
        .skip(spawn_index + 1)
        .any(|entry| match entry {
            JournalEntry::Successor(record) => {
                record.campaign_id == *invocation.campaign_id()
                    && record.node_id == invocation.node_id()
                    && record.runtime_id == Some(runtime)
            }
            JournalEntry::SuccessorHandoff(handoff) => {
                handoff.campaign_id == *invocation.campaign_id()
                    && handoff.node_id == invocation.node_id()
                    && handoff.runtime_id == runtime
            }
            _ => false,
        });
    if !later {
        let pid = std::process::id();
        let actual = process_incarnation(pid)
            .map_err(|source| PrepareError::InvalidBatchSelection {
                detail: format!("cannot inspect successor process {pid}: {source}"),
            })?
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!("successor process {pid} disappeared before publication admission"),
            })?;
        let binary =
            std::env::current_exe().map_err(|source| PrepareError::InvalidBatchSelection {
                detail: format!("cannot resolve successor executable identity: {source}"),
            })?;
        if origin.spawned_pid() != pid
            || origin.spawned_incarnation() != Some(&actual)
            || !same_path(origin.binary_path(), &binary)
        {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "only the exact process recorded by the initial Spawned publication may cross the startup barrier"
                    .to_string(),
            });
        }
    }
    Ok(Some(origin))
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

async fn wait_for_predecessor(
    repo_root: &Path,
    mode: RunMode,
    handoff: &Path,
) -> Result<super::control::PredecessorRelease, PrepareError> {
    let started = tokio::time::Instant::now();
    loop {
        if let Some(release) = successor_transfer_release(repo_root, mode, handoff)? {
            return Ok(release);
        }
        if started.elapsed() >= TRANSFER_TIMEOUT {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor transfer remained pending for {}s after Ready; predecessor did not commit and cleanly release the accepted R12->R13b attempt",
                    TRANSFER_TIMEOUT.as_secs()
                ),
            });
        }
        tokio::time::sleep(TRANSFER_POLL).await;
    }
}

fn attach_endpoint(source: PrepareError, endpoint: &ServerEndpoint) -> PrepareError {
    attach_release(source, endpoint.cleanup().err())
}

async fn stop_server(
    handle: tokio::task::JoinHandle<Result<(), PrepareError>>,
    endpoint: &ServerEndpoint,
    source: PrepareError,
) -> PrepareError {
    handle.abort();
    let _ = handle.await;
    attach_endpoint(source, endpoint)
}
// ANCHOR_END: prototype1_run_to_terminal

fn validate_command(
    command: &Prototype1StateCommand,
    repo_root: &Path,
) -> Result<profile::AdmittedRunProfile, PrepareError> {
    if command.init_parent_identity
        || command.identity_branch.is_some()
        || command.identity_instance.is_some()
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "parent identity bootstrap is owned by `prototype1-setup`; a live controller session begins at its completed R3 receipt"
                .to_string(),
        });
    }
    if command.node_id.is_some() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "`--node-id` is not controller authority; omit it and use the setup-admitted parent identity"
                .to_string(),
        });
    }
    if command.format != InspectOutputFormat::Table {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "the session-backed `prototype1-state` adapter does not yet carry a JSON presentation request through reconstruction; use table output until the projection adapter is explicit"
                .to_string(),
        });
    }

    let parent = load_parent_identity_optional(repo_root)?.ok_or_else(|| {
        PrepareError::InvalidBatchSelection {
            detail: format!(
                "no Prototype 1 parent identity exists under '{}'",
                repo_root.display()
            ),
        }
    })?;
    if command
        .campaign
        .as_ref()
        .is_some_and(|campaign| campaign != parent.campaign_id())
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "requested campaign '{}' does not match active parent campaign '{}'",
                command.campaign.as_ref().expect("checked campaign"),
                parent.campaign_id()
            ),
        });
    }
    let manifest = campaign_manifest_path(parent.campaign_id())?;
    let admitted = profile::load_admitted_run_profile(&manifest)?.ok_or_else(|| {
        PrepareError::InvalidBatchSelection {
            detail: format!(
                "controller session requires an admitted run profile for '{}'",
                manifest.display()
            ),
        }
    })?;
    assert_profile(
        "--stop-after",
        command.stop_after,
        admitted.profile.execution.state_stop_after(),
    )?;
    assert_profile(
        "--successor-selection",
        command.successor_selection,
        admitted.profile.selection.successor_selection(),
    )?;
    assert_profile(
        "--successor-selection-seed",
        command.successor_selection_seed,
        admitted.profile.selection.seed,
    )?;
    assert_profile(
        "--successor-selection-metrics",
        command.successor_selection_metrics,
        admitted.profile.selection.traversal_metrics(),
    )?;
    assert_profile(
        "--candidate-generator",
        command.candidate_generator,
        admitted.profile.generation.candidate_generator(),
    )?;
    Ok(admitted)
}

fn assert_profile<T>(flag: &str, requested: Option<T>, admitted: T) -> Result<(), PrepareError>
where
    T: Copy + Debug + PartialEq,
{
    if requested.is_some_and(|value| value != admitted) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "{flag}={:?} does not match the admitted run profile value {admitted:?}",
                requested.expect("checked assertion")
            ),
        });
    }
    Ok(())
}

fn release(lease: Lease<Idle>) -> Result<(), PrepareError> {
    match lease.release() {
        Ok(_) => Ok(()),
        Err(Failure::Retained {
            authority,
            source: first,
        }) => authority
            .release()
            .map(|_| ())
            .map_err(|retry| match retry {
                Failure::Retained { source, .. } | Failure::Uncertain { source } => {
                    session_error(format!("{source}; first release failed: {first}"))
                }
            }),
        Err(Failure::Uncertain { source }) => Err(session_error(source)),
    }
}

fn release_ready(lease: Lease<Idle>, receipt: &ReadyReceipt) -> Result<(), PrepareError> {
    match lease.release_ready(receipt.clone()) {
        Ok(_) => Ok(()),
        Err(Failure::Retained {
            authority,
            source: first,
        }) => authority
            .release_ready(receipt.clone())
            .map(|_| ())
            .map_err(|retry| match retry {
                Failure::Retained { source, .. } | Failure::Uncertain { source } => {
                    session_error(format!("{source}; first Ready release failed: {first}"))
                }
            }),
        Err(Failure::Uncertain { source }) => Err(session_error(source)),
    }
}

fn control_error(failure: ControlFailure) -> PrepareError {
    match failure {
        ControlFailure::Reconstruct { lease, source } => {
            let release = release(lease).err();
            attach_release(source, release)
        }
        ControlFailure::Admission(Failure::Retained { authority, source }) => {
            let release = release(authority).err();
            attach_release(session_error(source), release)
        }
        ControlFailure::Admission(Failure::Uncertain { source }) => session_error(source),
        ControlFailure::Persist(Failure::Retained { source, .. })
        | ControlFailure::Persist(Failure::Uncertain { source }) => session_error(source),
    }
}

fn attach_release(source: PrepareError, release: Option<PrepareError>) -> PrepareError {
    match release {
        Some(release) => PrepareError::InvalidBatchSelection {
            detail: format!("{source}; controller release also failed: {release}"),
        },
        None => source,
    }
}

fn result_detail(result: &AttemptResult) -> &str {
    match result {
        AttemptResult::Rejected { detail, .. }
        | AttemptResult::Cancelled { detail, .. }
        | AttemptResult::Indeterminate { detail, .. } => detail,
        AttemptResult::Committed { .. } => "committed result lost idle authority",
    }
}

fn session_error(source: impl std::fmt::Display) -> PrepareError {
    PrepareError::InvalidBatchSelection {
        detail: format!("controller session error: {source}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use crate::{
        cli::prototype1_state::{
            event::{RecordedAt, RuntimeId},
            identity::{PARENT_IDENTITY_SCHEMA_VERSION, ParentIdentityRecord},
            invocation::{
                Invocation, Role, SCHEMA_VERSION, hold_successor_publication, process_incarnation,
            },
            journal::{ActiveCheckoutAdvancedEntry, Streams},
            successor::Record as SuccessorRecord,
        },
        intervention::RecordStore,
    };

    #[tokio::test(flavor = "current_thread")]
    async fn successor_waits_for_synced_spawn_publication() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo_root = temp.path().join("repo");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let campaign = CampaignId::from("publication-campaign");
        let predecessor = ParentIdentity::root_bootstrap(
            campaign.clone(),
            "predecessor",
            "instance",
            "branch-predecessor",
            Some("artifact-predecessor".to_string()),
        );
        let parent = ParentIdentity::from_record_for_test(ParentIdentityRecord {
            schema_version: PARENT_IDENTITY_SCHEMA_VERSION.to_string(),
            campaign_id: campaign.clone(),
            parent_id: "successor".to_string(),
            node_id: "successor".to_string(),
            generation: 1,
            instance_id: Some("instance".to_string()),
            previous_parent_id: Some(predecessor.parent_id().to_string()),
            parent_node_id: Some(predecessor.node_id().to_string()),
            branch_id: "branch-successor".to_string(),
            artifact_branch: Some("artifact-successor".to_string()),
            created_at: "2026-07-13T00:00:00Z".to_string(),
        });
        let profile = RunProfileCommitment {
            schema_version: "prototype1-run-profile-commitment.v1".to_string(),
            profile_path: temp.path().join("run-profile.toml"),
            sha256: "profile-sha".to_string(),
            source_path: None,
            admitted_at: "2026-07-13T00:00:00Z".to_string(),
        };
        let runtime = RuntimeId::new();
        let node_dir = temp.path().join("prototype1/nodes/successor");
        let journal_path = temp.path().join("prototype1/transition-journal.jsonl");
        let invocation_path = invocation::invocation_path(&node_dir, runtime);
        fs::create_dir_all(
            invocation_path
                .parent()
                .expect("invocation parent directory"),
        )
        .expect("create invocation directory");
        let raw = Invocation {
            schema_version: SCHEMA_VERSION.to_string(),
            role: Role::Successor,
            campaign_id: campaign.clone(),
            node_id: parent.node_id().to_string(),
            runtime_id: runtime,
            journal_path: journal_path.clone(),
            channel_root: Some(invocation::channel_root(&node_dir, runtime)),
            node: None,
            request: None,
            resolved: None,
            active_parent_root: Some(repo_root.clone()),
            run_profile: Some(profile.clone()),
            predecessor_attempt: Some(
                crate::cli::prototype1_state::successor::PredecessorAttempt::new(
                    crate::cli::prototype1_state::session::SessionId::for_test(1),
                    crate::cli::prototype1_state::event::TransitionId::new(),
                    crate::cli::prototype1_state::session::Fence::for_test(1),
                    true,
                    true,
                ),
            ),
            created_at: "2026-07-13T00:00:00Z".to_string(),
        };
        fs::write(
            &invocation_path,
            serde_json::to_vec(&raw).expect("serialize invocation"),
        )
        .expect("write invocation");
        let invocation = load_successor(&invocation_path).expect("load successor invocation");
        let mut journal = PrototypeJournal::new(journal_path);
        journal
            .append(JournalEntry::ActiveCheckoutAdvanced(
                ActiveCheckoutAdvancedEntry {
                    recorded_at: RecordedAt::now(),
                    campaign_id: campaign,
                    previous_parent_identity: Some(predecessor),
                    selected_parent_identity: parent.clone(),
                    active_parent_root: repo_root.clone(),
                    selected_branch: "artifact-successor".to_string(),
                    installed_commit: "installed-commit".to_string(),
                },
            ))
            .expect("append installed checkout");

        let publication = hold_successor_publication(&invocation_path)
            .expect("hold publication before successor launch");
        let wait = await_successor_publication_at(
            &repo_root,
            &invocation_path,
            &invocation,
            &parent,
            &profile,
            Duration::from_secs(1),
        );
        tokio::pin!(wait);
        assert!(
            tokio::time::timeout(Duration::from_millis(25), wait.as_mut())
                .await
                .is_err(),
            "successor must remain behind the barrier before Spawned is published"
        );

        let incarnation = process_incarnation(std::process::id())
            .expect("inspect current process")
            .expect("current process incarnation");
        journal
            .append(JournalEntry::Successor(SuccessorRecord::spawned(
                &invocation,
                std::process::id(),
                incarnation,
                repo_root.clone(),
                std::env::current_exe().expect("current executable"),
                invocation_path.clone(),
                node_dir.join("successor-ready.json"),
                Streams {
                    stdout: node_dir.join("successor.stdout"),
                    stderr: node_dir.join("successor.stderr"),
                },
            )))
            .expect("sync exact Spawned publication");
        assert!(
            tokio::time::timeout(Duration::from_millis(25), wait.as_mut())
                .await
                .is_err(),
            "journal visibility alone must not bypass the predecessor-held barrier"
        );

        drop(publication);
        tokio::time::timeout(Duration::from_secs(1), wait)
            .await
            .expect("publication wait completes after release")
            .expect("exact Spawned origin is admitted");
    }
}
