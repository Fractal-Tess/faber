use crate::error::{QueueError, QueueResult};
use crate::job::ExecutionJob;
use crate::worker::Worker;
use faber_config::Config;
use faber_core::{Task, TaskResult};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock, mpsc, oneshot};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Queue manager that handles job queuing and worker coordination
pub struct QueueManager {
    config: Arc<Config>,
    queue: Arc<Mutex<VecDeque<ExecutionJob>>>,
    job_senders: Vec<mpsc::UnboundedSender<ExecutionJob>>,
    worker_handles: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    stats: Arc<RwLock<QueueStats>>,
    shutdown_sender: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

/// Statistics about the queue
#[derive(Debug, Default)]
pub struct QueueStats {
    pub total_jobs_submitted: u64,
    pub total_jobs_completed: u64,
    pub total_jobs_failed: u64,
    pub total_jobs_timed_out: u64,
    pub current_queue_size: usize,
    pub active_workers: usize,
}

impl QueueManager {
    /// Create a new queue manager with workers
    pub fn new(config: Config) -> Self {
        let config_arc = Arc::new(config);
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let stats = Arc::new(RwLock::new(QueueStats::default()));
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();

        let mut job_senders = Vec::new();
        let mut worker_handles = Vec::new();

        // Create workers
        for worker_id in 0..config_arc.queue.worker_count {
            let (sender, receiver) = mpsc::unbounded_channel();
            job_senders.push(sender);

            let worker = Worker::new(worker_id, Arc::clone(&config_arc), receiver);
            let handle = tokio::spawn(async move {
                worker.start().await;
            });
            worker_handles.push(handle);
        }

        info!(
            "QueueManager initialized with {} workers",
            config_arc.queue.worker_count
        );

        Self {
            config: config_arc,
            queue,
            job_senders,
            worker_handles: Arc::new(Mutex::new(worker_handles)),
            stats,
            shutdown_sender: Arc::new(Mutex::new(Some(shutdown_sender))),
        }
    }

    /// Submit a job to the queue and wait for results
    pub async fn submit_job(&self, tasks: Vec<Task>) -> QueueResult<Vec<TaskResult>> {
        // Check if queue is full
        {
            let queue = self.queue.lock().await;
            if queue.len() >= self.config.queue.max_queue_size {
                warn!("Queue is full, rejecting job with {} tasks", tasks.len());
                return Err(QueueError::QueueFull);
            }
        }

        // Create job
        let (job, result_receiver) = ExecutionJob::new(tasks);
        let job_id = job.id.clone();

        info!("Submitting job {} with {} tasks", job_id, job.tasks.len());

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_jobs_submitted += 1;
            stats.current_queue_size += 1;
        }

        // Try to assign job directly to an available worker
        let mut job_assigned = false;
        for sender in &self.job_senders {
            if sender.send(job.clone()).is_ok() {
                job_assigned = true;
                debug!("Job {} assigned directly to worker", job_id);
                break;
            }
        }

        if !job_assigned {
            // If no worker is available, add to queue
            {
                let mut queue = self.queue.lock().await;
                queue.push_back(job);
                debug!("Job {} added to queue (position: {})", job_id, queue.len());
            }

            // Try to process queue
            self.process_queue().await;
        }

        // Wait for result with timeout
        let wait_timeout = Duration::from_secs(self.config.queue.max_queue_wait_time_seconds);
        match timeout(wait_timeout, result_receiver).await {
            Ok(Ok(results)) => {
                // Update stats
                {
                    let mut stats = self.stats.write().await;
                    stats.total_jobs_completed += 1;
                    stats.current_queue_size = stats.current_queue_size.saturating_sub(1);
                }
                info!("Job {} completed successfully", job_id);
                Ok(results)
            }
            Ok(Err(_)) => {
                error!("Job {} result channel closed unexpectedly", job_id);
                Err(QueueError::ExecutionFailed {
                    message: "Result channel closed".to_string(),
                })
            }
            Err(_) => {
                warn!(
                    "Job {} timed out in queue after {} seconds",
                    job_id, self.config.queue.max_queue_wait_time_seconds
                );
                // Update stats
                {
                    let mut stats = self.stats.write().await;
                    stats.total_jobs_timed_out += 1;
                    stats.current_queue_size = stats.current_queue_size.saturating_sub(1);
                }
                Err(QueueError::JobTimeout {
                    job_id,
                    timeout_seconds: self.config.queue.max_queue_wait_time_seconds,
                })
            }
        }
    }

    /// Process jobs in the queue by assigning them to available workers
    async fn process_queue(&self) {
        let mut queue = self.queue.lock().await;

        while let Some(job) = queue.pop_front() {
            let mut job_assigned = false;

            // Try to assign to an available worker
            for sender in &self.job_senders {
                if sender.send(job.clone()).is_ok() {
                    job_assigned = true;
                    debug!("Job {} assigned to worker from queue", job.id);
                    break;
                }
            }

            if !job_assigned {
                // No available workers, put job back at front of queue
                queue.push_front(job);
                break;
            }
        }
    }

    /// Get current queue statistics
    pub async fn get_stats(&self) -> QueueStats {
        let stats = self.stats.read().await;
        let queue = self.queue.lock().await;

        QueueStats {
            total_jobs_submitted: stats.total_jobs_submitted,
            total_jobs_completed: stats.total_jobs_completed,
            total_jobs_failed: stats.total_jobs_failed,
            total_jobs_timed_out: stats.total_jobs_timed_out,
            current_queue_size: queue.len(),
            active_workers: self.config.queue.worker_count,
        }
    }

    /// Shutdown the queue manager and all workers (works with Arc)
    pub async fn shutdown(&self) -> QueueResult<()> {
        info!("Shutting down QueueManager");

        // Signal shutdown
        if let Some(sender) = self.shutdown_sender.lock().await.take() {
            let _ = sender.send(());
        }

        // Close all job senders to signal workers to shutdown
        for sender in &self.job_senders {
            sender.closed().await;
        }

        // Wait for all workers to finish
        let mut handles = self.worker_handles.lock().await;
        while let Some(handle) = handles.pop() {
            if let Err(e) = handle.await {
                error!("Worker failed to shutdown cleanly: {}", e);
            }
        }

        info!("QueueManager shutdown complete");
        Ok(())
    }
}
