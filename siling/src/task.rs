use crate::{argument::Argument, claim::ClaimId};

pub type TaskId = String;

pub enum Task<TInput: Argument, TOutput: Argument> {
    Pending(PendingTask),
    Claimed(ClaimedTask<TInput>),
    Acked(AckedTask<TOutput>),
}

/// Configuration of a task about to be added
#[derive(Default)]
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

impl<TInput: Argument, TOutput: Argument> Task<TInput, TOutput> {
    pub fn get_task_id(&self) -> &TaskId {
        match self {
            Task::Pending(task) => task.get_task_id(),
            Task::Claimed(task) => &task.task_id,
            Task::Acked(task) => &task.task_id,
        }
    }
}

impl PendingTask {
    pub fn get_task_id(&self) -> &TaskId {
        match self {
            PendingTask::Mature(task) => &task.task_id,
            PendingTask::Immature(task) => &task.task_id,
        }
    }
}

impl From<ImmatureTask> for PendingTask {
    fn from(task: ImmatureTask) -> Self {
        Self::Immature(task)
    }
}

impl From<MatureTask> for PendingTask {
    fn from(task: MatureTask) -> Self {
        Self::Mature(task)
    }
}

impl<TInput: Argument, TOutput: Argument> From<PendingTask> for Task<TInput, TOutput> {
    fn from(task: PendingTask) -> Self {
        Self::Pending(task)
    }
}

impl<TInput: Argument, TOutput: Argument> From<ClaimedTask<TInput>> for Task<TInput, TOutput> {
    fn from(task: ClaimedTask<TInput>) -> Self {
        Self::Claimed(task)
    }
}

impl<TInput: Argument, TOutput: Argument> From<AckedTask<TOutput>> for Task<TInput, TOutput> {
    fn from(task: AckedTask<TOutput>) -> Self {
        Self::Acked(task)
    }
}

impl TaskConfig {
    pub fn mature_at(mut self, mature_at: chrono::NaiveDateTime) -> Self {
        self.mature_at = Some(mature_at);
        self
    }
}
