#![allow(missing_docs)]
//! Core orchestration: `RagService` and configuration.
//!
//! [`RagService`] is the primary entry point for performing sparse, dense, and hybrid retrieval,
//! as well as assembling a token-budgeted context. It wraps the BM25 actor with timeouts, optional
//! retries/backoff (lenient mode), and exposes strict variants and persistence hooks.
//!
//! Configure behavior via [`RagConfig`], including fusion defaults, dense search parameters, and
//! context assembly policy. For diversity or learning-to-rank experiments, plug in a custom [`Reranker`].
mod unit_tests;
use super::*;
use ploke_core::rag_types::AssembledContext;
#[cfg(feature = "call_graph")]
use ploke_core::rag_types::{
    CallCalleeInfo, CallContextInfo, CallReceiverInfo, CallResolutionKind as RagCallResolutionKind,
    CallSiteKind as RagCallSiteKind, CallStatusKind as RagCallStatusKind, CallTargetInfo,
    CallTargetKind,
};
#[cfg(feature = "call_graph")]
use ploke_db::{
    CallContextRow, CallReceiver, CallRelationKind, CallResolutionKind, CallSiteKind,
    CallStatusKind,
};
use ploke_embed::indexer::EmbeddingProcessor;
use ploke_embed::runtime::EmbeddingRuntime;
use ploke_io::IoManagerHandle;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::trace;

#[derive(Debug, Clone, Copy)]
pub enum RetrievalStrategy {
    Dense,
    Sparse {
        strict: Option<bool>,
    },
    Hybrid {
        rrf: RrfConfig,
        mmr: Option<MmrConfig>,
    },
}
impl Default for RetrievalStrategy {
    fn default() -> Self {
        Self::Hybrid {
            rrf: RrfConfig::default(),
            mmr: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SearchParams {
    pub ef: usize,
    pub radius: f64,
    pub max_hits: usize,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            ef: 10,
            radius: 10.0,
            max_hits: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RagConfig {
    pub bm25_timeout_ms: u64,
    pub bm25_retry_backoff_ms: Vec<u64>,
    pub strict_bm25_by_default: bool,
    pub rrf_default: RrfConfig,
    pub mmr_default: Option<MmrConfig>,
    pub score_norm: ScoreNorm,
    pub search_per_type: HashMap<NodeType, SearchParams>,
    pub assembly_policy: AssemblyPolicy,
    pub token_counter: Arc<dyn TokenCounter>,
    pub reranker: Option<Arc<dyn Reranker>>,
    pub type_context: TypeContextConfig,
    #[cfg(feature = "call_graph")]
    pub call_context: CallContextConfig,
}

#[derive(Debug, Clone, Copy)]
pub struct TypeContextConfig {
    pub enabled: bool,
    pub max_seed_hits: usize,
    pub max_expanded_hits: usize,
    pub score_factor: f32,
    pub options: TypeContextOptions,
}

impl Default for TypeContextConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_seed_hits: 12,
            max_expanded_hits: 48,
            score_factor: 0.6,
            options: TypeContextOptions {
                max_distance: 4,
                ..TypeContextOptions::default()
            },
        }
    }
}

#[cfg(feature = "call_graph")]
#[derive(Debug, Clone, Copy)]
pub struct CallContextConfig {
    pub enabled: bool,
    pub max_owner_hits: usize,
    pub max_sites_per_owner: usize,
    pub max_targets_per_site: usize,
}

#[cfg(feature = "call_graph")]
impl Default for CallContextConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_owner_hits: 12,
            max_sites_per_owner: 16,
            max_targets_per_site: 8,
        }
    }
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            bm25_timeout_ms: crate::BM25_TIMEOUT_MS,
            bm25_retry_backoff_ms: crate::BM25_RETRY_BACKOFF_MS.to_vec(),
            strict_bm25_by_default: false,
            rrf_default: RrfConfig::default(),
            mmr_default: None,
            score_norm: ScoreNorm::default(),
            search_per_type: HashMap::new(),
            assembly_policy: AssemblyPolicy::default(),
            token_counter: Arc::new(crate::context::ApproxCharTokenizer),
            reranker: None,
            type_context: TypeContextConfig::default(),
            #[cfg(feature = "call_graph")]
            call_context: CallContextConfig::default(),
        }
    }
}

impl RagConfig {
    pub fn params_for(&self, ty: NodeType) -> SearchParams {
        self.search_per_type.get(&ty).copied().unwrap_or_default()
    }
}

/// Optional async reranker for candidate reordering using snippet texts.
#[allow(clippy::type_complexity)]
pub trait Reranker: Send + Sync + std::fmt::Debug {
    fn rerank<'a>(
        &'a self,
        query: &'a str,
        candidates: Vec<(Uuid, String)>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<(Uuid, f32)>, RagError>> + Send + 'a>>;
}

#[derive(Debug, Default)]
pub struct NoopReranker;

impl Reranker for NoopReranker {
    fn rerank<'a>(
        &'a self,
        _query: &'a str,
        candidates: Vec<(Uuid, String)>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<(Uuid, f32)>, RagError>> + Send + 'a>> {
        Box::pin(async move { Ok(candidates.into_iter().map(|(id, _)| (id, 1.0)).collect()) })
    }
}

fn type_context_kind(relation: TypeContextRelation) -> TypeContextKind {
    match relation {
        TypeContextRelation::SameResolvedType => TypeContextKind::SameResolvedType,
        TypeContextRelation::UsesTypeNested => TypeContextKind::UsesTypeNested,
        TypeContextRelation::TypeDefinitionImpact => TypeContextKind::TypeDefinitionImpact,
        TypeContextRelation::ImplOfTrait => TypeContextKind::ImplOfTrait,
        TypeContextRelation::ImplSelfType => TypeContextKind::ImplSelfType,
        TypeContextRelation::AliasExpansion => TypeContextKind::AliasExpansion,
        TypeContextRelation::TraitBound => TypeContextKind::TraitBound,
        TypeContextRelation::IteratorSurface => TypeContextKind::IteratorSurface,
        TypeContextRelation::ConstGenericAlias => TypeContextKind::ConstGenericAlias,
    }
}

#[cfg(feature = "call_graph")]
fn row_to_call_context(
    row: CallContextRow,
    max_targets: usize,
) -> Result<CallContextInfo, RagError> {
    let callee = match row.site.kind {
        CallSiteKind::Path => CallCalleeInfo::Path {
            path: row.site.path.ok_or_else(|| {
                DbError::Cozo(format!(
                    "path call site {} missing path payload",
                    row.site.id
                ))
            })?,
        },
        CallSiteKind::Method => CallCalleeInfo::Method {
            name: row.site.method.ok_or_else(|| {
                DbError::Cozo(format!(
                    "method call site {} missing method payload",
                    row.site.id
                ))
            })?,
            receiver: row.site.receiver.map(receiver_info),
        },
        CallSiteKind::Dynamic => CallCalleeInfo::Dynamic,
        CallSiteKind::Macro => CallCalleeInfo::Macro {
            name: row.site.macro_name.ok_or_else(|| {
                DbError::Cozo(format!(
                    "macro call site {} missing macro payload",
                    row.site.id
                ))
            })?,
        },
    };

    Ok(CallContextInfo {
        site_id: row.site.id,
        kind: site_kind(row.site.kind),
        span: row.site.span,
        callee,
        status: status_kind(row.status.status),
        resolution: row.status.resolution.map(resolution_kind),
        targets: row
            .targets
            .into_iter()
            .take(max_targets)
            .map(|target| CallTargetInfo {
                target_id: target.target_id,
                relation: target_kind(target.relation),
            })
            .collect(),
    })
}

#[cfg(feature = "call_graph")]
fn site_kind(kind: CallSiteKind) -> RagCallSiteKind {
    match kind {
        CallSiteKind::Path => RagCallSiteKind::Path,
        CallSiteKind::Method => RagCallSiteKind::Method,
        CallSiteKind::Dynamic => RagCallSiteKind::Dynamic,
        CallSiteKind::Macro => RagCallSiteKind::Macro,
    }
}

#[cfg(feature = "call_graph")]
fn receiver_info(receiver: CallReceiver) -> CallReceiverInfo {
    match receiver {
        CallReceiver::SelfValue => CallReceiverInfo::SelfValue,
        CallReceiver::SelfField { path } => CallReceiverInfo::SelfField { path },
        CallReceiver::LocalBinding { name } => CallReceiverInfo::LocalBinding { name },
        CallReceiver::TypedLocalBinding { name, type_path } => {
            CallReceiverInfo::TypedLocalBinding { name, type_path }
        }
        CallReceiver::InitializedLocalBinding { name, init_path } => {
            CallReceiverInfo::InitializedLocalBinding { name, init_path }
        }
        CallReceiver::BorrowedLocalBinding { name } => {
            CallReceiverInfo::BorrowedLocalBinding { name }
        }
        CallReceiver::BorrowedTypedLocalBinding { name, type_path } => {
            CallReceiverInfo::BorrowedTypedLocalBinding { name, type_path }
        }
        CallReceiver::DereferencedLocalBinding { name } => {
            CallReceiverInfo::DereferencedLocalBinding { name }
        }
        CallReceiver::DereferencedInitializedLocalBinding { name, init_path } => {
            CallReceiverInfo::DereferencedInitializedLocalBinding { name, init_path }
        }
        CallReceiver::FieldLocalBinding { name, field_path } => {
            CallReceiverInfo::FieldLocalBinding { name, field_path }
        }
        CallReceiver::FieldTypedLocalBinding {
            name,
            type_path,
            field_path,
        } => CallReceiverInfo::FieldTypedLocalBinding {
            name,
            type_path,
            field_path,
        },
        CallReceiver::FieldInitializedLocalBinding {
            name,
            init_path,
            field_path,
        } => CallReceiverInfo::FieldInitializedLocalBinding {
            name,
            init_path,
            field_path,
        },
        CallReceiver::PathCallResult { path } => CallReceiverInfo::PathCallResult { path },
        CallReceiver::MethodCallResult { method_name } => {
            CallReceiverInfo::MethodCallResult { method_name }
        }
        CallReceiver::AwaitResult => CallReceiverInfo::AwaitResult,
        CallReceiver::AwaitPathCallResult { path } => {
            CallReceiverInfo::AwaitPathCallResult { path }
        }
        CallReceiver::TryResult => CallReceiverInfo::TryResult,
        CallReceiver::TryPathCallResult { path } => CallReceiverInfo::TryPathCallResult { path },
        CallReceiver::Literal => CallReceiverInfo::Literal,
    }
}

#[cfg(feature = "call_graph")]
fn target_kind(kind: CallRelationKind) -> CallTargetKind {
    match kind {
        CallRelationKind::Function => CallTargetKind::Function,
        CallRelationKind::DynamicFunction => CallTargetKind::DynamicFunction,
        CallRelationKind::Method => CallTargetKind::Method,
        CallRelationKind::AssociatedFunction => CallTargetKind::AssociatedFunction,
        CallRelationKind::TupleStructConstructor => CallTargetKind::TupleStructConstructor,
        CallRelationKind::EnumVariantConstructor => CallTargetKind::EnumVariantConstructor,
        CallRelationKind::Struct => CallTargetKind::TupleStructConstructor,
        CallRelationKind::Variant => CallTargetKind::EnumVariantConstructor,
    }
}

#[cfg(feature = "call_graph")]
fn status_kind(kind: CallStatusKind) -> RagCallStatusKind {
    match kind {
        CallStatusKind::Resolved => RagCallStatusKind::Resolved,
        CallStatusKind::Unresolved => RagCallStatusKind::Unresolved,
        CallStatusKind::Ambiguous => RagCallStatusKind::Ambiguous,
        CallStatusKind::External => RagCallStatusKind::External,
        CallStatusKind::Unsupported => RagCallStatusKind::Unsupported,
    }
}

#[cfg(feature = "call_graph")]
fn resolution_kind(kind: CallResolutionKind) -> RagCallResolutionKind {
    match kind {
        CallResolutionKind::LocalExact => RagCallResolutionKind::LocalExact,
    }
}

/// RAG orchestration service.
///
/// This orchestrates hybrid search by combining:
/// - BM25 sparse search served by an in-memory actor
/// - Dense vector search served by the HNSW index in the database
///
/// Notes:
/// - BM25 search will gracefully fall back to dense search if the BM25 index is empty,
///   ensuring callers do not receive empty results due to indexing lag.
/// - Use `hybrid_search` to fuse the results via RRF for robust retrieval.
///
/// See crate tests for end-to-end examples using a fixture database.
#[derive(Debug)]
pub struct RagService {
    db: Arc<Database>,
    dense_embedder: Arc<EmbeddingRuntime>,
    bm_embedder: mpsc::Sender<Bm25Cmd>,
    cfg: RagConfig,
    io: Option<Arc<IoManagerHandle>>,
    type_context_degraded: bool,
    #[cfg(feature = "call_graph")]
    call_context_degraded: bool,
}

impl RagService {
    fn assemble(
        db: Arc<Database>,
        dense_embedder: Arc<EmbeddingRuntime>,
        bm_embedder: mpsc::Sender<Bm25Cmd>,
        mut cfg: RagConfig,
        io: Option<Arc<IoManagerHandle>>,
    ) -> Result<Self, RagError> {
        let type_context_degraded = Self::apply_type_context_gate(&db, &mut cfg)?;
        #[cfg(feature = "call_graph")]
        let call_context_degraded = Self::apply_call_context_gate(&db, &mut cfg)?;
        Ok(Self {
            db,
            dense_embedder,
            bm_embedder,
            cfg,
            io,
            type_context_degraded,
            #[cfg(feature = "call_graph")]
            call_context_degraded,
        })
    }

    fn apply_type_context_gate(db: &Database, cfg: &mut RagConfig) -> Result<bool, RagError> {
        if !cfg.type_context.enabled {
            return Ok(false);
        }
        if db.has_typed_type_graph_relations()? {
            return Ok(false);
        }
        tracing::warn!(
            "typed type-context expansion disabled: active database is missing typed-graph relations (type_contains, type_use, type_relation)"
        );
        cfg.type_context.enabled = false;
        Ok(true)
    }

    #[cfg(feature = "call_graph")]
    fn apply_call_context_gate(db: &Database, cfg: &mut RagConfig) -> Result<bool, RagError> {
        if !cfg.call_context.enabled {
            return Ok(false);
        }
        if db.has_call_graph_relations()? {
            return Ok(false);
        }
        tracing::warn!(
            "call-context payloads disabled: active database is missing call graph relations"
        );
        cfg.call_context.enabled = false;
        Ok(true)
    }

    /// Construct a new RAG service, starting the BM25 service actor.
    pub fn new(db: Arc<Database>, dense_embedder: Arc<EmbeddingRuntime>) -> Result<Self, RagError> {
        // ensure_tracer_initialized();
        let bm_embedder = bm25_service::start_default(db.clone())?;
        Self::assemble(db, dense_embedder, bm_embedder, RagConfig::default(), None)
    }

    /// Construct with explicit configuration (no IoManager).
    pub fn new_with_config(
        db: Arc<Database>,
        dense_embedder: Arc<EmbeddingRuntime>,
        cfg: RagConfig,
    ) -> Result<Self, RagError> {
        let bm_embedder = bm25_service::start_default(db.clone())?;
        Self::assemble(db, dense_embedder, bm_embedder, cfg, None)
    }

    /// Construct with an IoManager and default configuration.
    pub fn new_with_io(
        db: Arc<Database>,
        dense_embedder: Arc<EmbeddingRuntime>,
        io: IoManagerHandle,
    ) -> Result<Self, RagError> {
        Self::new_full(db, dense_embedder, io, RagConfig::default())
    }

    /// Construct with both IoManager and explicit configuration.
    pub fn new_full(
        db: Arc<Database>,
        dense_embedder: Arc<EmbeddingRuntime>,
        io: IoManagerHandle,
        cfg: RagConfig,
    ) -> Result<Self, RagError> {
        let bm_embedder = bm25_service::start_default(db.clone())?;
        Self::assemble(db, dense_embedder, bm_embedder, cfg, Some(Arc::new(io)))
    }

    /// Construct with both IoManager and rebuild avgld from db contents.
    pub fn new_rebuilt(
        db: Arc<Database>,
        dense_embedder: Arc<EmbeddingRuntime>,
        io: IoManagerHandle,
        cfg: RagConfig,
    ) -> Result<Self, RagError> {
        let bm_embedder = bm25_service::start_rebuilt(db.clone())?;
        Self::assemble(db, dense_embedder, bm_embedder, cfg, Some(Arc::new(io)))
    }

    /// Returns true when typed type-context expansion was requested but disabled
    /// because the active database lacks typed-graph relations.
    pub fn type_context_degraded(&self) -> bool {
        self.type_context_degraded
    }

    #[cfg(feature = "call_graph")]
    pub fn call_context_degraded(&self) -> bool {
        self.call_context_degraded
    }

    /// Convenience constructor for tests with an in-memory database and mock embedder.
    pub fn new_mock() -> Self {
        let db = Arc::new(ploke_db::Database::init_with_schema().expect("init test db"));
        let dense_embedder = Arc::new(ploke_embed::runtime::EmbeddingRuntime::with_default_set(
            ploke_embed::indexer::EmbeddingProcessor::new_mock(),
        ));
        let bm_embedder = bm25_service::start_default(db.clone()).expect("start bm25");
        Self::assemble(db, dense_embedder, bm_embedder, RagConfig::default(), None)
            .expect("in-memory test database should satisfy type-context gate")
    }

    /// Execute a BM25 search against the in-memory sparse index.
    ///
    /// Lenient mode: if the BM25 index is not ready or empty, this method may fall back to dense search.
    /// Returns a Vec of (document_id, score) pairs sorted by relevance.
    #[instrument(skip(self, query), fields(query_len = %query.len(), top_k = top_k, strict = false, timeout_ms = BM25_TIMEOUT_MS))]
    pub async fn search_bm25(
        &self,
        query: &str,
        top_k: usize,
        scope: RetrievalScope,
    ) -> Result<Vec<(Uuid, f32)>, RagError> {
        let mut attempts: usize = 0;

        loop {
            attempts += 1;

            let status_opt = match self.bm25_status().await {
                Ok(s) => Some(s),
                Err(e) => {
                    debug!("bm25_status check failed: {:?}", e);
                    None
                }
            };

            let (tx, rx) = oneshot::channel();
            self.bm_embedder
                .send(Bm25Cmd::Search {
                    query: query.to_string(),
                    top_k,
                    scope,
                    resp: tx,
                })
                .await
                .map_err(|e| {
                    RagError::Channel(format!(
                        "failed to send BM25 search command (len={}, top_k={}): {}",
                        query.len(),
                        top_k,
                        e
                    ))
                })?;

            let res = match timeout(Duration::from_millis(self.cfg.bm25_timeout_ms), rx).await {
                Ok(Ok(r)) => r,
                Ok(Err(recv_err)) => {
                    return Err(RagError::Channel(format!(
                        "BM25 search response channel closed (len={}, top_k={}): {}",
                        query.len(),
                        top_k,
                        recv_err
                    )));
                }
                Err(_) => {
                    return Err(RagError::Channel(format!(
                        "timeout waiting for BM25 search ({} ms)",
                        self.cfg.bm25_timeout_ms
                    )));
                }
            };

            if !res.is_empty() {
                debug!(
                    bm25_results = res.len(),
                    attempts = attempts,
                    "BM25 search succeeded"
                );
                return Ok(res);
            }

            // Decide retry/fallback based on status.
            let mut fallback_used = false;
            let should_retry = matches!(
                status_opt,
                Some(Bm25Status::Uninitialized) | Some(Bm25Status::Building)
            ) && attempts <= self.cfg.bm25_retry_backoff_ms.len();

            if should_retry {
                let backoff = self.cfg.bm25_retry_backoff_ms[attempts - 1];
                debug!(attempts = attempts, bm25_results = 0, bm25_status = ?status_opt, backoff_ms = backoff, "BM25 empty; retrying after backoff");
                sleep(Duration::from_millis(backoff)).await;
                continue;
            }

            let use_fallback = !matches!(status_opt, Some(Bm25Status::Ready { docs }) if docs > 0);

            if use_fallback {
                fallback_used = true;
                debug!(bm25_results = 0, bm25_status = ?status_opt, attempts = attempts, fallback_used = fallback_used, "BM25 not ready/empty; falling back to dense search");
                let dense_list = self.search(query, top_k, scope).await.map_err(|e| {
                    RagError::Embed(format!(
                        "dense search failed during BM25 fallback (len={}, top_k={}): {:?}",
                        query.len(),
                        top_k,
                        e
                    ))
                })?;
                debug!(dense_results = dense_list.len(), "Dense fallback complete");
                return Ok(dense_list);
            } else {
                debug!(bm25_results = 0, bm25_status = ?status_opt, attempts = attempts, fallback_used = fallback_used, "BM25 ready but returned 0 results; not falling back");
                return Ok(res);
            }
        }
    }

    /// Trigger a rebuild of the BM25 sparse index.
    /// For now, this is a fire-and-forget command to the BM25 service.
    #[instrument(skip(self))]
    pub async fn bm25_rebuild(&self) -> Result<(), RagError> {
        self.bm_embedder.send(Bm25Cmd::Rebuild).await.map_err(|e| {
            RagError::Channel(format!("failed to send BM25 rebuild command: {}", e))
        })?;
        debug!("BM25 rebuild command sent");
        Ok(())
    }

    /// Query BM25 actor for current status with a client-side timeout.
    #[instrument(skip(self), fields(timeout_ms = BM25_TIMEOUT_MS))]
    pub async fn bm25_status(&self) -> Result<Bm25Status, RagError> {
        self.bm25_status_with_timeout(Duration::from_millis(self.cfg.bm25_timeout_ms))
            .await
    }

    /// Query BM25 actor for current status with a caller-supplied timeout.
    ///
    /// Long-running setup paths may have just queued a rebuild and need to wait
    /// behind that rebuild rather than treating the normal interactive timeout
    /// as a fatal actor failure.
    #[instrument(skip(self), fields(timeout_ms = timeout_duration.as_millis()))]
    pub async fn bm25_status_with_timeout(
        &self,
        timeout_duration: Duration,
    ) -> Result<Bm25Status, RagError> {
        let (tx, rx) = oneshot::channel();
        self.bm_embedder
            .send(Bm25Cmd::Status { resp: tx })
            .await
            .map_err(|e| RagError::Channel(format!("failed to send BM25 status command: {}", e)))?;
        match timeout(timeout_duration, rx).await {
            Ok(Ok(Ok(status))) => Ok(status),
            Ok(Ok(Err(db_err))) => Err(RagError::Db(db_err)),
            Ok(Err(recv_err)) => Err(RagError::Channel(format!(
                "BM25 status response channel closed: {}",
                recv_err
            ))),
            Err(_) => Err(RagError::Channel(format!(
                "timeout waiting for BM25 status ({} ms)",
                timeout_duration.as_millis()
            ))),
        }
    }

    /// Save BM25 sidecar state to path via actor with timeout.
    #[instrument(skip(self, path), fields(timeout_ms = BM25_TIMEOUT_MS))]
    pub async fn bm25_save<P: AsRef<std::path::Path> + Send>(
        &self,
        path: P,
    ) -> Result<(), RagError> {
        let (tx, rx) = oneshot::channel();
        self.bm_embedder
            .send(Bm25Cmd::Save {
                path: path.as_ref().to_path_buf(),
                resp: tx,
            })
            .await
            .map_err(|e| RagError::Channel(format!("failed to send BM25 save command: {}", e)))?;
        match timeout(Duration::from_millis(self.cfg.bm25_timeout_ms), rx).await {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(db_err))) => Err(RagError::Db(db_err)),
            Ok(Err(recv_err)) => Err(RagError::Channel(format!(
                "BM25 save response channel closed: {}",
                recv_err
            ))),
            Err(_) => Err(RagError::Channel(format!(
                "timeout waiting for BM25 save ({} ms)",
                self.cfg.bm25_timeout_ms
            ))),
        }
    }

    /// Load BM25 state from path via actor with timeout.
    #[instrument(skip(self, path), fields(timeout_ms = BM25_TIMEOUT_MS))]
    pub async fn bm25_load<P: AsRef<std::path::Path> + Send>(
        &self,
        path: P,
    ) -> Result<(), RagError> {
        let (tx, rx) = oneshot::channel();
        self.bm_embedder
            .send(Bm25Cmd::Load {
                path: path.as_ref().to_path_buf(),
                resp: tx,
            })
            .await
            .map_err(|e| RagError::Channel(format!("failed to send BM25 load command: {}", e)))?;
        match timeout(Duration::from_millis(self.cfg.bm25_timeout_ms), rx).await {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(db_err))) => Err(RagError::Db(db_err)),
            Ok(Err(recv_err)) => Err(RagError::Channel(format!(
                "BM25 load response channel closed: {}",
                recv_err
            ))),
            Err(_) => Err(RagError::Channel(format!(
                "timeout waiting for BM25 load ({} ms)",
                self.cfg.bm25_timeout_ms
            ))),
        }
    }

    /// Execute a BM25 search in strict mode: no dense fallback.
    /// Returns error if the index is Uninitialized/Building/Empty when results are empty.
    #[instrument(skip(self, query), fields(query_len = %query.len(), top_k = top_k, strict = true, timeout_ms = BM25_TIMEOUT_MS))]
    pub async fn search_bm25_strict(
        &self,
        query: &str,
        top_k: usize,
        scope: RetrievalScope,
    ) -> Result<Vec<(Uuid, f32)>, RagError> {
        let status = self.bm25_status().await?;

        let (tx, rx) = oneshot::channel();
        self.bm_embedder
            .send(Bm25Cmd::Search {
                query: query.to_string(),
                top_k,
                scope,
                resp: tx,
            })
            .await
            .map_err(|e| {
                RagError::Channel(format!(
                    "failed to send BM25 search command (len={}, top_k={}): {}",
                    query.len(),
                    top_k,
                    e
                ))
            })?;

        let res = match timeout(Duration::from_millis(self.cfg.bm25_timeout_ms), rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(recv_err)) => {
                return Err(RagError::Channel(format!(
                    "BM25 search response channel closed (len={}, top_k={}): {}",
                    query.len(),
                    top_k,
                    recv_err
                )));
            }
            Err(_) => {
                return Err(RagError::Channel(format!(
                    "timeout waiting for BM25 search ({} ms)",
                    self.cfg.bm25_timeout_ms
                )));
            }
        };

        if !res.is_empty() {
            debug!(bm25_results = res.len(), "BM25 strict search succeeded");
            return Ok(res);
        }

        match status {
            Bm25Status::Uninitialized | Bm25Status::Building => {
                Err(RagError::Search("bm25 index not ready".to_string()))
            }
            Bm25Status::Empty | Bm25Status::Ready { docs: 0 } => {
                Err(RagError::Search("bm25 index empty".to_string()))
            }
            Bm25Status::Error(msg) => Err(RagError::Search(format!("bm25 error state: {}", msg))),
            Bm25Status::Ready { docs: _ } => {
                debug!("BM25 strict: Ready but query returned 0 results");
                Ok(res)
            }
        }
    }

    /// Perform a dense search using the HNSW index in the database.
    /// Returns a Vec of (snippet_id, score) pairs sorted by relevance.
    #[instrument(skip(self, query), fields(query_len = %query.len(), top_k = top_k))]
    pub async fn search(
        &self,
        query: &str,
        top_k: usize,
        scope: RetrievalScope,
    ) -> Result<Vec<(Uuid, f32)>, ploke_error::Error> {
        // Generate embedding for the query
        let embeddings = self
            .dense_embedder
            .generate_embeddings(vec![query.to_string()])
            .await?;

        let query_embedding = embeddings
            .into_iter()
            .next()
            .ok_or_else(|| RagError::Embed("failed to generate query embedding".to_string()))?;

        // Collect results from all node types including methods; pre-allocate to avoid reallocations
        let node_types = NodeType::primary_and_assoc_nodes();
        let mut all_results: Vec<(Uuid, f32)> = Vec::with_capacity(node_types.len() * top_k);

        for node_type in node_types {
            let params = self.cfg.params_for(node_type);
            let max_hits = params.max_hits.max(top_k);
            let args = SimilarArgs {
                db: &self.db,
                vector_query: &query_embedding,
                scope,
                k: top_k * 8,
                ef: params.ef, // Configurable ef value
                ty: node_type,
                max_hits,
                // radius: params.radius, // Configurable radius value
                radius: 100.0, // Configurable radius value
            };

            let result = search_similar_args(args)?;

            // Convert distance to similarity score (lower distance = higher similarity)
            all_results.extend(
                result
                    .typed_data
                    .v
                    .into_iter()
                    .zip(result.dist.into_iter())
                    .map(|(embed_data, distance)| (embed_data.id, 1.0 - distance as f32)),
            );
        }

        // Sort by score (highest first) and take top_k
        all_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        all_results.truncate(top_k);

        Ok(all_results)
    }

    fn expand_hits_with_type_context(
        &self,
        hits: &[(Uuid, f32)],
    ) -> Result<(Vec<(Uuid, f32)>, HashMap<Uuid, TypeContextInfo>), RagError> {
        let cfg = self.cfg.type_context;
        if !cfg.enabled || cfg.max_seed_hits == 0 || cfg.max_expanded_hits == 0 || hits.is_empty() {
            return Ok((hits.to_vec(), HashMap::new()));
        }

        let mut scores: HashMap<Uuid, f32> = HashMap::with_capacity(
            hits.len()
                + cfg
                    .max_seed_hits
                    .saturating_mul(cfg.max_expanded_hits.min(8)),
        );
        for &(id, score) in hits {
            scores.entry(id).or_insert(score);
        }

        let mut expanded_scores: HashMap<Uuid, f32> = HashMap::new();
        let mut expanded_context: HashMap<Uuid, TypeContextInfo> = HashMap::new();
        let mut owner_terminal_targets: HashSet<Uuid> = HashSet::new();
        for &(seed_id, seed_score) in hits.iter().take(cfg.max_seed_hits) {
            owner_terminal_targets.extend(
                self.db
                    .type_targets_reachable_from_owner(seed_id)?
                    .into_iter()
                    .map(|target| target.target_id),
            );
            for seed in [
                TypeContextSeed::Owner(seed_id),
                TypeContextSeed::Target(seed_id),
            ] {
                for candidate in self.db.expand_type_context(seed, cfg.options)? {
                    if candidate.node_id == seed_id || scores.contains_key(&candidate.node_id) {
                        continue;
                    }

                    let distance = candidate.distance.max(1) as f32;
                    let derived_score = seed_score * cfg.score_factor / distance;
                    expanded_scores
                        .entry(candidate.node_id)
                        .and_modify(|score| *score = score.max(derived_score))
                        .or_insert(derived_score);
                    expanded_context
                        .entry(candidate.node_id)
                        .and_modify(|existing| {
                            if candidate.distance < existing.distance {
                                *existing = TypeContextInfo {
                                    seed_id,
                                    relation: type_context_kind(candidate.relation),
                                    distance: candidate.distance,
                                };
                            }
                        })
                        .or_insert(TypeContextInfo {
                            seed_id,
                            relation: type_context_kind(candidate.relation),
                            distance: candidate.distance,
                        });
                }
            }
        }

        if expanded_scores.is_empty() {
            return Ok((hits.to_vec(), HashMap::new()));
        }

        let mut expanded: Vec<(Uuid, f32)> = expanded_scores.into_iter().collect();
        expanded.sort_by(|(left_id, left_score), (right_id, right_score)| {
            match owner_terminal_targets
                .contains(right_id)
                .cmp(&owner_terminal_targets.contains(left_id))
            {
                std::cmp::Ordering::Equal => match right_score
                    .partial_cmp(left_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
                {
                    std::cmp::Ordering::Equal => left_id.as_bytes().cmp(right_id.as_bytes()),
                    other => other,
                },
                other => other,
            }
        });
        expanded.truncate(cfg.max_expanded_hits);

        let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        let materialized_ids = self
            .db
            .get_snippet_nodes_ordered(expanded_ids)
            .map_err(|e| RagError::Embed(e.to_string()))?
            .into_iter()
            .map(|node| node.id)
            .collect::<HashSet<_>>();

        let mut merged = Vec::with_capacity(hits.len() + materialized_ids.len());
        merged.extend_from_slice(hits);
        merged.extend(
            expanded
                .into_iter()
                .filter(|(id, _)| materialized_ids.contains(id)),
        );
        expanded_context.retain(|id, _| materialized_ids.contains(id));
        Ok((merged, expanded_context))
    }

    #[cfg(feature = "call_graph")]
    fn collect_call_context(
        &self,
        hits: &[(Uuid, f32)],
    ) -> Result<HashMap<Uuid, Vec<CallContextInfo>>, RagError> {
        let cfg = self.cfg.call_context;
        if !cfg.enabled || cfg.max_owner_hits == 0 || hits.is_empty() {
            return Ok(HashMap::new());
        }

        let mut out = HashMap::new();
        for &(owner_id, _) in hits.iter().take(cfg.max_owner_hits) {
            let rows = self.db.call_context_for_owner(owner_id)?;
            if rows.is_empty() {
                continue;
            }
            let context = rows
                .into_iter()
                .take(cfg.max_sites_per_owner)
                .map(|row| row_to_call_context(row, cfg.max_targets_per_site))
                .collect::<Result<Vec<_>, _>>()?;
            if !context.is_empty() {
                out.insert(owner_id, context);
            }
        }
        Ok(out)
    }

    /// High-level API: retrieve and assemble a context using the chosen strategy and budget.
    /// Uses configured defaults (policy, tokenizer, strict bm25) unless overridden by the strategy.
    #[instrument(skip(self, query, budget, strategy), fields(query_len = %query.len(), top_k = top_k))]
    pub async fn get_context(
        &self,
        query: &str,
        top_k: usize,
        budget: &TokenBudget,
        strategy: &RetrievalStrategy,
        scope: RetrievalScope,
    ) -> Result<AssembledContext, RagError> {
        // 1) Retrieve hits according to strategy
        let hits: Vec<(Uuid, f32)> = match strategy {
            RetrievalStrategy::Dense => self
                .search(query, top_k, scope)
                .await
                .map_err(|e| RagError::Embed(format!("dense search failed: {:?}", e)))?,
            RetrievalStrategy::Sparse { strict } => {
                let use_strict = strict.unwrap_or(self.cfg.strict_bm25_by_default);
                if use_strict {
                    self.search_bm25_strict(query, top_k, scope).await?
                } else {
                    self.search_bm25(query, top_k, scope).await?
                }
            }
            RetrievalStrategy::Hybrid { rrf, mmr } => {
                let bm25_fut = self.search_bm25(query, top_k, scope);
                let dense_fut = self.search(query, top_k, scope);
                let (bm25_res, dense_res) = tokio::join!(bm25_fut, dense_fut);
                let bm25_list = bm25_res?;
                let dense_list = dense_res
                    .map_err(|e| RagError::Embed(format!("dense search failed: {:?}", e)))?;
                let mut fused = rrf_fuse(&bm25_list, &dense_list, rrf);
                fused.truncate(top_k);
                if let Some(mcfg) = mmr {
                    // Optional diversity re-ranking based on embeddings; default to no-embeddings map.
                    let embed_map: HashMap<Uuid, Vec<f32>> = HashMap::new();

                    mmr_select(&fused, top_k, &embed_map, mcfg)
                } else {
                    fused
                }
            }
        };

        let (hits, type_context) = self.expand_hits_with_type_context(&hits)?;

        // Optional reranker: requires IoManager to fetch texts
        let final_hits: Vec<(Uuid, f32)> = if let Some(rr) = &self.cfg.reranker {
            let io = self
                .io
                .as_ref()
                .ok_or_else(|| {
                    RagError::Search("IoManagerHandle not configured for reranking".to_string())
                })?
                .clone();

            // Fetch texts for candidates
            let ids: Vec<Uuid> = hits.iter().map(|(id, _)| *id).collect();
            let nodes = self
                .db
                .get_snippet_nodes_ordered(ids.clone())
                .map_err(|e| RagError::Embed(e.to_string()))?;
            let texts = io.get_snippets_batch(nodes).await.map_err(|e| {
                RagError::Search(format!("get_snippets_batch failed for rerank: {:?}", e))
            })?;
            let mut cand: Vec<(Uuid, String)> = Vec::new();
            for (i, res) in texts.into_iter().enumerate() {
                if let Some(id) = ids.get(i) {
                    match res {
                        Ok(s) => cand.push((*id, s)),
                        Err(e) => {
                            if self.cfg.assembly_policy.strict_io {
                                return Err(RagError::Search(format!(
                                    "IO error during rerank snippet fetch: {:?}",
                                    e
                                )));
                            }
                        }
                    }
                }
            }
            rr.rerank(query, cand).await?
        } else {
            hits
        };

        #[cfg(feature = "call_graph")]
        let call_context = self.collect_call_context(&final_hits)?;

        // 2) Assemble context
        let io = self
            .io
            .as_ref()
            .ok_or_else(|| RagError::Search("IoManagerHandle not configured".to_string()))?
            .clone();

        #[cfg(feature = "call_graph")]
        {
            return crate::context::assemble_context_with_context_maps(
                query,
                &final_hits,
                budget,
                &self.cfg.assembly_policy,
                &*self.cfg.token_counter,
                &self.db,
                &io,
                &type_context,
                &call_context,
            )
            .await;
        }

        #[cfg(not(feature = "call_graph"))]
        assemble_context_with_type_context(
            query,
            &final_hits,
            budget,
            &self.cfg.assembly_policy,
            &*self.cfg.token_counter,
            &self.db,
            &io,
            &type_context,
        )
        .await
    }

    /// Perform a hybrid search (BM25 + dense).
    ///
    /// Strict mode: if the dense search fails, propagate an error (do not silently fall back).
    /// Runs BM25 and dense searches concurrently, fuses results using Reciprocal Rank Fusion (RRF),
    /// and returns the top_k results ordered by fused score (higher = better).
    #[instrument(skip(self, query), fields(query_len = %query.len(), top_k = top_k))]
    pub async fn hybrid_search(
        &self,
        query: &str,
        top_k: usize,
        scope: RetrievalScope,
    ) -> Result<Vec<(Uuid, f32)>, RagError> {
        // Kick off both searches concurrently.
        let bm25_fut = self.search_bm25(query, top_k, scope);
        let dense_fut = self.search(query, top_k, scope);

        let (bm25_res, dense_res) = tokio::join!(bm25_fut, dense_fut);

        // Propagate BM25 errors as-is.
        let bm25_list = bm25_res?;

        // Dense errors are mapped to RagError::Embed since we're in strict mode.
        let dense_list =
            dense_res.map_err(|e| RagError::Embed(format!("dense search failed: {:?}", e)))?;

        // Fuse with configurable, weighted RRF and stable UUID tie-breaking.
        let mut out: Vec<(Uuid, f32)> = rrf_fuse(&bm25_list, &dense_list, &RrfConfig::default());
        out.truncate(top_k);

        debug!("Hybrid search returning {} fused results", out.len());
        Ok(out)
    }
}
