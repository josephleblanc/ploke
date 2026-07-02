use ploke_eval::walk_client::{DbQueryResult, WalkClient};

use crate::model::{WALK_REQUEST_TIMEOUT, WalkRequestKind, WalkRequestResult};

pub(crate) fn query_campaign_db(campaign: &str, script: &str) -> Result<DbQueryResult, String> {
    WalkClient::query_campaign_db(campaign, script).map_err(|error| error.to_string())
}

pub(crate) fn run_walk_request(kind: WalkRequestKind, client: WalkClient) -> WalkRequestResult {
    let socket = client.socket().to_path_buf();
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            return WalkRequestResult::Error(format!("failed to create tokio runtime: {error}"));
        }
    };

    runtime.block_on(async move {
        match kind {
            WalkRequestKind::Health => {
                match tokio::time::timeout(WALK_REQUEST_TIMEOUT, client.health()).await {
                    Ok(Ok(Some(snapshot))) => WalkRequestResult::Snapshot(snapshot),
                    Ok(Ok(None)) => WalkRequestResult::Offline(socket),
                    Ok(Err(error)) => WalkRequestResult::Error(error.to_string()),
                    Err(_) => WalkRequestResult::TimedOut,
                }
            }
            WalkRequestKind::Show => {
                match tokio::time::timeout(WALK_REQUEST_TIMEOUT, client.health()).await {
                    Ok(Ok(Some(_))) => {}
                    Ok(Ok(None)) => return WalkRequestResult::Offline(socket),
                    Ok(Err(error)) => return WalkRequestResult::Error(error.to_string()),
                    Err(_) => return WalkRequestResult::TimedOut,
                }
                match tokio::time::timeout(WALK_REQUEST_TIMEOUT, client.show()).await {
                    Ok(Ok(snapshot)) => WalkRequestResult::Snapshot(snapshot),
                    Ok(Err(error)) => WalkRequestResult::Error(error.to_string()),
                    Err(_) => WalkRequestResult::TimedOut,
                }
            }
        }
    })
}
