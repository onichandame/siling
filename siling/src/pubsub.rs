use std::pin::Pin;

use futures::{stream_select, Stream, StreamExt};
use thiserror::Error;

use crate::{
    event::{Event, EventAdaptor},
    task::TaskId,
};

#[derive(Clone)]
pub struct Pubsub<TAdaptor: EventAdaptor> {
    adaptor: TAdaptor,
    channel: (async_channel::Sender<Event>, async_channel::Receiver<Event>),
}

#[derive(Error, Debug)]
pub enum PubsubError<TBroadcastError> {
    #[error("Broadcast Error: {0}")]
    BroadcastError(#[source] TBroadcastError),
    #[error(transparent)]
    NarrowcastError(#[from] async_channel::SendError<Event>),
}

impl<TAdaptor: EventAdaptor> Pubsub<TAdaptor> {
    pub fn new(adaptor: TAdaptor) -> Self {
        let channel = async_channel::bounded(16);
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
        self.channel.0.send(event).await?;
        Ok(())
    }

    pub async fn subscribe(
        &mut self,
        id: Option<TaskId>,
    ) -> Result<Pin<Box<dyn Stream<Item = Event>>>, PubsubError<TAdaptor::Error>> {
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
