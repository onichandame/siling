use async_trait::async_trait;
use serde_json::Value;

use crate::{
    claim::{ClaimConfig, ClaimResult},
    task::{AckedTask, ClaimedTask, PendingTask, Task, TaskConfig, TaskId},
};

pub trait StorageError: std::error::Error + Send + Sync {}

#[async_trait]
pub trait StorageAdaptor: Send + Sync {
    type Error: StorageError;
    /// Append a task to the queue. No-op if the same task already exists in the pending list.
    ///
    /// Returns the id of the new task or None if the task already exists
    async fn add_task(
        &self,
        input: Value,
        config: Option<TaskConfig>,
    ) -> Result<Option<PendingTask>, Self::Error>;
    /// Try to claim a pending task for consumption
    async fn claim_task(
        &self,
        config: &Option<ClaimConfig>,
    ) -> Result<ClaimResult<Value>, Self::Error>;
    /// Renew a claim to prevent it being revoked and re-claimed by another claimer
    async fn renew_claim(
        &self,
        claim: &ClaimedTask<Value>,
    ) -> Result<ClaimedTask<Value>, Self::Error>;
    /// Report to the queue the output of a finished task. Returns Err if the task is not found or already acknowledged.
    async fn ack_task(
        &self,
        claim: ClaimedTask<Value>,
        output: Value,
    ) -> Result<AckedTask<Value>, Self::Error>;
    /// Get the current state of a task
    async fn find_task(&self, id: TaskId) -> Result<Option<Task<Value, Value>>, Self::Error>;
}
