use proc_macro2::TokenStream;
use quote::quote;

pub fn expand_storage_tests(factory: TokenStream) -> TokenStream {
    quote! {
        #[cfg(test)]
        mod tests {
            use super::*;
            use futures::{StreamExt,TryStreamExt,stream,join};
            use siling::{storage::StorageAdaptor,task::{TaskConfig,PendingTask,Task},claim::ClaimResult};

            #[tokio::test]
            async fn can_add_task() {
                let input="asdf".to_owned();
                let storage=#factory;
                let pending=storage.add_task(input.clone(),TaskConfig::default()).await.unwrap();
                assert!(pending.is_some());
                let pending=storage.add_task(input.clone(),TaskConfig::default().mature_at(now())).await.unwrap();
                assert!(pending.is_none());
            }

            #[tokio::test]
            async fn can_add_task_then_consume() {
                let input="asdf".to_owned();
                let storage=#factory;
                if storage.claim_task().await.unwrap().is_some(){
                    panic!("task queue not started empty");
                }
                if let Some(pending)=storage.add_task(input.clone(),Default::default()).await.unwrap(){
                    if let Some(ClaimResult::Claimed(task))=storage.claim_task().await.unwrap(){
                        assert_eq!(task.input, input);
                        assert_eq!(&task.task_id, pending.get_task_id());
                    }else{
                        panic!("task not consumed");
                    }
                    if storage.claim_task().await.unwrap().is_some(){
                        panic!("claimed task not cleared from pending list");
                    }
                }else{
                    panic!("task not added");
                }
            }

            #[tokio::test]
            async fn can_add_delayed_task_then_consume() {
                let input="asdf".to_owned();
                let delay=chrono::Duration::seconds(1);
                let storage=#factory;
                if let Some(pending)=storage.add_task(input.clone(),TaskConfig::default().mature_at(now()+delay)).await.unwrap(){
                    if let PendingTask::Immature(immature)=pending{
                        if let Some(ClaimResult::Immature(task))=storage.claim_task().await.unwrap(){
                            assert_eq!(task.task_id,immature.task_id);
                            tokio::time::sleep((delay+chrono::Duration::milliseconds(100)).to_std().unwrap()).await;
                            if let Some(ClaimResult::Claimed(task))=storage.claim_task().await.unwrap(){
                                assert_eq!(task.task_id,immature.task_id);
                            }else{
                                panic!("delayed task not maturated after specified time");
                            }
                        }else{
                            panic!("immature task not found");
                        }
                    }else{
                        panic!("delayed task not added as immature");
                    }
                }else{
                    panic!("task not added");
                }
            }

            #[tokio::test]
            async fn can_revoke_timed_out_claims(){
                let input="asdf".to_owned();
                let ttl=chrono::Duration::seconds(1);
                let storage=#factory;
                if let Some(pending)=storage.add_task(input.clone(),TaskConfig::default()).await.unwrap(){
                    storage.revoke(ttl).await.unwrap();
                    if let Some(ClaimResult::Claimed(claim1))=storage.claim_task().await.unwrap(){
                        assert_eq!(&claim1.task_id,pending.get_task_id());
                        tokio::time::sleep((ttl+chrono::Duration::milliseconds(100)).to_std().unwrap()).await;
                        storage.revoke(ttl).await.unwrap();
                        if let Some(ClaimResult::Claimed(claim2))=storage.claim_task().await.unwrap(){
                            assert_eq!(&claim2.task_id,pending.get_task_id());
                        }else{
                            panic!("task not claimed after revoke");
                        }
                    }else{
                        panic!("task not claimed");
                    }
                }else{
                    panic!("task not added");
                }
            }

            #[tokio::test]
            async fn can_ack_claims(){
                let input="asdf".to_owned();
                let output="qwer".to_owned();
                let storage=#factory;
                if let Some(pending)=storage.add_task(input.clone(),TaskConfig::default()).await.unwrap(){
                    if let Some(ClaimResult::Claimed(claim))=storage.claim_task().await.unwrap(){
                        let task=storage.ack_task(claim.clone(),output.clone()).await.unwrap();
                        assert_eq!(task.output,output);
                        assert_eq!(&task.task_id,pending.get_task_id());
                        assert!(storage.ack_task(claim.clone(),output.clone()).await.is_err());
                    }else{
                        panic!("task not claimed");
                    }
                }else{
                    panic!("task not added");
                }
            }

            #[tokio::test]
            async fn can_find_tasks(){
                let input="asdf".to_owned();
                let output="qwer".to_owned();
                let storage=#factory;
                if let Some(pending)=storage.add_task(input.clone(),TaskConfig::default()).await.unwrap(){
                    let task=storage.find_task(pending.get_task_id().to_owned()).await.unwrap().unwrap();
                    assert_eq!(task.get_task_id(),pending.get_task_id());
                    assert!(matches!(task,Task::Pending(_)));
                    if let Some(ClaimResult::Claimed(claim))=storage.claim_task().await.unwrap(){
                        let task=storage.find_task(claim.task_id.clone()).await.unwrap().unwrap();
                        assert_eq!(task.get_task_id(),&claim.task_id);
                        assert!(matches!(task,Task::Claimed(_)));
                        let acked=storage.ack_task(claim,output.clone()).await.unwrap();
                        if let Task::Acked(task)=storage.find_task(acked.task_id.clone()).await.unwrap().unwrap(){
                            assert_eq!(acked.task_id,task.task_id);
                            assert_eq!(acked.output,task.output);
                        }else{
                            panic!("acked task not found");
                        }
                    }else{
                        panic!("task not claimed");
                    }
                }else{
                    panic!("task not added");
                }
            }

            #[tokio::test]
            async fn can_cleanup_acked_tasks(){
                let input="asdf".to_owned();
                let output="qwer".to_owned();
                let ttl=chrono::Duration::seconds(1);
                let storage=#factory;
                storage.add_task(input.clone(),TaskConfig::default()).await.unwrap();
                if let Some(ClaimResult::Claimed(claim))=storage.claim_task().await.unwrap(){
                    let task=storage.ack_task(claim.clone(),output.clone()).await.unwrap();
                    storage.cleanup(ttl.clone()).await.unwrap();
                    assert_eq!(storage.find_task(task.task_id.clone()).await.unwrap().unwrap().get_task_id(),&claim.task_id);
                    tokio::time::sleep((ttl+chrono::Duration::milliseconds(100)).to_std().unwrap()).await;
                    storage.cleanup(ttl.clone()).await.unwrap();
                    assert!(storage.find_task(task.task_id.clone()).await.unwrap().is_none());
                }else{
                    panic!("task not claimed");
                }
            }

            fn now()->chrono::NaiveDateTime{
                chrono::Utc::now().naive_utc()
            }
        }
    }
}
