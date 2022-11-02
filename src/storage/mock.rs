use async_trait::async_trait;
use serde_json::Value;

use crate::task::TaskId;

use super::{StorageAdaptor, StorageError, StoredTask};

/// A local in-memory implementation of [super::StorageAdaptor]. The performance is HORRIBLE. This should only be used for unittests
pub struct MockStorageAdaptor {
    tasks: Vec<(TaskId, StoredTask)>,
    next_id: u64,
}

impl MockStorageAdaptor {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            tasks: vec![],
        }
    }
}

#[async_trait]
impl StorageAdaptor for MockStorageAdaptor {
    async fn add_task(&mut self, input: Value) -> Result<TaskId, StorageError> {
        let id = self.next_id;
        self.tasks.push((
            id,
            StoredTask {
                status: super::StoredTaskStatus::Pending,
                input,
                output: None,
            },
        ));
        Ok(id)
    }
}
