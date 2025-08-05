use serde::Deserialize;

/// Queue configuration for the execution queue system
#[derive(Debug, Clone, Deserialize)]
pub struct QueueConfig {
    /// Number of worker threads to process jobs
    pub worker_count: usize,
    /// Maximum number of jobs in the queue
    pub max_queue_size: usize,
}
