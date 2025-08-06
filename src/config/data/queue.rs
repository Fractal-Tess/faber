use serde::Deserialize;

/// Queue configuration for the execution queue system
use serde::de::{self, Deserializer, MapAccess, Visitor};
use std::fmt;

#[derive(Debug, Clone)]
pub struct QueueConfig {
    /// Number of worker threads to process jobs
    pub worker_count: u16,
    /// Maximum number of jobs in the queue
    pub max_queue_size: u16,
}

impl<'de> Deserialize<'de> for QueueConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawQueueConfig {
            worker_count: u16,
            max_queue_size: u16,
        }

        let raw = RawQueueConfig::deserialize(deserializer)?;
        if raw.worker_count == 0 {
            return Err(de::Error::custom(
                "QueueConfig: worker_count must be greater than 0",
            ));
        }
        if raw.max_queue_size == 0 {
            return Err(de::Error::custom(
                "QueueConfig: max_queue_size must be greater than 0",
            ));
        }
        Ok(QueueConfig {
            worker_count: raw.worker_count,
            max_queue_size: raw.max_queue_size,
        })
    }
}
