use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;

use crate::{event::Event, task::TaskId};

mod error;

pub use error::BroadcasterError;

#[async_trait]
pub trait BroadcasterAdaptor: Send + Sync + Clone {
    type Error: BroadcasterError;
    /// Broadcast an Event associating with a task to all subscribers.
    async fn publish(&self, event: Event) -> Result<(), Self::Error>;
    /// Create a subscriber to all events of a task.
    async fn subscribe(
        &self,
        id: Option<TaskId>,
    ) -> Result<Pin<Box<dyn Stream<Item = Event> + Send>>, Self::Error>;
}
