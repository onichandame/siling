pub trait BroadcasterError: std::error::Error + Send + Sync + Clone {}

impl<T: std::error::Error + Send + Sync + Clone> BroadcasterError for T {}
