use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransformError {
    #[error(transparent)]
    Cozo(#[from] cozo::Error),
}
