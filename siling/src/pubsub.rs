use std::pin::Pin;

use futures::{stream_select, Stream, StreamExt};
use thiserror::Error;

use siling_traits::{BroadcasterAdaptor, Event, TaskId};

#[derive(Clone)]
pub struct Pubsub<TAdaptor: BroadcasterAdaptor> {
    adaptor: TAdaptor,
    channel: (
        async_broadcast::Sender<Event>,
        async_broadcast::Receiver<Event>,
    ),
}

#[derive(Error, Debug)]
pub enum PubsubError<TBroadcastError> {
    #[error("Broadcast Error: {0}")]
    BroadcastError(#[source] TBroadcastError),
    #[error(transparent)]
    NarrowcastError(#[from] async_broadcast::SendError<Event>),
}

impl<TAdaptor: BroadcasterAdaptor> Pubsub<TAdaptor> {
    pub fn new(adaptor: TAdaptor) -> Self {
        let channel = async_broadcast::broadcast(16);
        Self { adaptor, channel }
    }

    pub async fn broadcast(&mut self, event: Event) -> Result<(), PubsubError<TAdaptor::Error>> {
        Ok(self
            .adaptor
            .publish(event)
            .await
            .map_err(|e| PubsubError::BroadcastError(e))?)
    }

    pub async fn narrowcast(&mut self, event: Event) -> Result<(), PubsubError<TAdaptor::Error>> {
        self.channel.0.broadcast(event).await?;
        Ok(())
    }

    pub async fn subscribe(
        &mut self,
        id: Option<TaskId>,
    ) -> Result<Pin<Box<dyn Stream<Item = Event> + Send>>, PubsubError<TAdaptor::Error>> {
        let broad = self
            .adaptor
            .subscribe(id.clone())
            .await
            .map_err(|e| PubsubError::BroadcastError(e))?;
        let narrow = Box::pin(self.channel.1.clone().filter(move |v| {
            let id = id.clone();
            let v = v.clone();
            async move {
                if let Some(id) = id.as_ref() {
                    if id != v.get_task_id() {
                        return false;
                    }
                }
                true
            }
        }));
        Ok(Box::pin(stream_select!(broad, narrow)))
    }
}
