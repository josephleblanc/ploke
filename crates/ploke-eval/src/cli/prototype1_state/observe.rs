//! Thin tracing helpers for Prototype 1 runtime observation.
//!
//! This module is telemetry-only. History and journal records remain the
//! authoritative projections of allowed protocol transitions.

use std::fmt;
use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::process::Output;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use ploke_records::ids::CampaignId;

use tracing::{Instrument, Level, Span, event};

use super::{
    eval_store::{TraceEventEvidence, write_trace_event_to_owner_db},
    identity::ParentIdentity,
    inner::{At, Transition},
    parent::ChildPlanFile,
};

pub(crate) const TARGET: &str = ploke_core::EXECUTION_DEBUG_TARGET;

#[derive(Debug, Clone)]
pub(crate) struct EvalTraceSinkConfig {
    pub(crate) campaign_id: CampaignId,
    pub(crate) db_path: PathBuf,
}

#[derive(Debug, Clone)]
struct ActiveEvalTraceSink {
    config: EvalTraceSinkConfig,
    next_index: i64,
}

#[derive(Debug)]
pub(crate) struct EvalTraceSinkGuard {
    previous: Option<ActiveEvalTraceSink>,
}

impl Drop for EvalTraceSinkGuard {
    fn drop(&mut self) {
        let mut slot = eval_trace_sink().lock().expect("eval trace sink mutex");
        *slot = self.previous.take();
    }
}

pub(crate) fn scoped_eval_trace_sink(config: Option<EvalTraceSinkConfig>) -> EvalTraceSinkGuard {
    let mut slot = eval_trace_sink().lock().expect("eval trace sink mutex");
    let previous = slot.take();
    *slot = config.map(|config| ActiveEvalTraceSink {
        config,
        next_index: 0,
    });
    EvalTraceSinkGuard { previous }
}

fn eval_trace_sink() -> &'static Mutex<Option<ActiveEvalTraceSink>> {
    static SINK: OnceLock<Mutex<Option<ActiveEvalTraceSink>>> = OnceLock::new();
    SINK.get_or_init(|| Mutex::new(None))
}

fn mirror_trace_event(mut evidence: TraceEventEvidence) {
    let mut slot = eval_trace_sink().lock().expect("eval trace sink mutex");
    let Some(sink) = slot.as_mut() else {
        return;
    };
    if evidence.campaign_id.is_none() {
        evidence.campaign_id = Some(sink.config.campaign_id.clone());
    }
    if evidence.source_event_index.is_none() {
        evidence.source_event_index = Some(sink.next_index);
        sink.next_index = sink.next_index.checked_add(1).unwrap_or(i64::MAX);
    }
    if let Err(error) = write_trace_event_to_owner_db(&sink.config.db_path, evidence) {
        tracing::warn!(
            target: TARGET,
            db_path = %sink.config.db_path.display(),
            error = %error,
            "prototype1 eval trace mirror failed"
        );
    }
}

macro_rules! span {
    ($name:literal) => {
        tracing::info_span!(target: $crate::cli::prototype1_state::observe::TARGET, $name)
    };
    ($name:literal, $($fields:tt)+) => {
        tracing::info_span!(target: $crate::cli::prototype1_state::observe::TARGET, $name, $($fields)+)
    };
}

pub(crate) use span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    Parent,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Self::Parent => "parent",
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pipeline {
    ChildPlanAuthority,
}

impl Pipeline {
    fn as_str(self) -> &'static str {
        match self {
            Self::ChildPlanAuthority => "prototype1.child_plan_authority",
        }
    }
}

impl fmt::Display for Pipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    TypestateTransition,
    RequestPublication,
    BatchAdmission,
    FailedBatchPersistence,
    RetryReplay,
    MessageLock,
    MessageReceive,
}

impl Stage {
    fn as_str(self) -> &'static str {
        match self {
            Self::TypestateTransition => "typestate_transition",
            Self::RequestPublication => "request_publication",
            Self::BatchAdmission => "batch_admission",
            Self::FailedBatchPersistence => "failed_batch_persistence",
            Self::RetryReplay => "retry_replay",
            Self::MessageLock => "message_lock",
            Self::MessageReceive => "message_receive",
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Authority {
    ParentBroadcastChannel,
}

impl Authority {
    fn as_str(self) -> &'static str {
        match self {
            Self::ParentBroadcastChannel => "parent_broadcast_channel",
        }
    }
}

impl fmt::Display for Authority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordKind {
    ChildPlanFile,
}

impl RecordKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::ChildPlanFile => "child_plan_file",
        }
    }
}

impl fmt::Display for RecordKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordAccess {
    Read,
    Write,
}

impl RecordAccess {
    fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

impl fmt::Display for RecordAccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum RecordRef<'a> {
    ChildPlanFile(&'a At<ChildPlanFile>),
}

impl RecordRef<'_> {
    fn kind(&self) -> RecordKind {
        match self {
            Self::ChildPlanFile(_) => RecordKind::ChildPlanFile,
        }
    }

    fn path(&self) -> &std::path::Path {
        match self {
            Self::ChildPlanFile(at) => at.path(),
        }
    }
}

pub(crate) trait ObservedTransition: Transition {
    const ROLE: Role;
    const PIPELINE: Pipeline;
    const STAGE: Stage;
    const AUTHORITY: Authority;
    const LABEL: &'static str;
}

#[derive(Debug, Clone, Copy)]
struct RecordSlot<'a> {
    access: RecordAccess,
    record: RecordRef<'a>,
}

const MAX_TRANSITION_RECORDS: usize = 6;

#[derive(Debug, Clone, Copy)]
struct RecordSlots<'a> {
    records: [Option<RecordSlot<'a>>; MAX_TRANSITION_RECORDS],
    len: usize,
}

impl<'a> RecordSlots<'a> {
    fn empty() -> Self {
        Self {
            records: [None; MAX_TRANSITION_RECORDS],
            len: 0,
        }
    }

    fn push(&mut self, access: RecordAccess, record: RecordRef<'a>) {
        if self.len < MAX_TRANSITION_RECORDS {
            self.records[self.len] = Some(RecordSlot { access, record });
            self.len += 1;
        } else {
            debug_assert!(
                false,
                "prototype1 typestate transition recorded too many persistence refs"
            );
        }
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn len(&self) -> usize {
        self.len
    }

    fn iter(&self) -> impl Iterator<Item = RecordSlot<'a>> + '_ {
        self.records[..self.len].iter().filter_map(|record| *record)
    }
}

#[derive(Debug)]
pub(crate) struct TransitionBuilder<'a, T: ObservedTransition> {
    parent: &'a ParentIdentity,
    stage: Stage,
    records: RecordSlots<'a>,
    _transition: PhantomData<T>,
}

pub(crate) fn transition<'a, T: ObservedTransition>(
    parent: &'a ParentIdentity,
) -> TransitionBuilder<'a, T> {
    TransitionBuilder {
        parent,
        stage: T::STAGE,
        records: RecordSlots::empty(),
        _transition: PhantomData,
    }
}

impl<'a, T: ObservedTransition> TransitionBuilder<'a, T> {
    pub(crate) fn stage(mut self, stage: Stage) -> Self {
        self.stage = stage;
        self
    }

    pub(crate) fn reads(mut self, record: RecordRef<'a>) -> Self {
        self.records.push(RecordAccess::Read, record);
        self
    }

    pub(crate) fn writes(mut self, record: RecordRef<'a>) -> Self {
        self.records.push(RecordAccess::Write, record);
        self
    }

    pub(crate) fn commit<R>(self, run: impl FnOnce() -> R) -> R {
        let started = Instant::now();
        let value = run();
        self.emit("committed", started.elapsed(), None);
        value
    }

    pub(crate) fn try_commit<R, E>(self, run: impl FnOnce() -> Result<R, E>) -> Result<R, E>
    where
        E: fmt::Display,
    {
        let started = Instant::now();
        let result = run();
        match &result {
            Ok(_) => self.emit("committed", started.elapsed(), None),
            Err(error) => self.emit("failed", started.elapsed(), Some(error)),
        }
        result
    }

    fn emit(&self, outcome: &'static str, duration: Duration, error: Option<&dyn fmt::Display>) {
        if self.records.is_empty() {
            self.emit_one(outcome, duration, error, None, 0);
        } else {
            for (index, record) in self.records.iter().enumerate() {
                self.emit_one(outcome, duration, error, Some(record), index);
            }
        }
    }

    fn emit_one(
        &self,
        outcome: &'static str,
        duration: Duration,
        error: Option<&dyn fmt::Display>,
        record: Option<RecordSlot<'_>>,
        record_index: usize,
    ) {
        let duration_ms = duration_ms(duration);
        let record_count = self.records.len();
        let parent = self.parent;
        let error_text = error.map(|error| error.to_string());
        match (record, error) {
            (Some(record), Some(error)) => event!(
                target: TARGET,
                Level::WARN,
                event = "typestate_transition",
                role = %T::ROLE,
                pipeline = %T::PIPELINE,
                phase = %self.stage,
                authority = %T::AUTHORITY,
                transition = T::LABEL,
                outcome,
                duration_ms,
                campaign_id = %parent.campaign_id(),
                parent_id = %parent.parent_id(),
                node_id = %parent.node_id(),
                generation = parent.generation(),
                branch_id = %parent.branch_id(),
                record_access = %record.access,
                record_kind = %record.record.kind(),
                record_path = %record.record.path().display(),
                record_index,
                record_count,
                error = %error,
                "prototype1 typestate transition failed"
            ),
            (Some(record), None) => event!(
                target: TARGET,
                Level::INFO,
                event = "typestate_transition",
                role = %T::ROLE,
                pipeline = %T::PIPELINE,
                phase = %self.stage,
                authority = %T::AUTHORITY,
                transition = T::LABEL,
                outcome,
                duration_ms,
                campaign_id = %parent.campaign_id(),
                parent_id = %parent.parent_id(),
                node_id = %parent.node_id(),
                generation = parent.generation(),
                branch_id = %parent.branch_id(),
                record_access = %record.access,
                record_kind = %record.record.kind(),
                record_path = %record.record.path().display(),
                record_index,
                record_count,
                "prototype1 typestate transition committed"
            ),
            (None, Some(error)) => event!(
                target: TARGET,
                Level::WARN,
                event = "typestate_transition",
                role = %T::ROLE,
                pipeline = %T::PIPELINE,
                phase = %self.stage,
                authority = %T::AUTHORITY,
                transition = T::LABEL,
                outcome,
                duration_ms,
                campaign_id = %parent.campaign_id(),
                parent_id = %parent.parent_id(),
                node_id = %parent.node_id(),
                generation = parent.generation(),
                branch_id = %parent.branch_id(),
                record_count,
                error = %error,
                "prototype1 typestate transition failed"
            ),
            (None, None) => event!(
                target: TARGET,
                Level::INFO,
                event = "typestate_transition",
                role = %T::ROLE,
                pipeline = %T::PIPELINE,
                phase = %self.stage,
                authority = %T::AUTHORITY,
                transition = T::LABEL,
                outcome,
                duration_ms,
                campaign_id = %parent.campaign_id(),
                parent_id = %parent.parent_id(),
                node_id = %parent.node_id(),
                generation = parent.generation(),
                branch_id = %parent.branch_id(),
                record_count,
                "prototype1 typestate transition committed"
            ),
        }
        self.emit_eval_trace(outcome, duration_ms, error_text, record, record_index);
    }

    fn emit_eval_trace(
        &self,
        outcome: &'static str,
        duration_ms: u64,
        error: Option<String>,
        record: Option<RecordSlot<'_>>,
        record_index: usize,
    ) {
        let parent = self.parent;
        let level = if error.is_some() { "WARN" } else { "INFO" };
        let (record_access, record_kind, record_path) = match record {
            Some(record) => (
                Some(record.access.to_string()),
                Some(record.record.kind().to_string()),
                Some(record.record.path().display().to_string()),
            ),
            None => (None, None, None),
        };
        mirror_trace_event(TraceEventEvidence {
            campaign_id: Some(parent.campaign_id().clone()),
            parent_id: Some(parent.parent_id().to_string()),
            runtime_id: None,
            node_id: Some(parent.node_id().to_string()),
            generation: Some(i64::from(parent.generation())),
            branch_id: Some(parent.branch_id().to_string()),
            role: Some(T::ROLE.to_string()),
            pipeline: Some(T::PIPELINE.to_string()),
            stage: Some(self.stage.to_string()),
            authority: Some(T::AUTHORITY.to_string()),
            transition: Some(T::LABEL.to_string()),
            event_name: Some("typestate_transition".to_string()),
            span_name: None,
            target: TARGET.to_string(),
            level: level.to_string(),
            outcome: Some(outcome.to_string()),
            duration_ms: Some(u64_to_i64(duration_ms)),
            record_access,
            record_kind,
            record_path,
            record_index: record.map(|_| usize_to_i64(record_index)),
            record_count: Some(usize_to_i64(self.records.len())),
            program: None,
            exit_code: None,
            error,
            source_log_ref: None,
            source_event_index: None,
            recorded_at: Some(chrono::Utc::now().to_rfc3339()),
        });
    }
}

#[derive(Debug)]
pub(crate) struct Step {
    span: Span,
    started: Instant,
}

impl Step {
    pub(crate) fn start(span: Span) -> Self {
        Self {
            span,
            started: Instant::now(),
        }
    }

    pub(crate) fn success(self) {
        self.finish("succeeded");
    }

    pub(crate) fn rejected(self) {
        self.finish("rejected");
    }

    pub(crate) fn removed(self) {
        self.finish("removed");
    }

    pub(crate) fn missing(self) {
        self.finish("missing");
    }

    pub(crate) fn timed_out(self) {
        self.finish("timed_out");
    }

    pub(crate) fn exited_before_ready(self) {
        self.finish("exited_before_ready");
    }

    pub(crate) fn fail(self, phase: &'static str, error: impl fmt::Display) {
        let duration_ms = duration_ms(self.started.elapsed());
        let error = error.to_string();
        event!(
            target: TARGET,
            parent: &self.span,
            Level::WARN,
            outcome = "failed",
            phase,
            duration_ms,
            error = %error,
            "prototype1 step failed"
        );
        self.emit_eval_trace("failed", "WARN", duration_ms, Some(phase), Some(error));
    }

    fn finish(self, outcome: &'static str) {
        let duration_ms = duration_ms(self.started.elapsed());
        event!(
            target: TARGET,
            parent: &self.span,
            Level::INFO,
            outcome,
            duration_ms,
            "prototype1 step finished"
        );
        self.emit_eval_trace(outcome, "INFO", duration_ms, None, None);
    }

    fn emit_eval_trace(
        &self,
        outcome: &'static str,
        level: &'static str,
        duration_ms: u64,
        phase: Option<&'static str>,
        error: Option<String>,
    ) {
        mirror_trace_event(TraceEventEvidence {
            campaign_id: None,
            parent_id: None,
            runtime_id: None,
            node_id: None,
            generation: None,
            branch_id: None,
            role: None,
            pipeline: None,
            stage: phase.map(str::to_string),
            authority: None,
            transition: None,
            event_name: Some("prototype1_step".to_string()),
            span_name: None,
            target: TARGET.to_string(),
            level: level.to_string(),
            outcome: Some(outcome.to_string()),
            duration_ms: Some(u64_to_i64(duration_ms)),
            record_access: None,
            record_kind: None,
            record_path: None,
            record_index: None,
            record_count: None,
            program: None,
            exit_code: None,
            error,
            source_log_ref: None,
            source_event_index: None,
            recorded_at: Some(chrono::Utc::now().to_rfc3339()),
        });
    }
}

pub(crate) fn command_output<F>(span: Span, program: &'static str, run: F) -> io::Result<Output>
where
    F: FnOnce() -> io::Result<Output>,
{
    let started = Instant::now();
    let result = {
        let _entered = span.enter();
        run()
    };

    match result {
        Ok(output) => {
            let outcome = if output.status.success() {
                "succeeded"
            } else {
                "rejected"
            };
            event!(
                target: TARGET,
                parent: &span,
                Level::INFO,
                outcome,
                duration_ms = duration_ms(started.elapsed()),
                program,
                exit_code = ?output.status.code(),
                stdout_excerpt = ?excerpt(&output.stdout),
                stderr_excerpt = ?excerpt(&output.stderr),
                "prototype1 command finished"
            );
            Ok(output)
        }
        Err(error) => {
            event!(
                target: TARGET,
                parent: &span,
                Level::WARN,
                outcome = "failed",
                duration_ms = duration_ms(started.elapsed()),
                program,
                error = %error,
                "prototype1 command failed"
            );
            Err(error)
        }
    }
}

pub(crate) fn io_result<T, F>(span: Span, phase: &'static str, run: F) -> io::Result<T>
where
    F: FnOnce() -> io::Result<T>,
{
    let started = Instant::now();
    let result = {
        let _entered = span.enter();
        run()
    };

    match result {
        Ok(value) => {
            event!(
                target: TARGET,
                parent: &span,
                Level::INFO,
                outcome = "succeeded",
                duration_ms = duration_ms(started.elapsed()),
                phase,
                "prototype1 io step finished"
            );
            Ok(value)
        }
        Err(error) => {
            event!(
                target: TARGET,
                parent: &span,
                Level::WARN,
                outcome = "failed",
                duration_ms = duration_ms(started.elapsed()),
                phase,
                error = %error,
                "prototype1 io step failed"
            );
            Err(error)
        }
    }
}

pub(crate) fn result<T, E, F>(span: Span, run: F) -> Result<T, E>
where
    E: fmt::Display,
    F: FnOnce() -> Result<T, E>,
{
    let started = Instant::now();
    let result = {
        let _entered = span.enter();
        run()
    };
    finish_result(span, started, result)
}

pub(crate) async fn future<T, E, Fut>(span: Span, future: Fut) -> Result<T, E>
where
    E: fmt::Display,
    Fut: Future<Output = Result<T, E>>,
{
    let started = Instant::now();
    let result = future.instrument(span.clone()).await;
    finish_result(span, started, result)
}

fn finish_result<T, E>(span: Span, started: Instant, result: Result<T, E>) -> Result<T, E>
where
    E: fmt::Display,
{
    match result {
        Ok(value) => {
            event!(
                target: TARGET,
                parent: &span,
                Level::INFO,
                outcome = "succeeded",
                duration_ms = duration_ms(started.elapsed()),
                "prototype1 result step finished"
            );
            Ok(value)
        }
        Err(error) => {
            event!(
                target: TARGET,
                parent: &span,
                Level::WARN,
                outcome = "failed",
                duration_ms = duration_ms(started.elapsed()),
                error = %error,
                "prototype1 result step failed"
            );
            Err(error)
        }
    }
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn u64_to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn usize_to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn excerpt(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut excerpt = trimmed.chars().take(4000).collect::<String>();
    if trimmed.chars().count() > 4000 {
        excerpt.push_str("...");
    }
    Some(excerpt)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::Mutex as TestMutex;

    use ploke_records::identity::ParentIdentityRecord;

    use super::*;
    use crate::cli::prototype1_state::eval_store::load_owner_eval_database;
    use crate::cli::prototype1_state::identity::PARENT_IDENTITY_SCHEMA_VERSION;

    static OBSERVE_TEST_LOCK: TestMutex<()> = TestMutex::new(());

    struct TestObservedTransition;

    impl Transition for TestObservedTransition {
        type From = ();
        type To = ();
    }

    impl ObservedTransition for TestObservedTransition {
        const ROLE: Role = Role::Parent;
        const PIPELINE: Pipeline = Pipeline::ChildPlanAuthority;
        const STAGE: Stage = Stage::TypestateTransition;
        const AUTHORITY: Authority = Authority::ParentBroadcastChannel;
        const LABEL: &'static str = "test_transition";
    }

    #[test]
    fn prototype1_observe_transition_builder_mirrors_trace_row_to_eval_db() {
        let _lock = OBSERVE_TEST_LOCK.lock().expect("observe test lock");
        let tmp = tempfile::tempdir().expect("tmp");
        let db_path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
        let parent = parent_identity();
        let _guard = scoped_eval_trace_sink(Some(EvalTraceSinkConfig {
            campaign_id: CampaignId::from("campaign"),
            db_path: db_path.clone(),
        }));

        transition::<TestObservedTransition>(&parent).commit(|| ());

        let rows = query_trace_rows(&db_path);
        assert_eq!(rows.rows.len(), 1);
        let row = rows.row_refs().next().expect("trace row");
        assert_eq!(
            row.get::<String>("event_name").expect("event"),
            "typestate_transition"
        );
        assert_eq!(row.get::<String>("outcome").expect("outcome"), "committed");
        assert_eq!(
            row.get::<String>("campaign_id").expect("campaign"),
            "campaign"
        );
        assert_eq!(row.get::<String>("parent_id").expect("parent"), "parent");
        assert_eq!(row.get::<String>("node_id").expect("node"), "parent");
        assert_eq!(row.get::<String>("role").expect("role"), "parent");
        assert_eq!(
            row.get::<String>("transition").expect("transition"),
            "test_transition"
        );
        assert_eq!(row.get::<i64>("source_event_index").expect("index"), 0);
    }

    #[test]
    fn prototype1_observe_step_mirrors_trace_row_to_eval_db() {
        let _lock = OBSERVE_TEST_LOCK.lock().expect("observe test lock");
        let tmp = tempfile::tempdir().expect("tmp");
        let db_path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
        let _guard = scoped_eval_trace_sink(Some(EvalTraceSinkConfig {
            campaign_id: CampaignId::from("campaign"),
            db_path: db_path.clone(),
        }));

        Step::start(span!("prototype1.test.step")).success();

        let rows = query_trace_rows(&db_path);
        assert_eq!(rows.rows.len(), 1);
        let row = rows.row_refs().next().expect("trace row");
        assert_eq!(
            row.get::<String>("event_name").expect("event"),
            "prototype1_step"
        );
        assert_eq!(row.get::<String>("outcome").expect("outcome"), "succeeded");
        assert_eq!(
            row.get::<String>("campaign_id").expect("campaign"),
            "campaign"
        );
        assert_eq!(row.get::<i64>("source_event_index").expect("index"), 0);
    }

    fn query_trace_rows(db_path: &Path) -> ploke_db::QueryResult {
        let db = load_owner_eval_database(db_path).expect("load owner db");
        db.raw_query_params(
            r#"
?[event_name, outcome, campaign_id, parent_id, node_id, role, transition, source_event_index] :=
    *eval_trace_event {
        event_name,
        outcome,
        campaign_id,
        parent_id,
        node_id,
        role,
        transition,
        source_event_index
    }
"#,
            BTreeMap::new(),
        )
        .expect("query trace rows")
    }

    fn parent_identity() -> ParentIdentity {
        ParentIdentity::from_record_for_test(ParentIdentityRecord {
            schema_version: PARENT_IDENTITY_SCHEMA_VERSION.to_string(),
            campaign_id: CampaignId::from("campaign"),
            parent_id: "parent".to_string(),
            node_id: "parent".to_string(),
            generation: 0,
            instance_id: Some("instance".to_string()),
            previous_parent_id: None,
            parent_node_id: None,
            branch_id: "branch-parent".to_string(),
            artifact_branch: Some("artifact-parent".to_string()),
            created_at: "2026-06-22T00:00:00Z".to_string(),
        })
    }
}
