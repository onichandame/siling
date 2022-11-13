use siling_traits::TaskId;
use thiserror::Error;

use crate::pubsub::PubsubError;

#[derive(Error, Debug)]
pub enum QueueError<TStorageError, TEventError> {
    #[error("Storage Error: {0}")]
    StorageError(#[source] TStorageError),
    #[error(transparent)]
    EventError(#[from] PubsubError<TEventError>),
    #[error(transparent)]
    ValueError(#[from] serde_json::Error),
    #[error("task {0} not found")]
    TaskNotFound(TaskId),
    #[error("unknown queue error: {0}")]
    Unknown(String),
}
