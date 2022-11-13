use std::{marker::PhantomData, pin::Pin};

use futures::{future::OptionFuture, select, stream, FutureExt, Stream, StreamExt};
use futures_timer::Delay;
use siling_traits::{
    AckedTask, Argument, ClaimResult, ClaimedTask, Event, EventAdaptor, ImmatureTask, PendingTask,
    StorageAdaptor, Task, TaskConfig, TaskId,
};

use crate::pubsub::Pubsub;

use self::error::QueueError;

mod config;
mod error;

pub use config::QueueConfig;

/// A queue receives and dispatches a single type of tasks
#[derive(Clone)]
pub struct Queue<
    TInput: Argument,
    TOutput: Argument,
    TStorage: StorageAdaptor,
    TEvent: EventAdaptor,
> {
    config: QueueConfig,
    storage: TStorage,
    pubsub: Pubsub<TEvent>,
    input: PhantomData<TInput>,
    output: PhantomData<TOutput>,
}

impl<TInput: Argument, TOutput: Argument, TStorage: StorageAdaptor, TEvent: EventAdaptor>
    Queue<TInput, TOutput, TStorage, TEvent>
{
    pub fn new(storage: TStorage, event: TEvent, config: QueueConfig) -> Self {
        Self {
            config,
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
        config: TaskConfig,
    ) -> Result<Option<PendingTask>, QueueError<TStorage::Error, TEvent::Error>> {
        let task = self
            .storage
            .add_task(serde_json::to_string(&input)?, config)
            .await
            .map_err(|e| QueueError::StorageError(e))?;
        if let Some(task) = task.as_ref() {
            self.pubsub
                .broadcast(Event::TaskAdded(task.get_task_id().clone()))
                .await
                .map_err(|e| QueueError::EventError(e))?;
            if let PendingTask::Mature(task) = task {
                self.pubsub
                    .broadcast(Event::TaskMaturated(task.task_id.clone()))
                    .await
                    .map_err(|e| QueueError::EventError(e))?;
            }
        }
        Ok(task)
    }

    /// Continuously pulls the oldest pending task from the queue. If there is no mature task in
    /// the queue, it will block until a task is maturated.
    pub async fn consume(
        self,
    ) -> Result<
        impl Stream<Item = Result<ClaimedTask<TInput>, QueueError<TStorage::Error, TEvent::Error>>>
            + Send,
        QueueError<TStorage::Error, TEvent::Error>,
    > {
        Ok(Box::pin(stream::try_unfold(
            self,
            move |mut this| async move {
                let task = this.try_pull().await?;
                if let Some(ClaimResult::Claimed(task)) = task {
                    return Ok(Some((task, this)));
                }
                let mut ticker: Pin<Box<OptionFuture<_>>> = Box::pin(
                    if let Some(ClaimResult::Immature(task)) = task {
                        Some(Self::wait_for_task(task.clone()).fuse())
                    } else {
                        None
                    }
                    .into(),
                );
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
                                Event::TaskMaturated(_) => {
                                    let task = this.try_pull().await?;
                                    if let Some(ClaimResult::Claimed(task)) = task {
                                        return Ok(Some((task, this)));
                                    } else if let Some(ClaimResult::Immature(task)) = task {
                                        ticker = Box::pin(
                                            Some(Self::wait_for_task(task.clone()).fuse()).into()
                                        );
                                    }
                                }
                                _others => {}
                            }
                        }
                    };
                }
            },
        )))
    }

    /// Report the output of a task to the queue and mark it as acknowledged
    pub async fn ack(
        &mut self,
        claim: ClaimedTask<TInput>,
        output: TOutput,
    ) -> Result<(), QueueError<TStorage::Error, TEvent::Error>> {
        self.storage
            .ack_task(
                Self::reverse_claim(&claim)?,
                serde_json::to_string(&output)?,
            )
            .await
            .map_err(|e| QueueError::StorageError(e))?;
        self.pubsub
            .broadcast(Event::TaskAcked(claim.task_id.clone()))
            .await?;
        if let Some(ttl) = self.config.acked_task_ttl.as_ref() {
            self.storage
                .cleanup(ttl.clone())
                .await
                .map_err(|e| QueueError::StorageError(e))?;
        }
        Ok(())
    }

    /// Await a task to be acked
    pub async fn await_task(
        &mut self,
        task_id: TaskId,
    ) -> Result<AckedTask<TOutput>, QueueError<TStorage::Error, TEvent::Error>> {
        let mut ticker = Box::pin(async move {}.fuse());
        let mut subscription = self.pubsub.subscribe(Some(task_id.clone())).await?.fuse();
        loop {
            select! {
                _=ticker=>{
                    if let Some(Task::Acked(task))=self.storage.find_task(task_id.clone()).await.map_err(|e|QueueError::StorageError(e))?{
                        return Ok(Self::parse_acked(task)?);
                    }
                },
                event=subscription.select_next_some()=>{
                    if let Event::TaskAcked(_) = event{
                        if let Some(Task::Acked(task))=self.storage.find_task(task_id.clone()).await.map_err(|e|QueueError::StorageError(e))?{
                            return Ok(Self::parse_acked(task)?);
                        }else{
                            return Err(QueueError::Unknown(format!("task {} acked but not found",&task_id)));
                        }
                    }
                }
            }
        }
    }

    async fn try_pull(
        &mut self,
    ) -> Result<Option<ClaimResult<TInput>>, QueueError<TStorage::Error, TEvent::Error>> {
        if let Some(ttl) = self.config.idle_claim_ttl.as_ref() {
            self.storage
                .revoke(ttl.clone())
                .await
                .map_err(|e| QueueError::StorageError(e))?;
        }
        let task = self
            .storage
            .claim_task()
            .await
            .map_err(|e| QueueError::StorageError(e))?;
        Ok(match task {
            Some(task) => Some(match task {
                ClaimResult::Claimed(task) => Self::parse_claim(&task)?.into(),
                ClaimResult::Immature(task) => task.into(),
            }),
            None => None,
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

    fn parse_claim(claim: &ClaimedTask<String>) -> Result<ClaimedTask<TInput>, serde_json::Error> {
        Ok(ClaimedTask {
            task_id: claim.task_id.clone(),
            claim_id: claim.claim_id.clone(),
            input: serde_json::from_str(&claim.input)?,
        })
    }

    fn reverse_claim(
        claim: &ClaimedTask<TInput>,
    ) -> Result<ClaimedTask<String>, serde_json::Error> {
        Ok(ClaimedTask {
            task_id: claim.task_id.clone(),
            claim_id: claim.claim_id.clone(),
            input: serde_json::to_string(&claim.input)?,
        })
    }

    fn parse_acked(acked: AckedTask<String>) -> Result<AckedTask<TOutput>, serde_json::Error> {
        Ok(AckedTask {
            task_id: acked.task_id,
            output: serde_json::from_str(&acked.output)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::SinkExt;
    use serde::{Deserialize, Serialize};
    use siling_mock::{event::MockEventAdaptor, storage::MockStorageAdaptor};
    use siling_traits::TaskConfig;

    #[derive(Serialize, Deserialize, Clone)]
    struct Input {
        param: String,
    }

    impl From<String> for Input {
        fn from(param: String) -> Self {
            Self { param }
        }
    }

    #[derive(Serialize, Deserialize, Clone)]
    struct Output {
        output: String,
    }

    impl From<String> for Output {
        fn from(output: String) -> Self {
            Self { output }
        }
    }

    #[tokio::test]
    async fn can_push_tasks() {
        let mut queue = get_queue(QueueConfig::default());
        let param = uuid::Uuid::new_v4().to_string();
        assert!(queue
            .push(param.clone().into(), TaskConfig::default())
            .await
            .unwrap()
            .is_some());
        assert!(queue
            .push(
                param.clone().into(),
                TaskConfig::default()
                    .mature_at(chrono::Utc::now().naive_utc() + chrono::Duration::seconds(100)),
            )
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn can_push_then_consume() {
        let mut queue = get_queue(QueueConfig::default());
        let param = uuid::Uuid::new_v4().to_string();
        let pending = queue
            .push(param.clone().into(), TaskConfig::default())
            .await
            .unwrap()
            .unwrap();
        let claims = consume_queue(&queue, 1).await;
        let task = claims.first().unwrap();
        assert_eq!(&task.task_id, pending.get_task_id());
        assert_eq!(&task.input.param, &param);
    }

    #[tokio::test]
    async fn can_consume_then_push() {
        let mut queue = get_queue(QueueConfig::default());
        let param = uuid::Uuid::new_v4().to_string();
        let queue2 = queue.clone();
        let (mut sender, mut recver) = futures::channel::mpsc::channel(0);
        tokio::spawn(async move {
            let claims = consume_queue(&queue2, 1).await;
            sender
                .send(claims.first().unwrap().to_owned())
                .await
                .unwrap();
        });
        tokio::time::sleep(chrono::Duration::milliseconds(100).to_std().unwrap()).await;
        let pending = queue
            .push(param.clone().into(), TaskConfig::default())
            .await
            .unwrap()
            .unwrap();
        let task = recver.next().await.unwrap();
        assert_eq!(&task.task_id, pending.get_task_id());
        assert_eq!(&task.input.param, &param);
    }

    #[tokio::test]
    async fn can_push_delayed_then_consume() {
        let delay = chrono::Duration::seconds(1);
        let mut queue = get_queue(QueueConfig::default().claim_ttl(chrono::Duration::seconds(1)));
        let param = uuid::Uuid::new_v4().to_string();
        let push_date = chrono::Utc::now().naive_utc();
        let pending = queue
            .push(
                param.clone().into(),
                TaskConfig::default().mature_at(chrono::Utc::now().naive_utc() + delay),
            )
            .await
            .unwrap()
            .unwrap();
        let claims = consume_queue(&queue, 1).await;
        let claim_date = chrono::Utc::now().naive_utc();
        let task = claims.first().unwrap();
        assert_eq!(&task.task_id, pending.get_task_id());
        assert_eq!(&task.input.param, &param);
        assert!(claim_date >= push_date + delay);
    }

    #[tokio::test]
    async fn can_reclaim_timed_out_claim() {
        let timeout = chrono::Duration::seconds(1);
        let mut queue = get_queue(QueueConfig::default().claim_ttl(timeout.clone()));
        let param = uuid::Uuid::new_v4().to_string();
        let pending = queue
            .push(param.clone().into(), TaskConfig::default())
            .await
            .unwrap()
            .unwrap();
        consume_queue(&queue, 1).await;
        tokio::time::sleep(
            (timeout + chrono::Duration::milliseconds(100))
                .to_std()
                .unwrap(),
        )
        .await;
        let claims = consume_queue(&queue, 1).await;
        let task = claims.first().unwrap();
        assert_eq!(&task.task_id, pending.get_task_id());
        assert_eq!(&task.input.param, &param);
    }

    #[tokio::test]
    async fn can_ack_claimed_task() {
        let mut queue = get_queue(QueueConfig::default());
        let param = uuid::Uuid::new_v4().to_string();
        let output = uuid::Uuid::new_v4().to_string();
        queue
            .push(param.clone().into(), TaskConfig::default())
            .await
            .unwrap()
            .unwrap();
        let claims = consume_queue(&queue, 1).await;
        let claim = claims.first().unwrap();
        queue
            .ack(claim.to_owned(), output.clone().into())
            .await
            .unwrap();
        assert!(queue
            .ack(claim.to_owned(), output.clone().into())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn can_await_pending_task() {
        let mut queue = get_queue(QueueConfig::default());
        let param = uuid::Uuid::new_v4().to_string();
        let output = uuid::Uuid::new_v4().to_string();
        let pending = queue
            .push(param.clone().into(), TaskConfig::default())
            .await
            .unwrap()
            .unwrap();
        let mut queue2 = queue.clone();
        let pending2 = pending.clone();
        let (mut sender, mut recver) = futures::channel::mpsc::channel(0);
        tokio::spawn(async move {
            sender
                .send(
                    queue2
                        .await_task(pending2.get_task_id().to_owned())
                        .await
                        .unwrap(),
                )
                .await
                .unwrap();
        });
        tokio::time::sleep(chrono::Duration::milliseconds(100).to_std().unwrap()).await;
        let claims = consume_queue(&queue, 1).await;
        let claim = claims.first().unwrap();
        queue
            .ack(claim.to_owned(), output.clone().into())
            .await
            .unwrap();
        let acked = recver.next().await.unwrap();
        assert_eq!(&acked.task_id, pending.get_task_id());
        assert_eq!(&acked.output.output, &output);
    }

    #[tokio::test]
    async fn can_await_claimed_task() {
        let mut queue = get_queue(QueueConfig::default());
        let param = uuid::Uuid::new_v4().to_string();
        let output = uuid::Uuid::new_v4().to_string();
        let pending = queue
            .push(param.clone().into(), TaskConfig::default())
            .await
            .unwrap()
            .unwrap();
        let claims = consume_queue(&queue, 1).await;
        let claim = claims.first().unwrap();
        let mut queue2 = queue.clone();
        let pending2 = pending.clone();
        let (mut sender, mut recver) = futures::channel::mpsc::channel(0);
        tokio::spawn(async move {
            sender
                .send(
                    queue2
                        .await_task(pending2.get_task_id().to_owned())
                        .await
                        .unwrap(),
                )
                .await
                .unwrap();
        });
        tokio::time::sleep(chrono::Duration::milliseconds(100).to_std().unwrap()).await;
        queue
            .ack(claim.to_owned(), output.clone().into())
            .await
            .unwrap();
        let acked = recver.next().await.unwrap();
        assert_eq!(&acked.task_id, pending.get_task_id());
        assert_eq!(&acked.output.output, &output);
    }

    #[tokio::test]
    async fn can_await_acked_task() {
        let mut queue = get_queue(QueueConfig::default());
        let param = uuid::Uuid::new_v4().to_string();
        let output = uuid::Uuid::new_v4().to_string();
        let pending = queue
            .push(param.clone().into(), TaskConfig::default())
            .await
            .unwrap()
            .unwrap();
        let claims = consume_queue(&queue, 1).await;
        let claim = claims.first().unwrap();
        queue
            .ack(claim.to_owned(), output.clone().into())
            .await
            .unwrap();
        let acked = queue
            .await_task(pending.get_task_id().to_owned())
            .await
            .unwrap();
        assert_eq!(&acked.task_id, pending.get_task_id());
        assert_eq!(&acked.output.output, &output);
    }

    fn get_queue(
        config: QueueConfig,
    ) -> Queue<Input, Output, MockStorageAdaptor, MockEventAdaptor> {
        Queue::new(MockStorageAdaptor::new(), MockEventAdaptor::new(), config)
    }

    async fn consume_queue(
        queue: &Queue<Input, Output, MockStorageAdaptor, MockEventAdaptor>,
        len: usize,
    ) -> Vec<ClaimedTask<Input>> {
        let mut result = vec![];
        let mut consumer = queue.to_owned().consume().await.unwrap().fuse();
        if len > 0 {
            while let Ok(task) = consumer.select_next_some().await {
                result.push(task);
                if result.len() >= len {
                    break;
                }
            }
        }
        if result.len() != len {
            panic!(
                "expect to claim {} tasks but received {}",
                len,
                result.len()
            );
        }
        result
    }
}
