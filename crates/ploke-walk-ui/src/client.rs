use std::future::Future;

use ploke_eval::walk_client::{
    AdvertisedStep, EvaluationRunCoordinate, EvaluationTraceIndex, EvaluationTraceSnapshot,
    LlmTraceCoordinate, LlmTraceIndex, LlmTraceSnapshot, OperationId, WalkClient,
    WalkConfigSnapshot, WalkEvidenceQuery, WalkPhase, WalkQuerySnapshot, WalkResponse,
    WalkStartConfig,
};
use ploke_eval::{
    setup_client::{
        CampaignId, RunSetupPreview, RunSetupReceipt, RunSetupRequest, admit_run_setup,
        preview_run_setup,
    },
    spec::PrepareError,
};

use crate::model::{
    DB_QUERY_TIMEOUT, OPERATION_REQUEST_TIMEOUT, TRACE_REQUEST_TIMEOUT, WALK_REQUEST_TIMEOUT,
    WalkRequestKind, WalkRequestResult,
};

pub(crate) fn trace_index(client: WalkClient) -> Result<EvaluationTraceIndex, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create tokio runtime: {error}"))?;
    runtime.block_on(async move {
        tokio::time::timeout(TRACE_REQUEST_TIMEOUT, client.evaluation_trace_index())
            .await
            .map_err(|_| {
                format!(
                    "completed-run index timed out after {}s",
                    TRACE_REQUEST_TIMEOUT.as_secs()
                )
            })?
            .map_err(|error| error.to_string())
    })
}

pub(crate) fn trace(
    client: WalkClient,
    coordinate: EvaluationRunCoordinate,
) -> Result<EvaluationTraceSnapshot, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create tokio runtime: {error}"))?;
    runtime.block_on(async move {
        tokio::time::timeout(TRACE_REQUEST_TIMEOUT, client.evaluation_trace(coordinate))
            .await
            .map_err(|_| {
                format!(
                    "evaluation trace timed out after {}s",
                    TRACE_REQUEST_TIMEOUT.as_secs()
                )
            })?
            .map_err(|error| error.to_string())
    })
}

pub(crate) fn llm_index(client: WalkClient) -> Result<LlmTraceIndex, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create tokio runtime: {error}"))?;
    runtime.block_on(async move {
        tokio::time::timeout(TRACE_REQUEST_TIMEOUT, client.llm_trace_index())
            .await
            .map_err(|_| {
                format!(
                    "live LLM index timed out after {}s",
                    TRACE_REQUEST_TIMEOUT.as_secs()
                )
            })?
            .map_err(|error| error.to_string())
    })
}

pub(crate) fn llm_trace(
    client: WalkClient,
    coordinate: LlmTraceCoordinate,
) -> Result<LlmTraceSnapshot, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create tokio runtime: {error}"))?;
    runtime.block_on(async move {
        tokio::time::timeout(TRACE_REQUEST_TIMEOUT, client.llm_trace(coordinate))
            .await
            .map_err(|_| {
                format!(
                    "live LLM observation timed out after {}s",
                    TRACE_REQUEST_TIMEOUT.as_secs()
                )
            })?
            .map_err(|error| error.to_string())
    })
}

pub(crate) fn query_db(
    client: WalkClient,
    campaign: &str,
    script: &str,
) -> Result<WalkQuerySnapshot, String> {
    let campaign = CampaignId::from(campaign);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create tokio runtime: {error}"))?;
    runtime.block_on(async move {
        tokio::time::timeout(DB_QUERY_TIMEOUT, client.query_db(Some(&campaign), script))
            .await
            .map_err(|_| {
                format!(
                    "database query timed out after {}s",
                    DB_QUERY_TIMEOUT.as_secs()
                )
            })?
            .map_err(|error| error.to_string())
    })
}

pub(crate) fn query_evidence(
    client: WalkClient,
    campaign: &str,
    view: WalkEvidenceQuery,
) -> Result<WalkQuerySnapshot, String> {
    let campaign = CampaignId::from(campaign);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create tokio runtime: {error}"))?;
    runtime.block_on(async move {
        tokio::time::timeout(
            DB_QUERY_TIMEOUT,
            client.query_evidence(Some(&campaign), view),
        )
        .await
        .map_err(|_| {
            format!(
                "database evidence query timed out after {}s",
                DB_QUERY_TIMEOUT.as_secs()
            )
        })?
        .map_err(|error| error.to_string())
    })
}

pub(crate) fn config(client: WalkClient) -> Result<WalkConfigSnapshot, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create tokio runtime: {error}"))?;
    runtime.block_on(async move {
        tokio::time::timeout(DB_QUERY_TIMEOUT, client.config())
            .await
            .map_err(|_| {
                format!(
                    "configuration request timed out after {}s",
                    DB_QUERY_TIMEOUT.as_secs()
                )
            })?
            .map_err(|error| error.to_string())
    })
}

pub(crate) fn preview_setup(request: &RunSetupRequest) -> Result<RunSetupPreview, String> {
    preview_run_setup(request).map_err(|error| error.to_string())
}

pub(crate) fn admit_setup(
    request: &RunSetupRequest,
    expected: &ploke_eval::walk_client::ContentHash,
) -> Result<RunSetupReceipt, String> {
    admit_run_setup(request, expected).map_err(|error| error.to_string())
}

pub(crate) fn start(
    client: WalkClient,
    campaign: Option<String>,
    target: WalkPhase,
    allow_live: bool,
    operation: OperationId,
) -> Result<WalkResponse, String> {
    run_operation(async move {
        client
            .start_advertised(
                WalkStartConfig {
                    campaign: campaign.map(CampaignId::from),
                    repo_root: None,
                },
                target,
                allow_live,
                operation,
            )
            .await
    })
}

pub(crate) fn step(
    client: WalkClient,
    advertised: AdvertisedStep,
    operation: OperationId,
) -> Result<WalkResponse, String> {
    run_operation(async move { client.step_advertised(&advertised, operation).await })
}

pub(crate) fn operation(
    client: WalkClient,
    operation: OperationId,
) -> Result<WalkResponse, String> {
    run_operation(async move { client.operation_status(operation).await })
}

pub(crate) fn stop_idle(client: WalkClient) -> Result<WalkResponse, String> {
    run_operation(async move { client.stop_idle_server().await })
}

fn run_operation(
    future: impl Future<Output = Result<WalkResponse, PrepareError>>,
) -> Result<WalkResponse, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create tokio runtime: {error}"))?;
    runtime.block_on(async move {
        tokio::time::timeout(OPERATION_REQUEST_TIMEOUT, future)
            .await
            .map_err(|_| {
                format!(
                    "walk operation request timed out after {}s",
                    OPERATION_REQUEST_TIMEOUT.as_secs()
                )
            })?
            .map_err(|error| error.to_string())
    })
}

pub(crate) fn run_walk_request(kind: WalkRequestKind, client: WalkClient) -> WalkRequestResult {
    let socket = client.socket().to_path_buf();
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            return WalkRequestResult::ClientError(format!(
                "failed to create tokio runtime: {error}"
            ));
        }
    };

    runtime.block_on(async move {
        match kind {
            WalkRequestKind::Health => {
                match tokio::time::timeout(WALK_REQUEST_TIMEOUT, client.health()).await {
                    Ok(Ok(Some(response))) => WalkRequestResult::Response(Box::new(response)),
                    Ok(Ok(None)) => WalkRequestResult::Offline(socket),
                    Ok(Err(error)) => WalkRequestResult::ClientError(error.to_string()),
                    Err(_) => WalkRequestResult::TimedOut,
                }
            }
            WalkRequestKind::Show => {
                match tokio::time::timeout(WALK_REQUEST_TIMEOUT, client.health()).await {
                    Ok(Ok(Some(_))) => {}
                    Ok(Ok(None)) => return WalkRequestResult::Offline(socket),
                    Ok(Err(error)) => return WalkRequestResult::ClientError(error.to_string()),
                    Err(_) => return WalkRequestResult::TimedOut,
                }
                match tokio::time::timeout(WALK_REQUEST_TIMEOUT, client.show()).await {
                    Ok(Ok(response)) => WalkRequestResult::Response(Box::new(response)),
                    Ok(Err(error)) => WalkRequestResult::ClientError(error.to_string()),
                    Err(_) => WalkRequestResult::TimedOut,
                }
            }
        }
    })
}
