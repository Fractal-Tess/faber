use std::env;
use std::time::Duration;

use faber_api::ExecutionLimits;
use faber_runtime::SandboxProfile;

pub type ConfigError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub host: String,
    pub api_key: String,
    pub cache_enabled: bool,
    pub store_backend: StoreBackend,
    pub store_limits: StoreLimits,
    pub execution_limits: ExecutionLimits,
    pub shutdown_timeout: Duration,
    /// First host UID/GID leased to sandboxes, and how many.
    pub sandbox_identity_base: u32,
    pub sandbox_identity_count: u32,
}

#[derive(Debug, Clone)]
pub struct StoreLimits {
    pub ttl: Duration,
    pub ttl_check_interval: Duration,
    pub max_total_bytes: u64,
    pub max_entries: usize,
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

/// Reads settings from a variable lookup and names the variable in every error.
struct Settings<F> {
    lookup: F,
}

impl<F: Fn(&str) -> Option<String>> Settings<F> {
    fn string(&self, name: &str, default: &str) -> String {
        (self.lookup)(name).unwrap_or_else(|| default.to_string())
    }

    fn parse<T>(&self, name: &str, default: T) -> Result<T, ConfigError>
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        match (self.lookup)(name) {
            Some(value) => value
                .trim()
                .parse::<T>()
                .map_err(|error| format!("{name}: invalid value {value:?}: {error}").into()),
            None => Ok(default),
        }
    }

    /// A number that must be at least one.
    fn positive<T>(&self, name: &str, default: T) -> Result<T, ConfigError>
    where
        T: std::str::FromStr + PartialOrd + From<u8>,
        T::Err: std::fmt::Display,
    {
        let value = self.parse(name, default)?;
        if value < T::from(1) {
            return Err(format!("{name} must be at least 1").into());
        }
        Ok(value)
    }

    fn flag(&self, name: &str) -> Result<bool, ConfigError> {
        match (self.lookup)(name) {
            None => Ok(false),
            Some(value) => match value.trim().to_ascii_lowercase().as_str() {
                "true" | "1" => Ok(true),
                "false" | "0" | "" => Ok(false),
                _ => Err(format!("{name}: expected true, false, 1 or 0, got {value:?}").into()),
            },
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| env::var(name).ok())
    }

    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let settings = Settings { lookup };
        Ok(Config {
            port: settings.positive("PORT", 3000)?,
            host: settings.string("HOST", "0.0.0.0"),
            api_key: (settings.lookup)("API_KEY")
                .filter(|key| !key.is_empty())
                .ok_or("API_KEY environment variable is required but not set")?,
            cache_enabled: settings.flag("CACHE_ENABLED")?,
            store_backend: Self::load_store_backend(&settings)?,
            store_limits: StoreLimits {
                ttl: Duration::from_secs(settings.parse("FABER_STORE_TTL_SECS", 3600)?),
                ttl_check_interval: Duration::from_secs(
                    settings.positive("FABER_STORE_TTL_CHECK_SECS", 60)?,
                ),
                max_total_bytes: settings
                    .positive("FABER_STORE_MAX_TOTAL_BYTES", 512 * 1024 * 1024)?,
                max_entries: settings.positive("FABER_STORE_MAX_ENTRIES", 1000)?,
            },
            execution_limits: Self::load_execution_limits(&settings)?,
            shutdown_timeout: Duration::from_millis(settings.parse("SHUTDOWN_TIMEOUT_MS", 5_000)?),
            sandbox_identity_base: settings.positive("SANDBOX_IDENTITY_BASE", 100_000)?,
            sandbox_identity_count: settings.positive("SANDBOX_IDENTITY_COUNT", 65_536)?,
        })
    }

    fn load_execution_limits(
        settings: &Settings<impl Fn(&str) -> Option<String>>,
    ) -> Result<ExecutionLimits, ConfigError> {
        let memory_max = settings.string("MEMORY_MAX", "256M");
        let memory_bytes = faber_runtime::parse_memory_limit(&memory_max)
            .map_err(|error| format!("MEMORY_MAX: invalid value {memory_max:?}: {error}"))?;
        if memory_bytes == u64::MAX {
            return Err("MEMORY_MAX must be finite for the API service".into());
        }
        if memory_bytes < 1024 * 1024 {
            return Err("MEMORY_MAX must be at least 1M".into());
        }

        let cpu_max = settings.string("CPU_MAX", "50000 100000");
        validate_cpu_max(&cpu_max).map_err(|error| format!("CPU_MAX: {error}"))?;

        let default_sandbox_profile = Self::load_sandbox_profile(
            "DEFAULT_SANDBOX_PROFILE",
            &settings.string("DEFAULT_SANDBOX_PROFILE", "compile_v2"),
        )?;
        let allowed_sandbox_profiles = settings
            .string("ALLOWED_SANDBOX_PROFILES", "compile_v2,native_v2")
            .split(',')
            .map(|value| Self::load_sandbox_profile("ALLOWED_SANDBOX_PROFILES", value.trim()))
            .collect::<Result<Vec<_>, _>>()?;
        if !allowed_sandbox_profiles.contains(&default_sandbox_profile) {
            return Err(
                "DEFAULT_SANDBOX_PROFILE must be present in ALLOWED_SANDBOX_PROFILES".into(),
            );
        }

        let max_parallel_tasks = settings.positive("MAX_PARALLEL_TASKS", 8)?;
        let max_concurrency = settings.positive("MAX_CONCURRENCY", 10)?;
        if max_parallel_tasks > max_concurrency {
            return Err(
                "MAX_PARALLEL_TASKS must not exceed MAX_CONCURRENCY (task slots); a wider step could never be admitted"
                    .into(),
            );
        }

        Ok(ExecutionLimits {
            memory_max,
            pids_max: settings.positive("PIDS_MAX", 64)?,
            cpu_max,
            wall_timeout: Duration::from_millis(settings.positive("WALL_TIMEOUT_MS", 5_000)?),
            cpu_time_limit: Duration::from_secs(settings.positive("CPU_TIME_LIMIT_SECS", 5)?),
            overall_timeout: Duration::from_millis(
                settings.positive("OVERALL_TIMEOUT_MS", 30_000)?,
            ),
            output_limit: settings.positive("OUTPUT_LIMIT_BYTES", 1024 * 1024)?,
            request_output_limit: settings
                .positive("REQUEST_OUTPUT_LIMIT_BYTES", 16 * 1024 * 1024)?,
            max_steps: settings.positive("MAX_STEPS_PER_REQUEST", 64)?,
            max_parallel_tasks,
            max_concurrency,
            execute_body_limit: settings.positive("EXECUTE_BODY_LIMIT_BYTES", 1024 * 1024)?,
            upload_file_limit: settings.positive("UPLOAD_FILE_LIMIT_BYTES", 50 * 1024 * 1024)?,
            max_concurrent_uploads: settings.positive("MAX_CONCURRENT_UPLOADS", 4)?,
            default_sandbox_profile,
            allowed_sandbox_profiles,
            readonly_paths: Self::load_readonly_paths(settings)?,
            cache: faber_api::CacheLimits {
                max_entries: settings.positive("CACHE_MAX_ENTRIES", 1024)?,
                max_bytes: settings.positive("CACHE_MAX_BYTES", 64 * 1024 * 1024)?,
                ttl: Duration::from_secs(settings.positive("CACHE_TTL_SECS", 300)?),
            },
        })
    }

    /// `SANDBOX_READONLY_PATHS`: comma-separated absolute paths of this image
    /// that sandboxes see read-only. Replaces the default list.
    fn load_readonly_paths(
        settings: &Settings<impl Fn(&str) -> Option<String>>,
    ) -> Result<Vec<String>, ConfigError> {
        const FORBIDDEN: [&str; 6] = ["/", "/proc", "/sys", "/dev", "/tmp", "/faber"];
        let default = faber_runtime::DEFAULT_READONLY_PATHS.join(",");
        settings
            .string("SANDBOX_READONLY_PATHS", &default)
            .split(',')
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(|path| {
                let normalized = path.trim_end_matches('/');
                let valid = path.starts_with('/')
                    && !path.split('/').any(|component| component == "..")
                    && !FORBIDDEN.iter().any(|forbidden| {
                        normalized == forbidden.trim_end_matches('/')
                            || (forbidden.len() > 1
                                && normalized.starts_with(&format!("{forbidden}/")))
                    });
                if valid {
                    Ok(path.to_string())
                } else {
                    Err(format!(
                        "SANDBOX_READONLY_PATHS: {path:?} must be an absolute path outside \
                         /proc, /sys, /dev, /tmp and /faber"
                    )
                    .into())
                }
            })
            .collect()
    }

    fn load_sandbox_profile(name: &str, value: &str) -> Result<SandboxProfile, ConfigError> {
        match value {
            "compile_v1" => Ok(SandboxProfile::CompileV1),
            "native_v1" => Ok(SandboxProfile::NativeV1),
            "compile_v2" => Ok(SandboxProfile::CompileV2),
            "native_v2" => Ok(SandboxProfile::NativeV2),
            _ => Err(format!("{name}: unknown sandbox profile {value:?}").into()),
        }
    }

    fn load_store_backend(
        settings: &Settings<impl Fn(&str) -> Option<String>>,
    ) -> Result<StoreBackend, ConfigError> {
        let path = || settings.string("FABER_STORE_PATH", "/var/lib/faber/store");
        match settings.string("FABER_STORE_BACKEND", "memory").as_str() {
            "memory" | "" => Ok(StoreBackend::Memory),
            "filesystem" => Ok(StoreBackend::Filesystem { path: path() }),
            "hybrid" => Ok(StoreBackend::Hybrid {
                path: path(),
                max_memory_entries: settings.positive("FABER_STORE_MAX_MEMORY_ENTRIES", 1000)?,
                max_memory_size: settings
                    .positive("FABER_STORE_MAX_MEMORY_SIZE", 100 * 1024 * 1024)?,
            }),
            other => Err(format!(
                "FABER_STORE_BACKEND: expected memory, filesystem or hybrid, got {other:?}"
            )
            .into()),
        }
    }
}

/// Validate a cgroup v2 `cpu.max` value: `<quota> [<period>]`, where quota is
/// `max` or microseconds and the period is 1000..=1000000 microseconds.
fn validate_cpu_max(value: &str) -> Result<(), String> {
    let mut fields = value.split_whitespace();
    let quota = fields
        .next()
        .ok_or_else(|| "expected \"<quota> [<period>]\", got an empty value".to_string())?;
    let period = fields.next();
    if fields.next().is_some() {
        return Err(format!("expected \"<quota> [<period>]\", got {value:?}"));
    }

    let period = match period {
        Some(period) => period
            .parse::<u64>()
            .map_err(|_| format!("period {period:?} is not a number of microseconds"))?,
        None => 100_000,
    };
    if !(1_000..=1_000_000).contains(&period) {
        return Err(format!("period {period} must be between 1000 and 1000000"));
    }
    if quota != "max" {
        let quota = quota
            .parse::<u64>()
            .map_err(|_| format!("quota {quota:?} must be \"max\" or a number of microseconds"))?;
        if quota < 1_000 {
            return Err(format!("quota {quota} must be at least 1000"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::Config;

    fn load(vars: &[(&str, &str)]) -> Result<Config, String> {
        let mut vars: HashMap<String, String> = vars
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        vars.entry("API_KEY".to_string())
            .or_insert_with(|| "key".to_string());
        Config::from_lookup(|name| vars.get(name).cloned()).map_err(|error| error.to_string())
    }

    #[test]
    fn defaults_are_valid() {
        let config = load(&[]).expect("defaults rejected");
        assert_eq!(config.execution_limits.max_concurrency, 10);
        assert_eq!(config.execution_limits.max_parallel_tasks, 8);
    }

    #[test]
    fn zero_limits_are_rejected_with_the_variable_name() {
        for name in [
            "MAX_CONCURRENCY",
            "PIDS_MAX",
            "WALL_TIMEOUT_MS",
            "CPU_TIME_LIMIT_SECS",
            "OVERALL_TIMEOUT_MS",
            "MAX_STEPS_PER_REQUEST",
            "MAX_PARALLEL_TASKS",
            "OUTPUT_LIMIT_BYTES",
            "REQUEST_OUTPUT_LIMIT_BYTES",
            "EXECUTE_BODY_LIMIT_BYTES",
            "UPLOAD_FILE_LIMIT_BYTES",
            "MAX_CONCURRENT_UPLOADS",
            "FABER_STORE_TTL_CHECK_SECS",
            "FABER_STORE_MAX_TOTAL_BYTES",
            "FABER_STORE_MAX_ENTRIES",
            "PORT",
        ] {
            let error = load(&[(name, "0")]).expect_err(name);
            assert!(error.contains(name), "{name}: {error}");
        }
    }

    #[test]
    fn unparsable_values_name_the_variable() {
        for (name, value) in [
            ("PORT", "http"),
            ("MAX_CONCURRENCY", "ten"),
            ("MEMORY_MAX", "lots"),
            ("CACHE_ENABLED", "yes please"),
            ("FABER_STORE_BACKEND", "redis"),
            ("DEFAULT_SANDBOX_PROFILE", "none"),
        ] {
            let error = load(&[(name, value)]).expect_err(name);
            assert!(error.contains(name), "{name}: {error}");
        }
    }

    #[test]
    fn memory_must_be_finite() {
        assert!(load(&[("MEMORY_MAX", "max")]).is_err());
        assert!(load(&[("MEMORY_MAX", "512M")]).is_ok());
    }

    #[test]
    fn cpu_max_format_is_checked_at_startup() {
        for value in [
            "max",
            "max 100000",
            "50000",
            "50000 100000",
            "200000 1000000",
        ] {
            load(&[("CPU_MAX", value)]).unwrap_or_else(|error| panic!("{value}: {error}"));
        }
        for value in [
            "",
            "fast",
            "50000 100000 1",
            "500 100000",
            "50000 10",
            "50000 abc",
        ] {
            let error = load(&[("CPU_MAX", value)]).expect_err(value);
            assert!(error.contains("CPU_MAX"), "{value}: {error}");
        }
    }

    #[test]
    fn parallel_fan_out_must_fit_the_task_slots() {
        let error = load(&[("MAX_CONCURRENCY", "4"), ("MAX_PARALLEL_TASKS", "8")]).unwrap_err();
        assert!(error.contains("MAX_PARALLEL_TASKS"));
        assert!(load(&[("MAX_CONCURRENCY", "8"), ("MAX_PARALLEL_TASKS", "8")]).is_ok());
    }
}
