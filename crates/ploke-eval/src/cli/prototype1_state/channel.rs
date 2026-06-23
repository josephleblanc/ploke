#![allow(dead_code)] // Staged for parent/child channel migration; some compatibility projections remain.

//! Role-indexed parent/child runtime channel.
//!
//! This module separates protocol authority from transport mechanics.
//! `Channel<Parent<S>, T>` and `Channel<Child<S>, T>` use the existing
//! role/state carriers as authority tokens, while `T: Transport` decides how
//! bytes move between runtimes.
//!
//! The design target is transport-agnostic parent/child communication: today's
//! transport is a pair of per-runtime file buffers, but the same role/state
//! contract must be able to run over a socket, websocket, queue, object-store
//! stream, or VM boundary. After the bootstrap/invocation step gives a runtime
//! its role, all parent/child messages that can affect parent selection,
//! successor promotion, or sealed History must cross a dedicated channel for
//! that child/runtime. Local files, git commits, branch registries, journals,
//! and result sidecars may remain as durable projections or child-owned stores,
//! but the parent must learn about selection-grade child facts through channel
//! payloads or channel-carried verifiable references to those stores.
//!
//! This rule is what lets local file-buffer execution scale later to cloud
//! execution: each child VM owns its own channel endpoint and can retain or git
//! commit its local evidence, while the parent consumes channel messages and
//! writes parent-owned projections. A raw path in a shared filesystem is not a
//! communication authority by itself; it becomes usable by the parent only when
//! the admitted channel message names it with enough identity/hash/tree context
//! for replay and later checkout/cherry-pick validation.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    marker::PhantomData,
    path::{Path, PathBuf},
};

use ploke_records::ids::CampaignId;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use super::{
    child::{self, Child},
    cli_facing::Prototype1TreatmentEvidence,
    eval_store,
    event::{RecordedAt, RuntimeId},
    invocation::{SuccessorCompletionRecord, SuccessorReadyRecord},
    parent::{self, Parent},
};
use crate::intervention::Prototype1RunnerResult;

const SCHEMA_VERSION: &str = "prototype1-runtime-channel.v1";

/// Message-level capability markers.
pub(crate) mod message {
    /// Child may send `ToParent::Ready`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Ready {}

    /// Child may send `ToParent::Evaluating`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Evaluating {}

    /// Child may send `ToParent::ResultWritten`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ResultWritten {}

    /// Child may send `ToParent::Result`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Result {}

    /// Child may send `ToParent::Failed`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Failed {}

    /// Child may send `ToParent::Exited`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Exited {}

    /// Parent may send `ToChild::Cancel`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Cancel {}
}

/// Directed stream capability markers.
pub(crate) mod stream {
    /// Parent may receive messages written by the child.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum FromChild {}

    /// Child may receive messages written by the parent.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum FromParent {}
}

/// State marker `S` may send message marker `M` through its role channel.
pub(crate) trait CanSend<M> {}

/// State marker `S` may receive message marker `M` through its role channel.
pub(crate) trait CanRecv<M> {}

/// Child state that may report non-normal termination evidence.
pub(crate) trait ChildCanTerminate {}

impl CanSend<message::Ready> for child::Starting {}
impl CanSend<message::Evaluating> for child::Ready {}
impl CanSend<message::ResultWritten> for child::Evaluating {}
impl CanSend<message::Result> for child::Evaluating {}
impl ChildCanTerminate for child::Starting {}
impl ChildCanTerminate for child::Ready {}
impl ChildCanTerminate for child::Evaluating {}
impl<S> CanSend<message::Failed> for S where S: ChildCanTerminate {}
impl<S> CanSend<message::Exited> for S where S: ChildCanTerminate {}

impl<S, M> CanSend<M> for Child<S> where S: CanSend<M> {}
impl<S, M> CanRecv<M> for Child<S> where S: CanRecv<M> {}
impl<S, M> CanSend<M> for Parent<S> where S: CanSend<M> {}
impl<S, M> CanRecv<M> for Parent<S> where S: CanRecv<M> {}

impl CanRecv<stream::FromParent> for child::Ready {}
impl CanRecv<stream::FromParent> for child::Evaluating {}

impl CanSend<message::Cancel> for parent::Selectable {}
impl CanRecv<stream::FromChild> for parent::Selectable {}
impl CanRecv<stream::FromChild> for parent::Retired {}
impl CanRecv<stream::FromChild> for super::c3::C3 {}
impl CanRecv<stream::FromChild> for super::c3::C4 {}

/// Direction of one serialized channel envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Direction {
    /// Parent wrote this message for the child.
    ParentToChild,
    /// Child wrote this message for the parent.
    ChildToParent,
}

/// Cursor into a transport endpoint.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct Cursor {
    offset: u64,
}

impl Cursor {
    /// Initial cursor before any records are read.
    pub(crate) const fn start() -> Self {
        Self { offset: 0 }
    }

    /// Byte offset after the last complete record observed by the caller.
    pub(crate) const fn offset(self) -> u64 {
        self.offset
    }
}

/// Transport receipt for an appended envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Receipt {
    endpoint: PathBuf,
    cursor: Cursor,
    bytes_written: usize,
}

impl Receipt {
    /// Endpoint that accepted the envelope.
    pub(crate) fn endpoint(&self) -> &Path {
        &self.endpoint
    }

    /// Cursor immediately after the appended record.
    pub(crate) fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Number of serialized bytes written, not including transport framing.
    pub(crate) fn bytes_written(&self) -> usize {
        self.bytes_written
    }
}

/// Backend mechanics for moving channel envelope bytes.
pub(crate) trait Transport {
    /// Transport-specific failure.
    type Error;

    /// Append one serialized envelope to one endpoint.
    fn append(&self, endpoint: &Endpoint, bytes: &[u8]) -> Result<Receipt, Self::Error>;

    /// Read complete serialized envelopes after `cursor`.
    fn read_since(
        &self,
        endpoint: &Endpoint,
        cursor: Cursor,
    ) -> Result<(Cursor, Vec<Vec<u8>>), Self::Error>;
}

/// Concrete endpoint for one directed side of a runtime channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Endpoint {
    campaign_id: CampaignId,
    node_id: String,
    runtime_id: RuntimeId,
    direction: Direction,
    path: PathBuf,
}

impl Endpoint {
    /// Campaign that owns this runtime attempt.
    pub(crate) fn campaign_id(&self) -> &CampaignId {
        &self.campaign_id
    }

    /// Node evaluated by this runtime attempt.
    pub(crate) fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Concrete runtime attempt.
    pub(crate) fn runtime_id(&self) -> RuntimeId {
        self.runtime_id
    }

    /// Message direction for this endpoint.
    pub(crate) fn direction(&self) -> Direction {
        self.direction
    }

    /// Concrete transport address.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

/// Both directed endpoints for one parent/child runtime channel.
///
/// An `Endpoints` value is scoped to one concrete `(campaign, node, runtime)`
/// tuple. Fanout must allocate distinct endpoints per child runtime so that
/// child outputs do not contend on shared mutable parent files. The file paths
/// here are the current transport projection; a non-file transport should
/// preserve the same identity and direction split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Endpoints {
    root: PathBuf,
    campaign_id: CampaignId,
    node_id: String,
    runtime_id: RuntimeId,
}

impl Endpoints {
    /// Construct endpoints rooted at `nodes/<node-id>/channels/<runtime-id>/`.
    pub(crate) fn new(
        root: PathBuf,
        campaign_id: CampaignId,
        node_id: String,
        runtime_id: RuntimeId,
    ) -> Self {
        Self {
            root,
            campaign_id,
            node_id,
            runtime_id,
        }
    }

    /// Root directory for this channel projection.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// Parent-write, child-read endpoint.
    pub(crate) fn parent_to_child(&self) -> Endpoint {
        self.endpoint(Direction::ParentToChild, "parent-to-child.jsonl")
    }

    /// Child-write, parent-read endpoint.
    pub(crate) fn child_to_parent(&self) -> Endpoint {
        self.endpoint(Direction::ChildToParent, "child-to-parent.jsonl")
    }

    fn endpoint(&self, direction: Direction, filename: &str) -> Endpoint {
        Endpoint {
            campaign_id: self.campaign_id.clone(),
            node_id: self.node_id.clone(),
            runtime_id: self.runtime_id,
            direction,
            path: self.root.join(filename),
        }
    }
}

/// Serialized channel record with runtime identity and payload hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Envelope<M> {
    schema_version: String,
    direction: Direction,
    campaign_id: CampaignId,
    node_id: String,
    runtime_id: RuntimeId,
    message_id: Uuid,
    recorded_at: RecordedAt,
    body_hash: String,
    body: M,
}

impl<M> Envelope<M> {
    /// Message direction carried by this envelope.
    pub(crate) fn direction(&self) -> Direction {
        self.direction
    }

    /// Concrete runtime attempt named by this envelope.
    pub(crate) fn runtime_id(&self) -> RuntimeId {
        self.runtime_id
    }

    /// Message payload.
    pub(crate) fn body(&self) -> &M {
        &self.body
    }

    fn validate_endpoint(&self, endpoint: &Endpoint) -> Result<(), EnvelopeError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(EnvelopeError::Schema {
                expected: SCHEMA_VERSION,
                actual: self.schema_version.clone(),
            });
        }
        if self.direction != endpoint.direction {
            return Err(EnvelopeError::Direction {
                expected: endpoint.direction,
                actual: self.direction,
            });
        }
        if self.campaign_id != endpoint.campaign_id {
            return Err(EnvelopeError::Campaign {
                expected: endpoint.campaign_id.to_string(),
                actual: self.campaign_id.to_string(),
            });
        }
        if self.node_id != endpoint.node_id {
            return Err(EnvelopeError::Node {
                expected: endpoint.node_id.clone(),
                actual: self.node_id.clone(),
            });
        }
        if self.runtime_id != endpoint.runtime_id {
            return Err(EnvelopeError::Runtime {
                expected: endpoint.runtime_id,
                actual: self.runtime_id,
            });
        }
        Ok(())
    }
}

impl<M> Envelope<M>
where
    M: Serialize,
{
    fn validate_body_hash(&self) -> Result<(), EnvelopeError> {
        let actual = body_hash(&self.body).map_err(EnvelopeError::BodyHashEncode)?;
        if self.body_hash != actual {
            return Err(EnvelopeError::BodyHash {
                expected: self.body_hash.clone(),
                actual,
            });
        }
        Ok(())
    }
}

impl<M> Envelope<M>
where
    M: Serialize,
{
    fn new(endpoint: &Endpoint, body: M) -> Result<Self, serde_json::Error> {
        Ok(Self {
            schema_version: SCHEMA_VERSION.to_string(),
            direction: endpoint.direction,
            campaign_id: endpoint.campaign_id.clone(),
            node_id: endpoint.node_id.clone(),
            runtime_id: endpoint.runtime_id,
            message_id: Uuid::new_v4(),
            recorded_at: RecordedAt::now(),
            body_hash: body_hash(&body)?,
            body,
        })
    }
}

fn body_hash<M>(body: &M) -> Result<String, serde_json::Error>
where
    M: Serialize,
{
    let bytes = serde_json::to_vec(body)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

/// Parent-to-child protocol messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToChild {
    /// Parent asks this child runtime to stop.
    Cancel { reason: String },
}

/// Child-to-parent protocol messages.
///
/// Messages in this family are the parent-visible boundary for child facts that
/// may later influence selection or History. A child may keep richer local
/// records and commit them to its own checkout/tree, but any parent decision
/// that depends on those records must be based on a `ToParent` payload or a
/// `ToParent`-carried reference that can locate and verify the child-owned
/// data later.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToParent {
    /// Child runtime has started and can be observed.
    Ready,
    /// Child runtime entered evaluation.
    Evaluating,
    /// Child completed execution and returned its terminal payload.
    ///
    /// This is the selection-grade terminal message. For a successful child it
    /// must either carry the treatment evidence directly or carry enough typed
    /// references to recover that evidence from the child's durable store. The
    /// parent may write `runner-result.json`, branch evaluations, journals, or
    /// monitor projections after consuming this message, but those projections
    /// are not substitutes for the channel boundary.
    Result {
        /// Attempt-scoped runner result produced by the child runtime.
        runner_result: Prototype1RunnerResult,
        /// Treatment evidence when the child completed execution.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        treatment: Option<Prototype1TreatmentEvidence>,
    },
    /// Child persisted its attempt-scoped runner result.
    ///
    /// Compatibility projection for callers that still exchange result paths.
    /// This notification is useful for reconstruction and operator diagnostics,
    /// but it must not by itself make a child selectable or History-sealable.
    /// New execution handoff should prefer `Result` carrying the runner result
    /// plus treatment evidence or verifiable child-store references.
    ResultWritten { runner_result_path: PathBuf },
    /// Successor runtime acknowledged bootstrap.
    SuccessorReady { record: SuccessorReadyRecord },
    /// Successor runtime completed its bounded controller turn.
    SuccessorCompletion { record: SuccessorCompletionRecord },
    /// Child failed before writing a normal terminal result.
    Failed { detail: String },
    /// Child process exited.
    Exited { status: Option<i32> },
}

/// Role-indexed parent/child channel.
///
/// `Channel<R, T>` is the protocol authority; `T` is only the byte-moving
/// backend. Code that needs parent/child evidence should depend on the
/// role-shaped channel, not on whether the current transport is JSONL files,
/// sockets, or something else.
///
/// Design constraint for Prototype 1 loop safety: any datum used by the parent
/// to compare children, select/promote a successor, or seal History must be
/// received over this child's channel, either inline or as a verifiable
/// reference to child-owned durable storage such as a git commit/tree plus
/// content hashes. Parent-owned files may cache or project those facts, but a
/// projection that was not derived from an admitted channel message is not
/// sufficient authority for selection or History.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Channel<R, T> {
    transport: T,
    endpoints: Endpoints,
    _role: PhantomData<R>,
    _private: Private,
}

impl<R, T> Channel<R, T> {
    fn new(endpoints: Endpoints, transport: T) -> Self {
        Self {
            transport,
            endpoints,
            _role: PhantomData,
            _private: Private,
        }
    }

    /// Endpoints shared by both sides of this runtime channel.
    pub(crate) fn endpoints(&self) -> &Endpoints {
        &self.endpoints
    }

    /// Construct a channel side from a real role/state carrier.
    pub(crate) fn for_role(_role: &R, endpoints: Endpoints, transport: T) -> Self {
        Self::new(endpoints, transport)
    }

    fn cast<U>(self) -> Channel<U, T> {
        Channel {
            transport: self.transport,
            endpoints: self.endpoints,
            _role: PhantomData,
            _private: Private,
        }
    }
}

impl<S, T> Channel<Parent<S>, T> {
    /// Construct a parent-side channel only from a real parent role/state.
    pub(crate) fn for_parent(_parent: &Parent<S>, endpoints: Endpoints, transport: T) -> Self {
        Self::new(endpoints, transport)
    }
}

impl<S, T> Channel<Child<S>, T> {
    /// Construct a child-side channel only from a real child role/state.
    pub(crate) fn for_child(_child: &Child<S>, endpoints: Endpoints, transport: T) -> Self {
        Self::new(endpoints, transport)
    }
}

impl<S, T> Channel<Parent<S>, T>
where
    S: CanSend<message::Cancel>,
    T: Transport,
{
    /// Ask this child runtime to stop.
    pub(crate) fn send_cancel(
        &self,
        reason: impl Into<String>,
    ) -> Result<Receipt, ChannelError<T::Error>> {
        self.write(
            &self.endpoints.parent_to_child(),
            ToChild::Cancel {
                reason: reason.into(),
            },
        )
    }
}

impl<R, T> Channel<R, T>
where
    R: CanRecv<stream::FromChild>,
    T: Transport,
{
    /// Receive messages written by this child runtime.
    pub(crate) fn recv_from_child(
        &self,
        cursor: Cursor,
    ) -> Result<(Cursor, Vec<Envelope<ToParent>>), ChannelError<T::Error>> {
        self.read(&self.endpoints.child_to_parent(), cursor)
    }
}

impl<S, T> Channel<Child<S>, T>
where
    S: CanRecv<stream::FromParent>,
    T: Transport,
{
    /// Receive messages written by the parent for this child runtime.
    pub(crate) fn recv_from_parent(
        &self,
        cursor: Cursor,
    ) -> Result<(Cursor, Vec<Envelope<ToChild>>), ChannelError<T::Error>> {
        self.read(&self.endpoints.parent_to_child(), cursor)
    }
}

impl<S, T> Channel<Child<S>, T>
where
    S: CanSend<message::Ready>,
    T: Transport,
{
    /// Send `Child<Ready>` evidence and advance the channel state.
    pub(crate) fn send_ready(
        self,
    ) -> Result<(Channel<Child<child::Ready>, T>, Receipt), ChannelError<T::Error>> {
        let receipt = self.write_child_message(ToParent::Ready, "ready")?;
        Ok((self.cast(), receipt))
    }
}

impl<S, T> Channel<Child<S>, T>
where
    S: CanSend<message::Evaluating>,
    T: Transport,
{
    /// Send `Child<Evaluating>` evidence and advance the channel state.
    pub(crate) fn send_evaluating(
        self,
    ) -> Result<(Channel<Child<child::Evaluating>, T>, Receipt), ChannelError<T::Error>> {
        let receipt = self.write_child_message(ToParent::Evaluating, "evaluating")?;
        Ok((self.cast(), receipt))
    }
}

impl<S, T> Channel<Child<S>, T>
where
    S: CanSend<message::Result>,
    T: Transport,
{
    /// Send direct terminal child execution payload and advance the channel state.
    pub(crate) fn send_terminal_result(
        self,
        runner_result: Prototype1RunnerResult,
        treatment: Option<Prototype1TreatmentEvidence>,
    ) -> Result<(Channel<Child<child::ResultWritten>, T>, Receipt), ChannelError<T::Error>> {
        let receipt = self.write(
            &self.endpoints.child_to_parent(),
            ToParent::Result {
                runner_result,
                treatment,
            },
        )?;
        Ok((self.cast(), receipt))
    }
}

impl<S, T> Channel<Child<S>, T>
where
    S: CanSend<message::ResultWritten>,
    T: Transport,
{
    /// Send path-based `Child<ResultWritten>` compatibility evidence.
    ///
    /// This is retained for compatibility with the artifact-backed handoff.
    /// It must not be used as terminal lifecycle authority; new execution
    /// handoff should use `send_terminal_result`.
    pub(crate) fn send_result_written(
        self,
        runner_result_path: PathBuf,
    ) -> Result<(Channel<Child<child::ResultWritten>, T>, Receipt), ChannelError<T::Error>> {
        let receipt = self.write(
            &self.endpoints.child_to_parent(),
            ToParent::ResultWritten { runner_result_path },
        )?;
        Ok((self.cast(), receipt))
    }
}

impl<S, T> Channel<Child<S>, T>
where
    S: CanSend<message::Failed>,
    T: Transport,
{
    /// Send non-terminal child failure evidence.
    pub(crate) fn send_failed(
        &self,
        detail: impl Into<String>,
    ) -> Result<Receipt, ChannelError<T::Error>> {
        self.write(
            &self.endpoints.child_to_parent(),
            ToParent::Failed {
                detail: detail.into(),
            },
        )
    }
}

impl<S, T> Channel<Child<S>, T>
where
    S: CanSend<message::Exited>,
    T: Transport,
{
    /// Send child process exit evidence.
    pub(crate) fn send_exited(
        &self,
        status: Option<i32>,
    ) -> Result<Receipt, ChannelError<T::Error>> {
        self.write(
            &self.endpoints.child_to_parent(),
            ToParent::Exited { status },
        )
    }
}

impl<R, T> Channel<R, T>
where
    T: Transport,
{
    /// Send successor bootstrap acknowledgement through the runtime channel.
    pub(crate) fn send_successor_ready(
        &self,
        record: SuccessorReadyRecord,
    ) -> Result<Receipt, ChannelError<T::Error>> {
        self.write(
            &self.endpoints.child_to_parent(),
            ToParent::SuccessorReady { record },
        )
    }

    /// Send successor bounded-turn completion through the runtime channel.
    pub(crate) fn send_successor_completion(
        &self,
        record: SuccessorCompletionRecord,
    ) -> Result<Receipt, ChannelError<T::Error>> {
        self.write(
            &self.endpoints.child_to_parent(),
            ToParent::SuccessorCompletion { record },
        )
    }

    fn write<M>(&self, endpoint: &Endpoint, message: M) -> Result<Receipt, ChannelError<T::Error>>
    where
        M: Serialize,
    {
        let envelope = Envelope::new(endpoint, message).map_err(ChannelError::Encode)?;
        let bytes = serde_json::to_vec(&envelope).map_err(ChannelError::Encode)?;
        self.transport
            .append(endpoint, &bytes)
            .map_err(ChannelError::Transport)
    }

    fn write_child_message(
        &self,
        message: ToParent,
        message_kind: &'static str,
    ) -> Result<Receipt, ChannelError<T::Error>> {
        let endpoint = self.endpoints.child_to_parent();
        let envelope = Envelope::new(&endpoint, message).map_err(ChannelError::Encode)?;
        let bytes = serde_json::to_vec(&envelope).map_err(ChannelError::Encode)?;
        let receipt = self
            .transport
            .append(&endpoint, &bytes)
            .map_err(ChannelError::Transport)?;
        mirror_channel_message(&endpoint, &envelope, &bytes, &receipt, message_kind)
            .map_err(ChannelError::EvalStore)?;
        Ok(receipt)
    }

    fn read<M>(
        &self,
        endpoint: &Endpoint,
        cursor: Cursor,
    ) -> Result<(Cursor, Vec<Envelope<M>>), ChannelError<T::Error>>
    where
        M: DeserializeOwned + Serialize,
    {
        let (cursor, records) = self
            .transport
            .read_since(endpoint, cursor)
            .map_err(ChannelError::Transport)?;
        let envelopes = records
            .into_iter()
            .map(|record| {
                let envelope: Envelope<M> =
                    serde_json::from_slice(&record).map_err(ChannelError::Decode)?;
                envelope
                    .validate_endpoint(endpoint)
                    .map_err(ChannelError::Envelope)?;
                envelope
                    .validate_body_hash()
                    .map_err(ChannelError::Envelope)?;
                Ok(envelope)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((cursor, envelopes))
    }
}

fn mirror_channel_message(
    endpoint: &Endpoint,
    envelope: &Envelope<ToParent>,
    bytes: &[u8],
    receipt: &Receipt,
    message_kind: &'static str,
) -> Result<(), eval_store::EvalStoreError> {
    let db_path = match eval_store::owner_eval_db_file_for_record_path(receipt.endpoint()) {
        Ok(path) => path,
        Err(eval_store::EvalStoreError::Validation { .. }) => return Ok(()),
        Err(source) => return Err(source),
    };
    if !db_path.is_file() {
        return Ok(());
    }
    eval_store::write_channel_message_to_owner_db(
        &db_path,
        eval_store::ChannelMessageEvidence {
            campaign_id: endpoint.campaign_id.clone(),
            node_id: endpoint.node_id.clone(),
            runtime_id: endpoint.runtime_id.to_string(),
            direction: direction_label(endpoint.direction).to_string(),
            message_kind: message_kind.to_string(),
            message_id: envelope.message_id.to_string(),
            endpoint_path: receipt.endpoint().to_path_buf(),
            cursor_offset: receipt.cursor().offset() as i64,
            bytes_written: receipt.bytes_written() as i64,
            body_hash: envelope.body_hash.clone(),
            content_sha256: format!("{:x}", Sha256::digest(bytes)),
            recorded_at: envelope.recorded_at.0.to_string(),
        },
    )?;
    Ok(())
}

fn direction_label(direction: Direction) -> &'static str {
    match direction {
        Direction::ParentToChild => "parent_to_child",
        Direction::ChildToParent => "child_to_parent",
    }
}

/// Channel-level serialization, validation, or transport failure.
#[derive(Debug)]
pub(crate) enum ChannelError<E> {
    /// Failed to encode an envelope.
    Encode(serde_json::Error),
    /// Failed to decode an envelope.
    Decode(serde_json::Error),
    /// Decoded envelope did not match the endpoint being read.
    Envelope(EnvelopeError),
    /// Transport failed.
    Transport(E),
    /// Eval-store mirror failed after the transport accepted an envelope.
    EvalStore(eval_store::EvalStoreError),
}

/// Envelope identity mismatch.
#[derive(Debug, Error)]
pub(crate) enum EnvelopeError {
    /// Unsupported schema version.
    #[error("channel schema mismatch: expected {expected}, got {actual}")]
    Schema {
        /// Expected schema version.
        expected: &'static str,
        /// Actual schema version.
        actual: String,
    },
    /// Direction did not match the endpoint.
    #[error("channel direction mismatch: expected {expected:?}, got {actual:?}")]
    Direction {
        /// Expected direction.
        expected: Direction,
        /// Actual direction.
        actual: Direction,
    },
    /// Campaign did not match the endpoint.
    #[error("channel campaign mismatch: expected {expected}, got {actual}")]
    Campaign {
        /// Expected campaign id.
        expected: String,
        /// Actual campaign id.
        actual: String,
    },
    /// Node did not match the endpoint.
    #[error("channel node mismatch: expected {expected}, got {actual}")]
    Node {
        /// Expected node id.
        expected: String,
        /// Actual node id.
        actual: String,
    },
    /// Runtime did not match the endpoint.
    #[error("channel runtime mismatch: expected {expected}, got {actual}")]
    Runtime {
        /// Expected runtime id.
        expected: RuntimeId,
        /// Actual runtime id.
        actual: RuntimeId,
    },
    /// Body hash did not match the decoded payload.
    #[error("channel body hash mismatch: expected {expected}, got {actual}")]
    BodyHash {
        /// Expected body hash carried by the envelope.
        expected: String,
        /// Actual body hash computed from the decoded body.
        actual: String,
    },
    /// Could not compute the body hash during validation.
    #[error("failed to encode channel body for hash validation: {0}")]
    BodyHashEncode(#[source] serde_json::Error),
}

/// Filesystem JSONL transport for the parent/child channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct FileTransport;

impl Transport for FileTransport {
    type Error = FileTransportError;

    fn append(&self, endpoint: &Endpoint, bytes: &[u8]) -> Result<Receipt, Self::Error> {
        if let Some(parent) = endpoint.path.parent() {
            fs::create_dir_all(parent).map_err(FileTransportError::CreateDir)?;
        }

        let start = endpoint
            .path
            .metadata()
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&endpoint.path)
            .map_err(FileTransportError::Open)?;
        file.write_all(bytes).map_err(FileTransportError::Write)?;
        file.write_all(b"\n").map_err(FileTransportError::Write)?;
        let cursor = Cursor {
            offset: start + bytes.len() as u64 + 1,
        };

        Ok(Receipt {
            endpoint: endpoint.path.clone(),
            cursor,
            bytes_written: bytes.len(),
        })
    }

    fn read_since(
        &self,
        endpoint: &Endpoint,
        cursor: Cursor,
    ) -> Result<(Cursor, Vec<Vec<u8>>), Self::Error> {
        if !endpoint.path.exists() {
            return Ok((cursor, Vec::new()));
        }

        let mut file = File::open(&endpoint.path).map_err(FileTransportError::Open)?;
        file.seek(SeekFrom::Start(cursor.offset))
            .map_err(FileTransportError::Seek)?;
        let mut tail = String::new();
        file.read_to_string(&mut tail)
            .map_err(FileTransportError::Read)?;

        let mut consumed = 0_u64;
        let mut records = Vec::new();
        for line in tail.split_inclusive('\n') {
            if !line.ends_with('\n') {
                break;
            }
            consumed += line.len() as u64;
            let line = line.trim_end_matches('\n').trim_end_matches('\r');
            if !line.is_empty() {
                records.push(line.as_bytes().to_vec());
            }
        }

        Ok((
            Cursor {
                offset: cursor.offset + consumed,
            },
            records,
        ))
    }
}

/// Filesystem transport failure.
#[derive(Debug, Error)]
pub(crate) enum FileTransportError {
    /// Could not create the channel directory.
    #[error("failed to create channel directory")]
    CreateDir(#[source] std::io::Error),
    /// Could not open the channel endpoint.
    #[error("failed to open channel endpoint")]
    Open(#[source] std::io::Error),
    /// Could not append to the channel endpoint.
    #[error("failed to append channel record")]
    Write(#[source] std::io::Error),
    /// Could not seek the channel endpoint.
    #[error("failed to seek channel endpoint")]
    Seek(#[source] std::io::Error),
    /// Could not read the channel endpoint.
    #[error("failed to read channel endpoint")]
    Read(#[source] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Private;

#[cfg(test)]
mod tests {
    use super::super::event::{Paths, Refs};
    use super::*;

    fn endpoints(root: PathBuf) -> Endpoints {
        Endpoints::new(
            root,
            CampaignId::from("campaign-1"),
            "node-1".to_string(),
            RuntimeId::new(),
        )
    }

    fn child() -> Child<child::Starting> {
        Child::new(
            PathBuf::from("/tmp/prototype1-test-journal.jsonl"),
            RuntimeId::new(),
            1,
            Refs {
                campaign_id: CampaignId::from("campaign-1"),
                node_id: "node-1".to_string(),
                instance_id: "instance-1".to_string(),
                source_state_id: "source-1".to_string(),
                branch_id: "branch-1".to_string(),
                candidate_id: "candidate-1".to_string(),
                branch_label: "label-1".to_string(),
                spec_id: "spec-1".to_string(),
            },
            Paths {
                repo_root: PathBuf::from("/tmp/repo"),
                workspace_root: PathBuf::from("/tmp/workspace"),
                binary_path: PathBuf::from("/tmp/bin/ploke-eval"),
                target_relpath: PathBuf::from("target.txt"),
                absolute_path: PathBuf::from("/tmp/workspace/target.txt"),
            },
            100,
        )
    }

    fn channel<R>(endpoints: Endpoints) -> Channel<R, FileTransport> {
        Channel::new(endpoints, FileTransport)
    }

    fn seed_owner_db(db_path: &Path) {
        std::fs::create_dir_all(db_path.parent().expect("eval db parent"))
            .expect("create eval db parent");
        ploke_db::Database::new_init()
            .expect("empty eval db")
            .write_backup_to_path(db_path)
            .expect("seed owner eval db");
    }

    #[test]
    fn parent_and_child_have_opposite_directions_from_existing_role_states() {
        let temp = tempfile::tempdir().unwrap();
        let endpoints = endpoints(temp.path().join("channels/runtime-1"));
        let child_role = child();
        let parent = channel::<Parent<parent::Selectable>>(endpoints.clone());
        let child = Channel::for_child(&child_role, endpoints, FileTransport);

        parent.send_cancel("test cancellation").unwrap();
        let (child, _) = child.send_ready().unwrap();

        let (_, child_messages) = child.recv_from_parent(Cursor::start()).unwrap();
        let (_, parent_messages) = parent.recv_from_child(Cursor::start()).unwrap();

        assert_eq!(child_messages.len(), 1);
        assert_eq!(child_messages[0].direction(), Direction::ParentToChild);
        assert_eq!(
            child_messages[0].body(),
            &ToChild::Cancel {
                reason: "test cancellation".to_string()
            }
        );
        assert_eq!(parent_messages.len(), 1);
        assert_eq!(parent_messages[0].direction(), Direction::ChildToParent);
        assert!(matches!(parent_messages[0].body(), ToParent::Ready));
    }

    #[test]
    fn prototype1_eval_store_channel_ready_writes_owner_db_row() {
        let temp = tempfile::tempdir().unwrap();
        let prototype1_root = temp.path().join("prototype1");
        let db_path = prototype1_root.join("eval-store.cozo.sqlite");
        seed_owner_db(&db_path);
        let endpoints = endpoints(prototype1_root.join("nodes/node-1/channels/runtime-1"));
        let endpoint = endpoints.child_to_parent();
        let child_role = child();
        let child = Channel::for_child(&child_role, endpoints, FileTransport);

        let (_child, receipt) = child.send_ready().expect("send ready");

        let db = eval_store::load_owner_eval_database(&db_path).expect("owner eval DB loads");
        let mut params = std::collections::BTreeMap::new();
        params.insert(
            "campaign_id".to_string(),
            cozo::DataValue::from(endpoint.campaign_id().to_string()),
        );
        params.insert(
            "node_id".to_string(),
            cozo::DataValue::from(endpoint.node_id().to_string()),
        );
        params.insert(
            "runtime_id".to_string(),
            cozo::DataValue::from(endpoint.runtime_id().to_string()),
        );
        let rows = db
            .raw_query_params(
                r#"
?[
    direction,
    message_kind,
    store_scope,
    producer_role,
    visibility_scope,
    source_class,
    evidence_class,
    validation_status,
    endpoint_path,
    cursor_offset,
    bytes_written,
    body_hash,
    content_sha256
] :=
    *eval_channel_message {
        campaign_id,
        node_id,
        runtime_id,
        direction,
        message_kind,
        store_scope,
        producer_role,
        visibility_scope,
        source_class,
        evidence_class,
        validation_status,
        endpoint_path,
        cursor_offset,
        bytes_written,
        body_hash,
        content_sha256
    },
    campaign_id = $campaign_id,
    node_id = $node_id,
    runtime_id = $runtime_id
"#,
                params,
            )
            .expect("query channel message rows");

        assert_eq!(rows.rows.len(), 1);
        let row = rows.row_refs().next().expect("channel row");
        assert_eq!(
            row.get::<String>("direction").expect("direction"),
            "child_to_parent"
        );
        assert_eq!(row.get::<String>("message_kind").expect("kind"), "ready");
        assert_eq!(row.get::<String>("store_scope").expect("scope"), "channel");
        assert_eq!(
            row.get::<String>("producer_role").expect("producer"),
            "child"
        );
        assert_eq!(
            row.get::<String>("visibility_scope").expect("visibility"),
            "parent_visible"
        );
        assert_eq!(
            row.get::<String>("source_class").expect("source"),
            "direct_write"
        );
        assert_eq!(
            row.get::<String>("evidence_class").expect("evidence"),
            "channel_message"
        );
        assert_eq!(
            row.get::<String>("validation_status").expect("status"),
            "valid"
        );
        assert_eq!(
            row.get::<String>("endpoint_path").expect("path"),
            receipt.endpoint().display().to_string()
        );
        assert_eq!(
            row.get::<i64>("cursor_offset").expect("cursor"),
            receipt.cursor().offset() as i64
        );
        assert_eq!(
            row.get::<i64>("bytes_written").expect("bytes"),
            receipt.bytes_written() as i64
        );
        assert!(
            !row.get::<String>("body_hash")
                .expect("body hash")
                .is_empty(),
            "channel row carries envelope body hash"
        );
        assert!(
            !row.get::<String>("content_sha256")
                .expect("content hash")
                .is_empty(),
            "channel row carries serialized envelope hash"
        );
    }

    #[test]
    fn prototype1_storage_authority_negative_channel_row_cannot_replace_envelope() {
        let temp = tempfile::tempdir().unwrap();
        let prototype1_root = temp.path().join("prototype1");
        let db_path = prototype1_root.join("eval-store.cozo.sqlite");
        let endpoints = endpoints(prototype1_root.join("nodes/node-1/channels/runtime-1"));
        let endpoint = endpoints.child_to_parent();
        eval_store::write_channel_message_to_owner_db(
            &db_path,
            eval_store::ChannelMessageEvidence {
                campaign_id: endpoint.campaign_id().clone(),
                node_id: endpoint.node_id().to_string(),
                runtime_id: endpoint.runtime_id().to_string(),
                direction: direction_label(endpoint.direction()).to_string(),
                message_kind: "ready".to_string(),
                message_id: Uuid::new_v4().to_string(),
                endpoint_path: endpoint.path().to_path_buf(),
                cursor_offset: 1,
                bytes_written: 1,
                body_hash: "missing-envelope-body-hash".to_string(),
                content_sha256: "missing-envelope-content-hash".to_string(),
                recorded_at: "0".to_string(),
            },
        )
        .expect("write channel mirror row");
        let parent = channel::<Parent<parent::Selectable>>(endpoints);

        let (_, messages) = parent
            .recv_from_child(Cursor::start())
            .expect("read child channel");

        assert!(
            messages.is_empty(),
            "eval_channel_message row must not synthesize a channel envelope"
        );
        assert!(!endpoint.path().exists());
        assert!(db_path.is_file());
    }

    #[test]
    fn file_transport_reads_only_new_complete_records() {
        let temp = tempfile::tempdir().unwrap();
        let endpoints = endpoints(temp.path().join("channels/runtime-1"));
        let child_role = child();
        let child = Channel::for_child(&child_role, endpoints.clone(), FileTransport);
        let parent = channel::<Parent<parent::Selectable>>(endpoints);

        let (child, first) = child.send_ready().unwrap();
        child.send_evaluating().unwrap();

        let (_, all_messages) = parent.recv_from_child(Cursor::start()).unwrap();
        let (_, new_messages) = parent.recv_from_child(first.cursor()).unwrap();

        assert_eq!(all_messages.len(), 2);
        assert_eq!(new_messages.len(), 1);
        assert!(matches!(new_messages[0].body(), ToParent::Evaluating));
    }

    #[test]
    fn envelope_validation_rejects_wrong_endpoint_direction() {
        let temp = tempfile::tempdir().unwrap();
        let endpoints = endpoints(temp.path().join("channels/runtime-1"));
        let child_endpoint = endpoints.child_to_parent();
        let parent_endpoint = endpoints.parent_to_child();
        let envelope = Envelope::new(&child_endpoint, ToParent::Ready).unwrap();

        let error = envelope.validate_endpoint(&parent_endpoint).unwrap_err();

        assert!(matches!(error, EnvelopeError::Direction { .. }));
    }

    #[test]
    fn envelope_validation_rejects_wrong_body_hash() {
        let temp = tempfile::tempdir().unwrap();
        let endpoints = endpoints(temp.path().join("channels/runtime-1"));
        let child_endpoint = endpoints.child_to_parent();
        let mut envelope = Envelope::new(&child_endpoint, ToParent::Ready).unwrap();
        envelope.body_hash = "not-the-real-hash".to_string();

        let error = envelope.validate_body_hash().unwrap_err();

        assert!(matches!(error, EnvelopeError::BodyHash { .. }));
    }
}
