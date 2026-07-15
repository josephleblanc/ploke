use ploke_eval::walk_client::{
    EvaluationRunCoordinate, EvaluationTraceIndex, EvaluationTraceSnapshot, WalkClient,
    WalkQuerySnapshot,
};

use crate::model::{
    DB_QUERY_TIMEOUT, TRACE_REQUEST_TIMEOUT, WALK_REQUEST_TIMEOUT, WalkRequestKind,
    WalkRequestResult,
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

pub(crate) fn query_db(
    client: WalkClient,
    campaign: &str,
    script: &str,
) -> Result<WalkQuerySnapshot, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create tokio runtime: {error}"))?;
    runtime.block_on(async move {
        tokio::time::timeout(DB_QUERY_TIMEOUT, client.query_db(Some(campaign), script))
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
                    Ok(Ok(Some(response))) => WalkRequestResult::Response(response),
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
                    Ok(Ok(response)) => WalkRequestResult::Response(response),
                    Ok(Err(error)) => WalkRequestResult::ClientError(error.to_string()),
                    Err(_) => WalkRequestResult::TimedOut,
                }
            }
        }
    })
}
