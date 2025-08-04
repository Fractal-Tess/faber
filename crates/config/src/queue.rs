use serde::{Deserialize, Serialize};

/// Queue configuration for the execution queue system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    /// Number of worker threads to process jobs
    pub worker_count: usize,
    /// Maximum number of jobs in the queue
    pub max_queue_size: usize,
    /// Maximum time a job can wait in the queue before being cancelled (seconds)
    pub max_queue_wait_time_seconds: u64,
    /// Maximum time a job can run before being cancelled (seconds)
    pub max_job_execution_time_seconds: u64,
    /// How often workers check for new jobs (milliseconds)
    pub worker_poll_interval_ms: u64,
    /// Maximum number of concurrent sandboxes (should match worker_count)
    pub max_concurrent_sandboxes: usize,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            worker_count: 2,
            max_queue_size: 100,
            max_queue_wait_time_seconds: 300,    // 5 minutes
            max_job_execution_time_seconds: 120, // 2 minutes
            worker_poll_interval_ms: 100,
            max_concurrent_sandboxes: 2,
        }
    }
}
