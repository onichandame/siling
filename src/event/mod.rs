use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use thiserror::Error;

use crate::task::TaskId;

#[async_trait]
pub trait EventAdaptor {
    /// Broadcast an Event associating with a task to all online subscribers.
    async fn broadcast(&self, event: Event) -> Result<(), EventError>;
    /// Create a subscriber to all events of a task.
    async fn subscribe(&self, id: TaskId)
        -> Result<Pin<Box<dyn Stream<Item = Event>>>, EventError>;
}

pub enum Event {
    TaskAdded(TaskId),
    TaskClaimed(TaskId),
    TaskAcked(TaskId),
}

#[derive(Error, Debug)]
pub enum EventError {}
