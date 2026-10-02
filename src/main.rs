use faber_api::axum;
use faber_api::{ServeConfig, build_router, serve};
use faber_runtime::Runtime;
use faber_store::StoreConfig;

mod config;
use config::{Config, StoreBackend};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let log_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "faber=info,faber_api=info,faber_runtime=info".into());
    // LOG_FORMAT=json emits one JSON object per line for log collectors.
    if std::env::var("LOG_FORMAT").is_ok_and(|format| format.eq_ignore_ascii_case("json")) {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(log_filter)
            .try_init()?;
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(log_filter)
            .try_init()?;
    }

    let config = Config::from_env()?;
    if config.api_key.split(',').any(|key| key.trim().len() < 32) {
        tracing::warn!(
            "an API key is shorter than 32 characters; generate keys with `openssl rand -hex 32`"
        );
    }
    let limits = &config.execution_limits;
    let longest_request = limits
        .wall_timeout
        .saturating_mul(u32::try_from(limits.max_steps).unwrap_or(u32::MAX));
    if longest_request > limits.overall_timeout {
        tracing::warn!(
            max_steps = limits.max_steps,
            wall_timeout = ?limits.wall_timeout,
            overall_timeout = ?limits.overall_timeout,
            "a request of MAX_STEPS_PER_REQUEST steps at WALL_TIMEOUT_MS each can outlast \
             OVERALL_TIMEOUT_MS; steps past the deadline are reported as not_started"
        );
    }
    Runtime::initialize()?;
    Runtime::reclaim_stale_sandboxes()?;
    Runtime::configure_service_limits(
        &config.execution_limits.memory_max,
        config.execution_limits.pids_max,
        config.execution_limits.max_concurrency,
        config.execution_limits.request_output_limit,
    )?;
    Runtime::configure_identities(config.sandbox_identity_base, config.sandbox_identity_count)?;

    let mut store_config = match &config.store_backend {
        StoreBackend::Memory => StoreConfig::builder().memory().build(),
        StoreBackend::Filesystem { path } => StoreConfig::builder().filesystem(path).build(),
        StoreBackend::Hybrid {
            path,
            max_memory_entries,
            max_memory_size,
        } => StoreConfig::builder()
            .hybrid(path, *max_memory_entries, *max_memory_size)
            .build(),
    };
    store_config.max_file_size = config.execution_limits.upload_file_limit as u64;
    store_config.default_ttl = config.store_limits.ttl;
    store_config.ttl_check_interval = config.store_limits.ttl_check_interval;
    store_config.max_total_bytes = config.store_limits.max_total_bytes;
    store_config.max_entries = config.store_limits.max_entries;

    let file_store = faber_store::create_store(store_config);
    if !config.store_limits.ttl.is_zero() {
        faber_store::spawn_expiry_sweeper(
            file_store.clone(),
            config.store_limits.ttl_check_interval,
        );
    }

    let router = build_router(
        config.api_key.clone(),
        config.cache_enabled,
        file_store,
        config.execution_limits,
    );
    let router = axum::Router::new().nest("/api/v1", router);

    let serve_config = ServeConfig {
        port: config.port,
        host: config.host,
        router,
        shutdown_timeout: config.shutdown_timeout,
    };

    serve(serve_config).await?;

    Ok(())
}
