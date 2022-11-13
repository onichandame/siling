use serde::{de::DeserializeOwned, Serialize};

pub trait Argument: Serialize + DeserializeOwned + Send + Clone {}

impl<T: Serialize + DeserializeOwned + Send + Clone> Argument for T {}
