use std::pin::Pin;

use async_channel::{Receiver, SendError, Sender};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use siling::{
    event::{Event, EventAdaptor},
    task::TaskId,
};
use thiserror::Error;

#[derive(Clone)]
pub struct MockEventAdaptor {
    channel: (Sender<Event>, Receiver<Event>),
}

#[derive(Error, Debug, Clone)]
pub enum Error {
    #[error(transparent)]
    SendError(#[from] SendError<Event>),
}

impl MockEventAdaptor {
    pub fn new() -> Self {
        let channel = async_channel::bounded(8);
        Self { channel }
    }
}

#[async_trait]
impl EventAdaptor for MockEventAdaptor {
    type Error = Error;
    async fn publish(&self, event: Event) -> Result<(), Self::Error> {
        self.channel.0.send(event).await?;
        Ok(())
    }

    async fn subscribe(
        &self,
        id: Option<TaskId>,
    ) -> Result<Pin<Box<dyn Stream<Item = Event>>>, Self::Error> {
        Ok(Box::pin(self.channel.1.clone().filter(move |event| {
            let id = id.clone();
            let event = event.clone();
            async move {
                if let Some(id) = id.as_ref() {
                    if id != event.get_task_id() {
                        return false;
                    }
                }
                true
            }
        })))
    }
}

#[cfg(test)]
mod tests {
    use siling::test_events;

    test_events!(MockEventAdaptor::new());
}
