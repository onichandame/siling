pub trait StorageError: std::error::Error + Send + Sync {}

impl<T: std::error::Error + Send + Sync> StorageError for T {}
