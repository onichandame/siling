use std::{marker::PhantomData, time::Duration};

use event::{Event, EventAdaptor, EventError};
use futures::{select, stream, StreamExt};
use futures_timer::Delay;
use task::{AckedTask, Argument, ClaimedTask, Task, TaskId};
use thiserror::Error;

use storage::{StorageAdaptor, StorageError};

use crate::storage::StoredTaskStatus;

mod event;
mod storage;
mod task;

/// A queue receives and dispatches a single type of tasks
pub struct Queue<
    TStorage: StorageAdaptor,
    TEvent: EventAdaptor,
    TInput: Argument,
    TOutput: Argument,
> {
    storage: TStorage,
    event: TEvent,
    input: PhantomData<TInput>,
    output: PhantomData<TOutput>,
}

#[derive(Error, Debug)]
pub enum QueueError {
    #[error(transparent)]
    StorageError(#[from] StorageError),
    #[error(transparent)]
    EventError(#[from] EventError),
    #[error(transparent)]
    ValueError(#[from] serde_json::Error),
    #[error("task {0} error: {1}")]
    TaskError(TaskId, String),
    #[error("task {0} not found")]
    TaskNotFound(TaskId),
    #[error("unknown queue error: {0}")]
    Unknown(String),
}

impl<TStorage: StorageAdaptor, TEvent: EventAdaptor, TInput: Argument, TOutput: Argument>
    Queue<TStorage, TEvent, TInput, TOutput>
{
    pub fn new(storage: TStorage, event: TEvent) -> Self {
        Self {
            storage,
            event,
            input: PhantomData,
            output: PhantomData,
        }
    }

    /// Push new task to the current queue
    pub async fn add(&self, input: TInput) -> Result<TaskId, QueueError> {
        let id = self.storage.add_task(serde_json::to_value(input)?).await?;
        self.event.broadcast(Event::TaskAdded(id.clone())).await?;
        Ok(id)
    }

    /// Get input of a task and mark the task as claimed
    pub async fn claim(&self) -> Result<ClaimedTask<TInput>, QueueError> {
        let (id, input) = self.storage.claim_task().await?;
        let input = serde_json::from_value(input)?;
        self.event.broadcast(Event::TaskClaimed(id.clone())).await?;
        Ok(ClaimedTask { id, input })
    }

    /// Report the output of a task to the queue and mark it as acknowledged
    pub async fn ack(&self, task: AckedTask<TOutput>) -> Result<(), QueueError> {
        self.storage
            .ack_task(task.id.clone(), serde_json::to_value(task.output)?)
            .await?;
        self.event
            .broadcast(Event::TaskAcked(task.id.clone()))
            .await?;
        Ok(())
    }

    /// Get the current status of a task
    pub async fn fetch(&self, id: TaskId) -> Result<Option<Task<TInput, TOutput>>, QueueError> {
        let stored_task = self.storage.find_task(id.clone()).await?;
        Ok(if let Some(task) = stored_task {
            Some(match task.status {
                StoredTaskStatus::Pending => Task::Pending { id },
                StoredTaskStatus::Claimed => Task::Claimed(ClaimedTask {
                    id: id.clone(),
                    input: serde_json::from_value(task.input)?,
                }),
                StoredTaskStatus::Acked => Task::Acked(AckedTask {
                    id: id.clone(),
                    output: serde_json::from_value(task.output.ok_or(QueueError::TaskError(
                        id.clone(),
                        "output missing".to_owned(),
                    ))?)?,
                }),
            })
        } else {
            None
        })
    }

    /// Blocking for the result of a task
    pub async fn wait(&self, id: TaskId) -> Result<AckedTask<TOutput>, QueueError> {
        let task = self.fetch(id.clone()).await?;
        let handle_event = || async {
            let task = self
                .fetch(id.clone())
                .await?
                .ok_or(QueueError::TaskNotFound(id.clone()))?;
            if let Task::Acked(task) = task {
                return Ok(Some(task));
            }
            Ok::<Option<AckedTask<TOutput>>, QueueError>(None)
        };
        match task {
            Some(task) => match task {
                Task::Pending { id: _ } | Task::Claimed(_) => {
                    let mut subscriber = self.event.subscribe(id.clone()).await?.fuse();
                    let mut ticker = Box::pin(
                        stream::unfold(5, |timeout| async move {
                            Delay::new(Duration::from_secs(timeout)).await;
                            let max_timeout = 60;
                            let next_timeout = std::cmp::min(timeout * 2, max_timeout);
                            Some(((), next_timeout))
                        })
                        .fuse(),
                    );
                    select! {
                        event = subscriber.select_next_some() => {
                            if let Event::TaskAcked(_) = event {
                                if let Some(task) = handle_event().await? {
                                    return Ok(task);
                                }
                            }
                        },
                        _ = ticker.select_next_some() => {
                            if let Some(task) = handle_event().await? {
                                return Ok(task);
                            }
                        }
                    };
                    Err(QueueError::Unknown("subscription failed".to_owned()))
                }
                Task::Acked(task) => Ok(task),
            },
            None => Err(QueueError::TaskNotFound(id.clone())),
        }
    }
}
