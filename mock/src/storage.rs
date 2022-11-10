use async_trait::async_trait;
use futures::lock::Mutex;
use siling::{
    claim::{ClaimId, ClaimResult},
    storage::StorageAdaptor,
    task::{
        AckedTask, ClaimedTask, ImmatureTask, MatureTask, PendingTask, Task, TaskConfig, TaskId,
    },
};
use thiserror::Error;

pub struct MockStorageAdaptor {
    inner: Mutex<InnerAdaptor>,
}

#[derive(Default)]
struct InnerAdaptor {
    pending_list: Vec<(TaskId, chrono::NaiveDateTime, String)>,
    claimed_list: Vec<(TaskId, ClaimId, chrono::NaiveDateTime, String)>,
    acked_list: Vec<(TaskId, chrono::NaiveDateTime, String)>,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("task reported by an invalid claimer({0})")]
    InvalidClaimer(ClaimId),
}

impl MockStorageAdaptor {
    pub fn new() -> Self {
        Self {
            inner: Default::default(),
        }
    }
}

#[async_trait]
impl StorageAdaptor for MockStorageAdaptor {
    type Error = Error;

    async fn add_task(
        &mut self,
        input: String,
        config: Option<TaskConfig>,
    ) -> Result<Option<PendingTask>, Self::Error> {
        let lists = self.inner.get_mut();
        if lists.pending_list.iter().any(
            |(_, _, task_input)| {
                if task_input == &input {
                    true
                } else {
                    false
                }
            },
        ) {
            return Ok(None);
        }
        let mature_at = if let Some(config) = config {
            if let Some(mature_at) = config.mature_at {
                mature_at
            } else {
                Self::now()
            }
        } else {
            Self::now()
        };
        let ind = lists
            .pending_list
            .iter()
            .position(
                |(_, mature_at2, _)| {
                    if mature_at2 > &mature_at {
                        true
                    } else {
                        false
                    }
                },
            )
            .unwrap_or(0);
        let task_id = uuid::Uuid::new_v4().to_string();
        lists
            .pending_list
            .splice(ind..ind, vec![(task_id.clone(), mature_at.clone(), input)]);
        Ok(Some(if mature_at > Self::now() {
            PendingTask::Immature(ImmatureTask { task_id, mature_at })
        } else {
            PendingTask::Mature(MatureTask { task_id })
        }))
    }

    async fn claim_task(&mut self) -> Result<ClaimResult<String>, Self::Error> {
        let lists = self.inner.get_mut();
        Ok(
            if let Some((task_id, mature_at, input)) = lists.pending_list.first() {
                if mature_at <= &Self::now() {
                    let claim_id = uuid::Uuid::new_v4().to_string();
                    lists.pending_list.remove(0);
                    lists.claimed_list.push((
                        task_id.to_owned(),
                        claim_id.clone(),
                        Self::now(),
                        input.to_owned(),
                    ));
                    ClaimResult::Claimed(ClaimedTask {
                        task_id: task_id.to_owned(),
                        claim_id: uuid::Uuid::new_v4().to_string(),
                        input: input.to_owned(),
                    })
                } else {
                    ClaimResult::Immature(ImmatureTask {
                        task_id: task_id.to_owned(),
                        mature_at: mature_at.to_owned(),
                    })
                }
            } else {
                ClaimResult::None
            },
        )
    }

    async fn ack_task(
        &mut self,
        claim: ClaimedTask<String>,
        output: String,
    ) -> Result<AckedTask<String>, Self::Error> {
        let lists = self.inner.get_mut();
        if let Some((ind, (task_id, claim_id, claimd_at, input))) = lists
            .claimed_list
            .iter()
            .enumerate()
            .find(|(ind, (_, claim_id, _, _))| {
                if claim_id == &claim.claim_id {
                    true
                } else {
                    false
                }
            })
        {
            let acked_at = Self::now();
            lists.claimed_list.splice(ind..ind, vec![]);
            lists
                .acked_list
                .push((task_id.to_owned(), acked_at.clone(), output.clone()));
            Ok(AckedTask {
                task_id: task_id.to_owned(),
                output: output.clone(),
            })
        } else {
            Err(Error::InvalidClaimer(claim.claim_id))
        }
    }

    async fn find_task(&mut self, id: TaskId) -> Result<Option<Task<String, String>>, Self::Error> {
        let lists = self.inner.get_mut();
        Ok(
            if let Some((task_id, mature_at, _)) = lists
                .pending_list
                .iter()
                .find(|(task_id, _, _)| task_id == &id)
            {
                let task = if mature_at <= &Self::now() {
                    PendingTask::Mature(MatureTask {
                        task_id: task_id.to_owned(),
                    })
                } else {
                    PendingTask::Immature(ImmatureTask {
                        task_id: task_id.to_owned(),
                        mature_at: mature_at.to_owned(),
                    })
                };
                Some(Task::Pending(task))
            } else {
                None
            },
        )
    }
    /// Delete old acked task and revoke timed-out claimes
    async fn cleanup(&mut self, ttl: chrono::Duration) -> Result<(), Self::Error> {
        let lists = self.inner.get_mut();
        Ok(())
    }
}

impl MockStorageAdaptor {
    fn now() -> chrono::NaiveDateTime {
        chrono::Utc::now().naive_utc()
    }
}
