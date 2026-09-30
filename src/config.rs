use std::env;
use std::time::Duration;

use faber_api::ExecutionLimits;
use faber_runtime::SandboxProfile;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub host: String,
    pub api_key: String,
    pub cache_enabled: bool,
    pub store_backend: StoreBackend,
    pub execution_limits: ExecutionLimits,
    pub shutdown_timeout: Duration,
}

#[derive(Debug, Clone)]
pub enum StoreBackend {
    Memory,
    Filesystem {
        path: String,
    },
    Hybrid {
        path: String,
        max_memory_entries: usize,
        max_memory_size: u64,
    },
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Config {
            port: Self::load_port()?,
            host: Self::load_host(),
            api_key: Self::load_api_key()?,
            cache_enabled: Self::load_cache_enabled(),
            store_backend: Self::load_store_backend(),
            execution_limits: Self::load_execution_limits()?,
            shutdown_timeout: Duration::from_millis(Self::load_env("SHUTDOWN_TIMEOUT_MS", 5_000)?),
        })
    }

    fn load_port() -> Result<u16, Box<dyn std::error::Error + Send + Sync>> {
        let port_str = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
        port_str.parse::<u16>().map_err(|e| e.into())
    }

    fn load_host() -> String {
        env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string())
    }

    fn load_api_key() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        env::var("API_KEY")
            .map_err(|_| "API_KEY environment variable is required but not set".into())
    }

    fn load_cache_enabled() -> bool {
        env::var("CACHE_ENABLED")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false)
    }

    fn load_execution_limits() -> Result<ExecutionLimits, Box<dyn std::error::Error + Send + Sync>>
    {
        let memory_max = env::var("MEMORY_MAX").unwrap_or_else(|_| "256M".to_string());
        if memory_max.trim().eq_ignore_ascii_case("max") {
            return Err("MEMORY_MAX must be finite for the API service".into());
        }

        let default_sandbox_profile = Self::load_sandbox_profile(
            &env::var("DEFAULT_SANDBOX_PROFILE").unwrap_or_else(|_| "compile_v1".to_string()),
        )?;
        let allowed_sandbox_profiles = env::var("ALLOWED_SANDBOX_PROFILES")
            .unwrap_or_else(|_| "compile_v1,native_v1".to_string())
            .split(',')
            .map(|value| Self::load_sandbox_profile(value.trim()))
            .collect::<Result<Vec<_>, _>>()?;
        if !allowed_sandbox_profiles.contains(&default_sandbox_profile) {
            return Err(
                "DEFAULT_SANDBOX_PROFILE must be present in ALLOWED_SANDBOX_PROFILES".into(),
            );
        }

        let max_parallel_tasks = Self::load_env("MAX_PARALLEL_TASKS", 8)?;
        let max_concurrency = Self::load_env("MAX_CONCURRENCY", 10)?;
        if max_parallel_tasks > max_concurrency {
            return Err(
                "MAX_PARALLEL_TASKS must not exceed MAX_CONCURRENCY (task slots); a wider step could never be admitted"
                    .into(),
            );
        }

        Ok(ExecutionLimits {
            memory_max,
            pids_max: Self::load_env("PIDS_MAX", 64)?,
            cpu_max: env::var("CPU_MAX").unwrap_or_else(|_| "50000 100000".to_string()),
            wall_timeout: Duration::from_millis(Self::load_env("WALL_TIMEOUT_MS", 5_000)?),
            cpu_time_limit: Duration::from_secs(Self::load_env("CPU_TIME_LIMIT_SECS", 5)?),
            overall_timeout: Duration::from_millis(Self::load_env("OVERALL_TIMEOUT_MS", 30_000)?),
            output_limit: Self::load_env("OUTPUT_LIMIT_BYTES", 1024 * 1024)?,
            max_steps: Self::load_env("MAX_STEPS_PER_REQUEST", 64)?,
            max_parallel_tasks,
            max_concurrency,
            execute_body_limit: Self::load_env("EXECUTE_BODY_LIMIT_BYTES", 1024 * 1024)?,
            upload_file_limit: Self::load_env("UPLOAD_FILE_LIMIT_BYTES", 50 * 1024 * 1024)?,
            default_sandbox_profile,
            allowed_sandbox_profiles,
        })
    }

    fn load_sandbox_profile(
        value: &str,
    ) -> Result<SandboxProfile, Box<dyn std::error::Error + Send + Sync>> {
        match value {
            "compile_v1" => Ok(SandboxProfile::CompileV1),
            "native_v1" => Ok(SandboxProfile::NativeV1),
            _ => Err(format!("Unknown sandbox profile: {value}").into()),
        }
    }

    fn load_env<T>(name: &str, default: T) -> Result<T, Box<dyn std::error::Error + Send + Sync>>
    where
        T: std::str::FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        match env::var(name) {
            Ok(value) => value.parse::<T>().map_err(Into::into),
            Err(_) => Ok(default),
        }
    }

    fn load_store_backend() -> StoreBackend {
        match env::var("FABER_STORE_BACKEND").unwrap_or_default().as_str() {
            "filesystem" => {
                let path = env::var("FABER_STORE_PATH")
                    .unwrap_or_else(|_| "/var/lib/faber/store".to_string());
                StoreBackend::Filesystem { path }
            }
            "hybrid" => {
                let path = env::var("FABER_STORE_PATH")
                    .unwrap_or_else(|_| "/var/lib/faber/store".to_string());
                let max_memory_entries = env::var("FABER_STORE_MAX_MEMORY_ENTRIES")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1000);
                let max_memory_size = env::var("FABER_STORE_MAX_MEMORY_SIZE")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(100 * 1024 * 1024);
                StoreBackend::Hybrid {
                    path,
                    max_memory_entries,
                    max_memory_size,
                }
            }
            _ => StoreBackend::Memory,
        }
    }
}
