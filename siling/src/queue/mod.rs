use std::{marker::PhantomData, time::Duration};

use chrono::format::Item;
use futures::{future::OptionFuture, select, stream, Future, FutureExt, Stream, StreamExt};
use futures_timer::Delay;

use crate::{
    argument::Argument,
    claim::{ClaimConfig, ClaimResult},
    event::{Event, EventAdaptor},
    pubsub::Pubsub,
    storage::StorageAdaptor,
    task::{ClaimedTask, ImmatureTask, PendingTask, TaskConfig},
};

use self::error::QueueError;

mod error;

/// A queue receives and dispatches a single type of tasks
#[derive(Clone)]
pub struct Queue<
    TInput: Argument,
    TOutput: Argument,
    TStorage: StorageAdaptor,
    TEvent: EventAdaptor,
> {
    storage: TStorage,
    pubsub: Pubsub<TEvent>,
    input: PhantomData<TInput>,
    output: PhantomData<TOutput>,
}

impl<TInput: Argument, TOutput: Argument, TStorage: StorageAdaptor, TEvent: EventAdaptor>
    Queue<TInput, TOutput, TStorage, TEvent>
{
    pub fn new(storage: TStorage, event: TEvent) -> Self {
        Self {
            storage,
            pubsub: Pubsub::new(event),
            input: PhantomData,
            output: PhantomData,
        }
    }

    /// Push new task to the current queue. No-op if the task already exists in the pending list.
    pub async fn push(
        &mut self,
        input: TInput,
        config: Option<TaskConfig>,
    ) -> Result<Option<PendingTask>, QueueError<TStorage::Error, TEvent::Error>> {
        let task = self
            .storage
            .add_task(serde_json::to_value(&input)?, config)
            .await
            .map_err(|e| QueueError::StorageError(e))?;
        if let Some(task) = task.as_ref() {
            self.pubsub
                .broadcast(Event::TaskAdded(task.get_task_id().clone()))
                .await
                .map_err(|e| QueueError::EventError(e))?;
        }
        Ok(task)
    }

    async fn test() -> Result<impl Stream<Item = Result<String, i32>>, i32> {
        Ok(stream::try_unfold(0, |next| async move { Err(next) }))
    }

    /// Continuously pull the oldest pending task from the queue. If there is no mature task in
    /// the queue, it will block until a task is maturated/added.
    pub async fn consume(
        self,
        config: Option<ClaimConfig>,
    ) -> Result<
        impl Stream<Item = Result<ClaimedTask<TInput>, QueueError<TStorage::Error, TEvent::Error>>>,
        QueueError<TStorage::Error, TEvent::Error>,
    > {
        Ok(stream::try_unfold(
            (self, None),
            move |(mut this, next_task)| {
                let config = config.clone();
                async move {
                    let task = this.try_pull(&config).await?;
                    if let ClaimResult::Claimed(task) = task {
                        return Ok(Some((task, (this, next_task))));
                    }
                    let mut ticker: OptionFuture<
                        Result<(), QueueError<TStorage::Error, TEvent::Error>>,
                    > = if let ClaimResult::Immature(task) = task {
                        Some(Self::wait_for_task(task.clone()))
                    } else {
                        None
                    }
                    .into();
                    let mut subscription = Box::pin(this.pubsub.subscribe(None).await?.fuse());
                    loop {
                        select! {
                            tick = ticker => {
                                if tick.is_some() {
                                    this.pubsub.narrowcast(Event::TaskMaturated("".to_owned())).await?;
                                }
                            }
                            ev = subscription.select_next_some() => {
                                match ev {
                                    Event::TaskAdded(_) | Event::TaskMaturated(_) => {
                                        let task = this.try_pull(&config).await?;
                                        if let ClaimResult::Claimed(task) = task {
                                            return Ok(Some((task,(this, next_task))));
                                        } else if let ClaimResult::Immature(task) = task {
                                            ticker = Box::pin(Self::wait_for_task(Some(task).clone()).fuse());
                                        }
                                    }
                                    _others => {}
                                }
                            }
                        };
                    }
                }
            },
        ))
    }

    async fn try_pull(
        &self,
        config: &Option<ClaimConfig>,
    ) -> Result<ClaimResult<TInput>, QueueError<TStorage::Error, TEvent::Error>> {
        let task = self
            .storage
            .claim_task(config)
            .await
            .map_err(|e| QueueError::StorageError(e))?;
        Ok(match task {
            ClaimResult::Claimed(task) => ClaimResult::Claimed(self.parse_claim(&task)?),
            ClaimResult::Immature(task) => ClaimResult::Immature(task),
            ClaimResult::None => ClaimResult::None,
        })
    }

    async fn wait_for_task(
        task: ImmatureTask,
    ) -> Result<(), QueueError<TStorage::Error, TEvent::Error>> {
        Delay::new(
            (task.mature_at - chrono::Utc::now().naive_utc())
                .to_std()
                .map_err(|e| QueueError::Unknown(e.to_string()))?,
        )
        .await;
        Ok(())
    }

    fn parse_claim(
        &self,
        claim: &ClaimedTask<serde_json::Value>,
    ) -> Result<ClaimedTask<TInput>, serde_json::Error> {
        Ok(ClaimedTask {
            task_id: claim.task_id.clone(),
            claim_id: claim.claim_id.clone(),
            input: serde_json::from_value(claim.input.clone())?,
        })
    }

    ///// Report the output of a task to the queue and mark it as acknowledged
    //pub async fn ack(&self, task: AckedTask<TOutput>) -> Result<(), QueueError> {
    //    self.storage
    //        .ack_task(task.id.clone(), serde_json::to_value(task.output)?)
    //        .await?;
    //    self.event
    //        .broadcast(Event::TaskAcked(task.id.clone()))
    //        .await?;
    //    Ok(())
    //}

    ///// Get the current status of a task
    //pub async fn fetch(&self, id: TaskId) -> Result<Option<Task<TInput, TOutput>>, QueueError> {
    //    let stored_task = self.storage.find_task(id.clone()).await?;
    //    Ok(if let Some(task) = stored_task {
    //        Some(match task.status {
    //            StoredTaskStatus::Pending => Task::Pending { id },
    //            StoredTaskStatus::Claimed => Task::Claimed(ClaimedTask {
    //                id: id.clone(),
    //                input: serde_json::from_value(task.input)?,
    //            }),
    //            StoredTaskStatus::Acked => Task::Acked(AckedTask {
    //                id: id.clone(),
    //                output: serde_json::from_value(task.output.ok_or(QueueError::TaskError(
    //                    id.clone(),
    //                    "output missing".to_owned(),
    //                ))?)?,
    //            }),
    //        })
    //    } else {
    //        None
    //    })
    //}

    ///// Blocking for the result of a task
    //pub async fn wait(&self, id: TaskId) -> Result<AckedTask<TOutput>, QueueError> {
    //    let task = self.fetch(id.clone()).await?;
    //    let handle_event = || async {
    //        let task = self
    //            .fetch(id.clone())
    //            .await?
    //            .ok_or(QueueError::TaskNotFound(id.clone()))?;
    //        if let Task::Acked(task) = task {
    //            return Ok(Some(task));
    //        }
    //        Ok::<Option<AckedTask<TOutput>>, QueueError>(None)
    //    };
    //    match task {
    //        Some(task) => match task {
    //            Task::Pending { id: _ } | Task::Claimed(_) => {
    //                let mut subscriber = self.event.subscribe(id.clone()).await?.fuse();
    //                let mut ticker = Box::pin(
    //                    stream::unfold(5, |timeout| async move {
    //                        Delay::new(Duration::from_secs(timeout)).await;
    //                        let max_timeout = 60;
    //                        let next_timeout = std::cmp::min(timeout * 2, max_timeout);
    //                        Some(((), next_timeout))
    //                    })
    //                    .fuse(),
    //                );
    //                select! {
    //                    event = subscriber.select_next_some() => {
    //                        if let Event::TaskAcked(_) = event {
    //                            if let Some(task) = handle_event().await? {
    //                                return Ok(task);
    //                            }
    //                        }
    //                    },
    //                    _ = ticker.select_next_some() => {
    //                        if let Some(task) = handle_event().await? {
    //                            return Ok(task);
    //                        }
    //                    }
    //                };
    //                Err(QueueError::Unknown("subscription failed".to_owned()))
    //            }
    //            Task::Acked(task) => Ok(task),
    //        },
    //        None => Err(QueueError::TaskNotFound(id.clone())),
    //    }
    //}
}