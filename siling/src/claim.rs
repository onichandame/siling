use crate::{
    argument::Argument,
    task::{ClaimedTask, ImmatureTask},
};

pub type ClaimId = String;

#[derive(Clone)]
pub enum ClaimResult<TInput: Argument> {
    /// Only immature tasks are found in the queue
    Immature(ImmatureTask),
    /// A mature task has been claimed
    Claimed(ClaimedTask<TInput>),
}

impl<TInput: Argument> From<ImmatureTask> for ClaimResult<TInput> {
    fn from(task: ImmatureTask) -> Self {
        Self::Immature(task)
    }
}

impl<TInput: Argument> From<ClaimedTask<TInput>> for ClaimResult<TInput> {
    fn from(task: ClaimedTask<TInput>) -> Self {
        Self::Claimed(task)
    }
}
