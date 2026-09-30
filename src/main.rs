use faber_api::axum;
use faber_api::{ServeConfig, build_router, serve};
use faber_runtime::Runtime;
use faber_store::StoreConfig;

mod config;
use config::{Config, StoreBackend};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "faber=info,faber_api=info,faber_runtime=info".into()),
        )
        .try_init()?;

    let config = Config::from_env()?;
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
    Runtime::configure_service_limits(
        &config.execution_limits.memory_max,
        config.execution_limits.pids_max,
        config.execution_limits.max_concurrency,
    )?;

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

    let file_store = faber_store::create_store(store_config);

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
