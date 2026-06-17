//! Length-prefixed JSON IPC helpers for walk client/server messages.
//!
//! Unix sockets are byte streams, so every serde JSON payload is framed as a
//! four-byte little-endian length followed by exactly that many payload bytes.
//! This borrows Zellij's explicit framing discipline without adopting protobuf
//! for this private debug-only protocol.

use std::path::Path;

use serde::{Serialize, de::DeserializeOwned};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};

use crate::spec::PrepareError;

/// Hard cap for one request or response frame.
const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// Connect to an existing walk socket.
pub(crate) async fn connect(path: &Path) -> Result<UnixStream, PrepareError> {
    UnixStream::connect(path)
        .await
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_connect",
            detail: format!(
                "failed to connect to walk socket '{}': {source}",
                path.display()
            ),
        })
}

/// Serialize and send one framed JSON value.
pub(crate) async fn send<T: Serialize>(
    stream: &mut UnixStream,
    value: &T,
) -> Result<(), PrepareError> {
    let body = serde_json::to_vec(value).map_err(PrepareError::Serialize)?;
    if body.len() > MAX_FRAME_BYTES {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "prototype1-state-walk IPC frame too large: {} bytes > {}",
                body.len(),
                MAX_FRAME_BYTES
            ),
        });
    }
    let len = body.len() as u32;
    stream
        .write_all(&len.to_le_bytes())
        .await
        .map_err(write_error)?;
    stream.write_all(&body).await.map_err(write_error)?;
    stream.flush().await.map_err(write_error)?;
    Ok(())
}

/// Receive and deserialize one framed JSON value.
pub(crate) async fn recv<T: DeserializeOwned>(stream: &mut UnixStream) -> Result<T, PrepareError> {
    let mut len_bytes = [0_u8; 4];
    stream
        .read_exact(&mut len_bytes)
        .await
        .map_err(read_error)?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "prototype1-state-walk IPC frame too large: {len} bytes > {MAX_FRAME_BYTES}"
            ),
        });
    }
    let mut body = vec![0_u8; len];
    stream.read_exact(&mut body).await.map_err(read_error)?;
    serde_json::from_slice(&body).map_err(PrepareError::Serialize)
}

fn read_error(source: std::io::Error) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_ipc_read",
        detail: source.to_string(),
    }
}

fn write_error(source: std::io::Error) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_ipc_write",
        detail: source.to_string(),
    }
}
