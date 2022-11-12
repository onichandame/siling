#[derive(Clone, Default)]
pub struct QueueConfig {
    /// Maximun TTL of acked task. Note that the cleanup is lazy
    pub(crate) acked_task_ttl: Option<chrono::Duration>,
    /// Maximun TTL of an idle claim before it should be revoked. Note that the revoked claims
    /// cannot be acked anymore. Thus this ttl must be longer than the execution time of a task
    ///
    /// When set to None, no claim will be revoked. It is recommended to set a finite TTL.
    pub(crate) idle_claim_ttl: Option<chrono::Duration>,
}

impl QueueConfig {
    pub fn acked_ttl(mut self, ttl: chrono::Duration) -> Self {
        self.acked_task_ttl = Some(ttl);
        self
    }

    pub fn claim_ttl(mut self, ttl: chrono::Duration) -> Self {
        self.idle_claim_ttl = Some(ttl);
        self
    }
}
