use super::*;

#[derive(Debug, Error, Clone)]
pub enum RecvError {
    #[error("Failed to send request to IO Manager")]
    SendError,
    #[error("Failed to receive response from IO Manager")]
    RecvError,
}

// Define the additional error variants locally since we can't edit ploke-error
#[derive(Debug, Error, Clone)]
pub enum IoError {
    #[error("IO channel error")]
    Recv(#[from] RecvError),

    #[error("File content changed since indexing: {path}")]
    ContentMismatch {
        name: String,
        id: uuid::Uuid,
        file_tracking_hash: uuid::Uuid,
        namespace: uuid::Uuid,
        path: PathBuf,
    },

    #[error("Parse error in {path}: {message}")]
    ParseError { path: PathBuf, message: String },

    #[error(
        "Requested byte range {start_byte}..{end_byte} out of range for file {path} (length {file_len})"
    )]
    OutOfRange {
        path: PathBuf,
        start_byte: usize,
        end_byte: usize,
        file_len: usize,
    },

    // Other existing variants...
    #[error("Shutdown initiated")]
    ShutdownInitiated,

    #[error("File operation {operation} failed for {path}: {source} (kind: {kind:?})")]
    FileOperation {
        operation: &'static str,
        path: PathBuf,
        source: Arc<std::io::Error>,
        kind: std::io::ErrorKind,
    },

    #[error("UTF-8 decoding error in {path}: {source}")]
    Utf8 {
        path: PathBuf,
        source: std::string::FromUtf8Error,
    },

    #[error("Invalid UTF-8 boundaries in {path}: indices {start_byte}..{end_byte}")]
    InvalidCharBoundary {
        path: PathBuf,
        start_byte: usize,
        end_byte: usize,
    },
}

impl From<IoError> for ploke_error::Error {
    fn from(e: IoError) -> ploke_error::Error {
        use IoError::*;
        match e {
            ContentMismatch {
                name,
                id,
                file_tracking_hash,
                namespace,
                path,
            } => ploke_error::Error::Fatal(FatalError::ContentMismatch {
                name,
                id,
                file_tracking_hash,
                namespace,
                path,
            }),

            ParseError { path, message } => ploke_error::Error::Fatal(FatalError::SyntaxError(
                format!("Parse error in {}: {}", path.display(), message),
            )),

            OutOfRange {
                path,
                start_byte,
                end_byte,
                file_len,
            } => ploke_error::Error::Fatal(FatalError::FileOperation {
                operation: "read",
                path,
                source: Arc::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "Byte range {}-{} exceeds file length {}",
                        start_byte, end_byte, file_len
                    ),
                )),
            }),

            ShutdownInitiated => ploke_error::Error::Fatal(FatalError::ShutdownInitiated),

            FileOperation {
                operation,
                path,
                source,
                kind,
            } => ploke_error::Error::Fatal(FatalError::FileOperation {
                operation,
                path,
                source,
            }),

            Utf8 { path, source } => ploke_error::Error::Fatal(FatalError::Utf8 { path, source }),
            InvalidCharBoundary {
                path,
                start_byte,
                end_byte,
            } => {
                // Create a FromUtf8Error to capture the decoding failure
                let err_msg = format!(
                    "InvalidCharacterBoundary: Byte range {}-{} splits multi-byte Unicode character in file {}",
                    start_byte, end_byte, path.to_string_lossy()
                );

                ploke_error::Error::Fatal(FatalError::SyntaxError(err_msg))
            }
            Recv(recv_error) => ploke_error::Error::Internal(
                ploke_error::InternalError::CompilerError(recv_error.to_string()),
            ),
        }
    }
}
