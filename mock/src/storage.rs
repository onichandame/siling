use std::sync::Arc;

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

#[derive(Clone)]
pub struct MockStorageAdaptor {
    inner: Arc<Mutex<InnerAdaptor>>,
}

#[derive(Default)]
struct InnerAdaptor {
    pending_list: Vec<StoredPendingTask>,
    claimed_list: Vec<StoredClaimedTask>,
    acked_list: Vec<StoredAckedTask>,
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
        &self,
        input: String,
        config: TaskConfig,
    ) -> Result<Option<PendingTask>, Self::Error> {
        let lists = &mut *self.inner.lock().await;
        if lists.pending_list.iter().any(|task| &input == &task.input) {
            return Ok(None);
        }
        let mature_at = if let Some(mature_at) = config.mature_at.as_ref() {
            mature_at.to_owned()
        } else {
            Self::now()
        };
        let ind = lists
            .pending_list
            .iter()
            .position(|task| &task.mature_at > &mature_at)
            .unwrap_or(0);
        let task_id = uuid::Uuid::new_v4().to_string();
        lists.pending_list.splice(
            ind..ind,
            vec![StoredPendingTask {
                task_id: task_id.clone(),
                mature_at: mature_at.clone(),
                input,
            }],
        );
        Ok(Some(if mature_at > Self::now() {
            ImmatureTask { task_id, mature_at }.into()
        } else {
            MatureTask { task_id }.into()
        }))
    }

    async fn claim_task(&self) -> Result<Option<ClaimResult<String>>, Self::Error> {
        let lists = &mut *self.inner.lock().await;
        Ok(if let Some(task) = lists.pending_list.first() {
            let task = task.clone();
            Some(if &task.mature_at <= &Self::now() {
                let claim_id = uuid::Uuid::new_v4().to_string();
                lists.pending_list.remove(0);
                lists.claimed_list.push(StoredClaimedTask {
                    task_id: task.task_id.clone(),
                    claim_id: claim_id.clone(),
                    input: task.input.clone(),
                    claimed_at: Self::now(),
                });
                ClaimedTask {
                    task_id: task.task_id.clone(),
                    claim_id,
                    input: task.input.clone(),
                }
                .into()
            } else {
                ImmatureTask {
                    task_id: task.task_id.clone(),
                    mature_at: task.mature_at.clone(),
                }
                .into()
            })
        } else {
            None
        })
    }

    async fn revoke(&self, ttl: chrono::Duration) -> Result<(), Self::Error> {
        let lists = &mut *self.inner.lock().await;
        while let Some(task) = lists.claimed_list.first() {
            let task = task.to_owned();
            if task.claimed_at - ttl < Self::now() {
                lists.claimed_list.remove(0);
                if !lists.pending_list.iter().any(|v| v.input == task.input) {
                    let mature_at = Self::now();
                    let ind = lists
                        .pending_list
                        .iter()
                        .position(|v| v.mature_at >= mature_at)
                        .unwrap_or(0);
                    lists.pending_list.splice(
                        ind..ind,
                        vec![StoredPendingTask {
                            task_id: task.task_id,
                            input: task.input,
                            mature_at,
                        }],
                    );
                }
            } else {
                break;
            }
        }
        Ok(())
    }

    async fn ack_task(
        &self,
        claim: ClaimedTask<String>,
        output: String,
    ) -> Result<AckedTask<String>, Self::Error> {
        let lists = &mut *self.inner.lock().await;
        if let Some((ind, task)) = lists
            .claimed_list
            .iter()
            .enumerate()
            .find(|(_, task)| task.claim_id == claim.claim_id)
        {
            let task = task.clone();
            lists.claimed_list.remove(ind);
            lists.acked_list.push(StoredAckedTask {
                task_id: task.task_id.clone(),
                output: output.clone(),
                acked_at: Self::now(),
            });
            Ok(AckedTask {
                task_id: task.task_id.clone(),
                output: output.clone(),
            })
        } else {
            Err(Error::InvalidClaimer(claim.claim_id))
        }
    }

    async fn find_task(&self, id: TaskId) -> Result<Option<Task<String, String>>, Self::Error> {
        let lists = &*self.inner.lock().await;
        Ok(
            if let Some(task) = lists.pending_list.iter().find(|task| task.task_id == id) {
                let task = if task.mature_at <= Self::now() {
                    MatureTask {
                        task_id: task.task_id.clone(),
                    }
                    .into()
                } else {
                    ImmatureTask {
                        task_id: task.task_id.clone(),
                        mature_at: task.mature_at.clone(),
                    }
                    .into()
                };
                Some(Task::Pending(task))
            } else if let Some(task) = lists.claimed_list.iter().find(|task| task.task_id == id) {
                Some(Task::Claimed(ClaimedTask {
                    task_id: task.task_id.clone(),
                    claim_id: task.claim_id.clone(),
                    input: task.input.clone(),
                }))
            } else if let Some(task) = lists.acked_list.iter().find(|task| task.task_id == id) {
                Some(Task::Acked(AckedTask {
                    task_id: task.task_id.clone(),
                    output: task.output.clone(),
                }))
            } else {
                None
            },
        )
    }

    async fn cleanup(&self, ttl: chrono::Duration) -> Result<(), Self::Error> {
        let lists = &mut *self.inner.lock().await;
        while let Some(task) = lists.acked_list.first() {
            let task = task.to_owned();
            if task.acked_at + ttl < Self::now() {
                lists.acked_list.remove(0);
            } else {
                break;
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
struct StoredPendingTask {
    task_id: TaskId,
    input: String,
    mature_at: chrono::NaiveDateTime,
}

#[derive(Clone)]
struct StoredClaimedTask {
    task_id: TaskId,
    claim_id: ClaimId,
    input: String,
    claimed_at: chrono::NaiveDateTime,
}

#[derive(Clone)]
struct StoredAckedTask {
    task_id: TaskId,
    output: String,
    acked_at: chrono::NaiveDateTime,
}

impl MockStorageAdaptor {
    fn now() -> chrono::NaiveDateTime {
        chrono::Utc::now().naive_utc()
    }
}

#[cfg(test)]
use siling::test_storage_adaptor;
#[cfg(test)]
test_storage_adaptor!(MockStorageAdaptor::new());
