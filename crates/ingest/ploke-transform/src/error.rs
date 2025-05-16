use ploke_error::InternalError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransformError {
    #[error(transparent)]
    Internal(#[from] InternalError),
    
    #[error("Database operation failed: {0}")]
    Database(String),
    
    #[error("Schema validation failed: {0}")]
    SchemaValidation(String),
    
    #[error("Data transformation failed: {0}")]
    Transformation(String),
}

impl From<cozo::Error> for TransformError {
    fn from(err: cozo::Error) -> Self {
        TransformError::Database(err.to_string())
    }
}
