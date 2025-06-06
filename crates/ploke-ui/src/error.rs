use std::sync::mpsc;

use ploke_error::Error;

use crate::ProcessingStatus;

#[derive(Debug, thiserror::Error)]
pub enum UiError {
    // ... existing variants ...
    #[error("Thread communication failed: {0}")]
    TrySendError(#[from] flume::TrySendError<ProcessingStatus>),
    #[error("Thread communication failed: {0}")]
    SendError(#[from] flume::SendError<ProcessingStatus>),
    // #[error("JoinHandle Failed")]
    // JoinHandle(#[from] std::thread::JoinHandle<Result<(), &dyn std::error::Error>>),
    // #[error("Channel error with flume: {0}")]
}

impl From<UiError> for Error {
    fn from(value: UiError) -> Self {
        Error::UiError( format!("{:?}", value) )
    }
}
