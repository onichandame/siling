use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;

use crate::task::TaskId;

pub trait EventError: std::error::Error + Send + Sync {}

#[async_trait]
pub trait EventAdaptor: Send + Sync {
    type Error: EventError;
    /// Broadcast an Event associating with a task to all subscribers.
    async fn publish(&self, event: Event) -> Result<(), Self::Error>;
    /// Create a subscriber to all events of a task.
    async fn subscribe(
        &self,
        id: Option<TaskId>,
    ) -> Result<Pin<Box<dyn Stream<Item = Event>>>, Self::Error>;
}

#[derive(Clone)]
pub enum Event {
    TaskAdded(TaskId),
    TaskMaturated(TaskId),
    TaskClaimed(TaskId),
    TaskAcked(TaskId),
}

impl Event {
    pub fn get_task_id(&self) -> &TaskId {
        match self {
            Event::TaskAcked(id) => id,
            Event::TaskAdded(id) => id,
            Event::TaskClaimed(id) => id,
            Event::TaskMaturated(id) => id,
        }
    }
}
