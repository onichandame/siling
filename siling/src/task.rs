use crate::{argument::Argument, claim::ClaimId};

pub type TaskId = String;

pub enum Task<TInput: Argument, TOutput: Argument> {
    Pending(PendingTask),
    Claimed(ClaimedTask<TInput>),
    Acked(AckedTask<TOutput>),
}

/// Configuration of a task about to be added
pub struct TaskConfig {
    /// The datetime(UTC) after which the task is mature for execution
    pub mature_at: Option<chrono::NaiveDateTime>,
}

pub enum PendingTask {
    Mature(MatureTask),
    Immature(ImmatureTask),
}

pub struct MatureTask {
    pub task_id: TaskId,
}

#[derive(Clone)]
pub struct ImmatureTask {
    pub task_id: TaskId,
    pub mature_at: chrono::NaiveDateTime,
}

#[derive(Clone)]
pub struct ClaimedTask<TInput: Argument> {
    pub task_id: TaskId,
    pub claim_id: ClaimId,
    pub input: TInput,
}

pub struct AckedTask<TOutput: Argument> {
    pub task_id: TaskId,
    pub output: TOutput,
}

impl PendingTask {
    pub fn get_task_id(&self) -> &TaskId {
        match self {
            PendingTask::Mature(task) => &task.task_id,
            PendingTask::Immature(task) => &task.task_id,
        }
    }
}
