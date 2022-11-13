#[macro_export]
macro_rules! test_events {
    ($factory: expr) => {
        #[tokio::test]
        async fn can_publish() {
            use siling::prelude::*;
            let adaptor = $factory;
            let subscriber = adaptor.clone();
            let subscription = async move { subscriber.subscribe(None).await.unwrap() };
            adaptor
                .publish(Event::TaskAdded("".to_owned()))
                .await
                .unwrap();
            subscription.await;
        }
    };
}
