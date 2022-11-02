use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

use crate::task::TaskId;

#[cfg(feature = "mock")]
mod mock;

#[async_trait]
pub trait StorageAdaptor {
    /// Append a task to the queue. No-op if the same task already exists.
    async fn add_task(&self, input: Value) -> Result<TaskId, StorageError>;
    /// Claim a pending task for consumption. The status is shifted to Claimed.
    async fn claim_task(&self) -> Result<(TaskId, Value), StorageError>;
    /// Report back the output of a task. Returns Err if the task is not found or already acknowledged.
    async fn ack_task(&self, id: TaskId, output: Value) -> Result<(), StorageError>;
    /// Get the current state of a task.
    async fn find_task(&self, id: TaskId) -> Result<Option<StoredTask>, StorageError>;
}

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("unknown storage error: {0}")]
    Unknown(String),
}

pub enum StoredTaskStatus {
    Pending,
    Claimed,
    Acked,
}

pub struct StoredTask {
    pub status: StoredTaskStatus,
    pub input: Value,
    pub output: Option<Value>,
}
