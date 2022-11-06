use crate::{
    argument::Argument,
    task::{ClaimedTask, ImmatureTask},
};

pub type ClaimId = String;

#[derive(Clone)]
pub struct ClaimConfig {
    /// The duration after which the inactive claims should be revoked
    pub timeout: Option<chrono::Duration>,
}

#[derive(Clone)]
pub enum ClaimResult<TInput: Argument> {
    /// Only immature tasks are found in the queue
    Immature(ImmatureTask),
    /// A mature task has been claimed
    Claimed(ClaimedTask<TInput>),
    /// The pending list is empty
    None,
}
