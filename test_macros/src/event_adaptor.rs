use proc_macro2::TokenStream;
use quote::quote;

pub fn expand_event_adaptor_tests(factory: TokenStream) -> TokenStream {
    quote! {
        #[cfg(test)]
        mod tests {
            use super::*;
            use futures::{StreamExt,TryStreamExt,stream,join};
            use siling::event::{EventAdaptor, Event};

            macro_rules! get_subscriptions {
                ($adaptor:ident,$len:expr)=>{
                    {
                        let mut subscriptions=vec![];
                        for _ in 1..$len{
                            subscriptions.push($adaptor.subscribe(None).await.unwrap().fuse());
                        }
                        subscriptions
                    }
                };
            }

            macro_rules! await_subscriptions {
                ($subscriptions:ident)=>{
                    {
                        let mut events=$subscriptions.into_iter().map(|mut subscription|{async move {
                            while let ev=subscription.select_next_some().await{
                                return ev;
                            }
                            panic!("no event received");
                        }}).collect::<Vec<_>>();
                    }
                };
            }

            #[tokio::test]
            async fn can_receive_events() {
                let task_id="asdf".to_owned();
                let adaptor = #factory;
                let mut subscription=adaptor.subscribe(None).await.unwrap().fuse();
                let received_event = async move {
                    while let ev=subscription.select_next_some().await{
                        return ev;
                    }
                    panic!("no event received");
                };
                adaptor.publish(Event::TaskAdded(task_id.clone())).await.unwrap();
                assert_eq!(received_event.await, Event::TaskAdded(task_id));
            }

            #[tokio::test]
            async fn can_broadcast_events() {
                let task_id="qwer".to_owned();
                let adaptor=#factory;
                let subscriptions=get_subscriptions!(adaptor,10);
                let events=await_subscriptions!(subscriptions);
            }

        }
    }
}
