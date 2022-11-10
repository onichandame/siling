use async_trait::async_trait;

use crate::{
    claim::ClaimResult,
    task::{AckedTask, ClaimedTask, PendingTask, Task, TaskConfig, TaskId},
};

pub use self::error::StorageError;

mod error;

#[async_trait]
pub trait StorageAdaptor: Send + Sync {
    type Error: StorageError;
    /// Append a task to the queue. No-op if the same task already exists in the pending list.
    ///
    /// Returns the id of the new task or None if the task already exists
    async fn add_task(
        &mut self,
        input: String,
        config: Option<TaskConfig>,
    ) -> Result<Option<PendingTask>, Self::Error>;
    /// Try to claim a pending task for consumption
    async fn claim_task(&mut self) -> Result<ClaimResult<String>, Self::Error>;
    /// Report to the queue the output of a finished task. Returns Err if the task is not found or already acknowledged.
    async fn ack_task(
        &mut self,
        claim: ClaimedTask<String>,
        output: String,
    ) -> Result<AckedTask<String>, Self::Error>;
    /// Get the current state of a task
    async fn find_task(&mut self, id: TaskId) -> Result<Option<Task<String, String>>, Self::Error>;
    /// Delete old acked task and revoke timed-out claimes
    async fn cleanup(&mut self, ttl: chrono::Duration) -> Result<(), Self::Error>;
}
