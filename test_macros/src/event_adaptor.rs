use proc_macro2::TokenStream;
use quote::quote;

pub fn expand_event_adaptor_tests(factory: TokenStream) -> TokenStream {
    quote! {
        #[cfg(test)]
        mod tests {
            use super::*;
            use futures::{StreamExt,TryStreamExt,stream,join};
            use siling::event::{EventAdaptor, Event};

            #[tokio::test]
            async fn can_broadcast_events() {
                let task_id="asdf".to_owned();
                let adaptor = #factory;
                let mut subscription1=adaptor.subscribe(None).await.unwrap().fuse();
                let subscriber1 = async move {
                    while let ev=subscription1.select_next_some().await{
                        return ev;
                    }
                    panic!("no event received");
                };
                let mut subscription2=adaptor.subscribe(None).await.unwrap().fuse();
                let subscriber2 = async move {
                    while let ev=subscription2.select_next_some().await{
                        return ev;
                    }
                    panic!("no event received");
                };
                adaptor.publish(Event::TaskAdded(task_id.clone())).await.unwrap();
                assert_eq!(subscriber1.await.get_task_id(), &task_id);
                assert_eq!(subscriber2.await.get_task_id(), &task_id);
            }
        }
    }
}
