use crate::error::{QueueError, QueueResult};
use crate::job::ExecutionJob;
use crate::worker::Worker;
use faber_config::GlobalConfig;
use faber_executor::{Task, TaskResult};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, mpsc, oneshot};
use tracing::{debug, error, info, warn};

/// Queue manager that handles job queuing and worker coordination
pub struct QueueManager {
    config: Arc<GlobalConfig>,
    queue: Arc<Mutex<VecDeque<ExecutionJob>>>,
    job_senders: Arc<Mutex<Vec<mpsc::UnboundedSender<ExecutionJob>>>>,
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
    pub current_queue_size: usize,
    pub active_workers: usize,
}

impl QueueManager {
    /// Create a new queue manager with workers
    pub fn new(config: GlobalConfig) -> Self {
        let config_arc = Arc::new(config);
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let stats = Arc::new(RwLock::new(QueueStats::default()));
        let (shutdown_sender, _shutdown_receiver) = oneshot::channel();

        let mut job_senders = Vec::new();
        let mut worker_handles = Vec::new();

        // Create workers
        for worker_id in 0..(config.queue.worker_count as usize) {
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
            config.queue.worker_count
        );

        Self {
            config: config_arc,
            queue,
            job_senders: Arc::new(Mutex::new(job_senders)),
            worker_handles: Arc::new(Mutex::new(worker_handles)),
            stats,
            shutdown_sender: Arc::new(Mutex::new(Some(shutdown_sender))),
        }
    }

    /// Remove closed worker channels from the job_senders list
    async fn cleanup_dead_channels(&self) {
        let mut senders = self.job_senders.lock().await;
        let initial_count = senders.len();

        // Remove channels that are closed
        senders.retain(|sender| !sender.is_closed());

        let removed_count = initial_count - senders.len();
        if removed_count > 0 {
            warn!("Removed {} dead worker channels", removed_count);

            // Update active workers count in stats
            let mut stats = self.stats.write().await;
            stats.active_workers = senders.len();
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

        // Update stats - increment submitted jobs
        {
            let mut stats = self.stats.write().await;
            stats.total_jobs_submitted += 1;
        }

        // Try to find an available worker by attempting to send to each worker
        let mut job_assigned = false;
        let mut was_queued = false;
        let mut dead_channels_detected = false;

        {
            let senders = self.job_senders.lock().await;
            for (worker_index, sender) in senders.iter().enumerate() {
                // Try to send without blocking - if successful, worker will pick it up
                match sender.send(job.clone()) {
                    Ok(()) => {
                        job_assigned = true;
                        debug!(
                            "Job {} assigned directly to worker {}",
                            job_id, worker_index
                        );
                        break;
                    }
                    Err(_) => {
                        // This sender is closed, worker has shut down
                        debug!("Worker {} sender is closed", worker_index);
                        dead_channels_detected = true;
                        continue;
                    }
                }
            }
        }

        // Clean up dead channels if we detected any
        if dead_channels_detected {
            self.cleanup_dead_channels().await;
        }

        if !job_assigned {
            // If no worker channels accepted the job, add to queue for later processing
            {
                let mut queue = self.queue.lock().await;
                queue.push_back(job);
                debug!("Job {} added to queue (position: {})", job_id, queue.len());
            }

            // Update stats to reflect job is now queued
            {
                let mut stats = self.stats.write().await;
                stats.current_queue_size += 1;
            }

            was_queued = true;

            // Try to process the queue
            self.process_queue().await;
        }

        // Wait for result
        match result_receiver.await {
            Ok(results) => {
                // Update stats - job completed successfully
                {
                    let mut stats = self.stats.write().await;
                    stats.total_jobs_completed += 1;
                    // Only decrement if job was actually in the queue
                    if was_queued && stats.current_queue_size > 0 {
                        stats.current_queue_size -= 1;
                    }
                }
                info!("Job {} completed successfully", job_id);
                Ok(results)
            }
            Err(_) => {
                error!("Job {} result channel closed unexpectedly", job_id);
                // Update stats - job failed
                {
                    let mut stats = self.stats.write().await;
                    stats.total_jobs_failed += 1;
                    if was_queued && stats.current_queue_size > 0 {
                        stats.current_queue_size -= 1;
                    }
                }
                Err(QueueError::ExecutionFailed {
                    message: "Result channel closed".to_string(),
                })
            }
        }
    }

    /// Process jobs in the queue by assigning them to available workers
    async fn process_queue(&self) {
        let mut queue = self.queue.lock().await;

        let mut jobs_assigned = 0;
        let mut dead_channels_detected = false;

        while let Some(job) = queue.pop_front() {
            let mut job_assigned = false;

            // Try to assign to an available worker
            {
                let senders = self.job_senders.lock().await;
                for (worker_index, sender) in senders.iter().enumerate() {
                    match sender.send(job.clone()) {
                        Ok(()) => {
                            job_assigned = true;
                            jobs_assigned += 1;
                            debug!(
                                "Job {} assigned to worker {} from queue",
                                job.id, worker_index
                            );
                            break;
                        }
                        Err(_) => {
                            // This sender is closed, worker has shut down
                            debug!(
                                "Worker {} sender is closed during queue processing",
                                worker_index
                            );
                            dead_channels_detected = true;
                            continue;
                        }
                    }
                }
            }

            if !job_assigned {
                // No available workers, put job back at front of queue
                queue.push_front(job);
                break;
            }
        }

        // Release the queue lock before other operations
        drop(queue);

        // Update stats to reflect jobs that were moved from queue to workers
        if jobs_assigned > 0 {
            let mut stats = self.stats.write().await;
            stats.current_queue_size = stats.current_queue_size.saturating_sub(jobs_assigned);
            debug!(
                "Processed {} jobs from queue, {} remaining",
                jobs_assigned, stats.current_queue_size
            );
        }

        // Clean up dead channels if we detected any
        if dead_channels_detected {
            self.cleanup_dead_channels().await;
        }
    }

    /// Get current queue statistics
    pub async fn get_stats(&self) -> QueueStats {
        // Get both stats and queue under locks for consistency
        let stats = self.stats.read().await;
        let queue = self.queue.lock().await;

        // Use the actual queue size as the authoritative source
        let actual_queue_size = queue.len();

        QueueStats {
            total_jobs_submitted: stats.total_jobs_submitted,
            total_jobs_completed: stats.total_jobs_completed,
            total_jobs_failed: stats.total_jobs_failed,
            current_queue_size: actual_queue_size,
            active_workers: stats.active_workers,
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
        {
            let senders = self.job_senders.lock().await;
            for sender in senders.iter() {
                sender.closed().await;
            }
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
