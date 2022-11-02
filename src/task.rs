use serde::{de::DeserializeOwned, Serialize};

pub type TaskId = String;

pub trait Argument: Serialize + DeserializeOwned {}

pub enum Task<TInput: Argument, TOutput: Argument> {
    Pending { id: TaskId },
    Claimed(ClaimedTask<TInput>),
    Acked(AckedTask<TOutput>),
}

pub struct ClaimedTask<TInput: Argument> {
    pub id: TaskId,
    pub input: TInput,
}

pub struct AckedTask<TOutput: Argument> {
    pub id: TaskId,
    pub output: TOutput,
}
