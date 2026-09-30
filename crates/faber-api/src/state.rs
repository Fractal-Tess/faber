use crate::cache::ExecutionCache;
use faber_runtime::SandboxProfile;
use faber_store::FileStore;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

#[derive(Clone, Debug)]
pub struct ExecutionLimits {
    pub memory_max: String,
    pub pids_max: u32,
    pub cpu_max: String,
    pub wall_timeout: Duration,
    pub cpu_time_limit: Duration,
    pub overall_timeout: Duration,
    pub output_limit: usize,
    pub request_output_limit: usize,
    pub max_steps: usize,
    pub max_parallel_tasks: usize,
    /// Task slots: how many sandboxed tasks may run at once across all
    /// requests. A request reserves as many slots as its widest step and
    /// holds them until its sandbox has actually finished, not merely until
    /// its HTTP request ends. Must be at least `max_parallel_tasks`.
    pub max_concurrency: usize,
    pub execute_body_limit: usize,
    pub upload_file_limit: usize,
    pub default_sandbox_profile: SandboxProfile,
    pub allowed_sandbox_profiles: Vec<SandboxProfile>,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            memory_max: "256M".to_string(),
            pids_max: 64,
            cpu_max: "50000 100000".to_string(),
            wall_timeout: Duration::from_secs(5),
            cpu_time_limit: Duration::from_secs(5),
            overall_timeout: Duration::from_secs(30),
            output_limit: 1024 * 1024,
            request_output_limit: 16 * 1024 * 1024,
            max_steps: 64,
            max_parallel_tasks: 8,
            max_concurrency: 10,
            execute_body_limit: 1024 * 1024,
            upload_file_limit: 50 * 1024 * 1024,
            default_sandbox_profile: SandboxProfile::CompileV1,
            allowed_sandbox_profiles: vec![SandboxProfile::CompileV1, SandboxProfile::NativeV1],
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub cache: ExecutionCache,
    pub file_store: Arc<dyn FileStore>,
    pub api_key: String,
    pub cache_enabled: bool,
    pub execution_limits: ExecutionLimits,
    pub execution_slots: Arc<Semaphore>,
}

impl AppState {
    pub fn new(
        api_key: String,
        cache_enabled: bool,
        file_store: Arc<dyn FileStore>,
        execution_limits: ExecutionLimits,
    ) -> Self {
        Self {
            execution_slots: Arc::new(Semaphore::new(
                execution_limits.max_concurrency.min(Semaphore::MAX_PERMITS),
            )),
            cache: ExecutionCache::new(),
            file_store,
            api_key,
            cache_enabled,
            execution_limits,
        }
    }
}
