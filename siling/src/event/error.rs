pub trait EventError: std::error::Error + Send + Sync + Clone {}

impl<T: std::error::Error + Send + Sync + Clone> EventError for T {}
