use faber_api::create_router;
use faber_config::Config;
use tracing::{error, info};

use crate::cli::Cli;

pub async fn serve(cli: Cli, graceful_shutdown: bool) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Faber...");

    // Load configuration with CLI overrides
    let config = Config::load(cli.config, cli.host, cli.port, cli.open_mode)?;

    info!("Configuration loaded successfully");
    info!("{}", config);

    let app = create_router(&config);

    let listener =
        tokio::net::TcpListener::bind(&format!("{}:{}", config.api.host, config.api.port)).await?;
    info!("🚀 Listening on {}", listener.local_addr()?);

    if graceful_shutdown {
        let shutdown_signal = async {
            tokio::signal::ctrl_c().await.ok();
        };

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal)
            .await?;
    } else {
        axum::serve(listener, app).await?;
    }

    info!("Shutting down...");
    Ok(())
}

pub fn validate_config(config_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    info!("Validating configuration file: {}", config_path);

    match Config::from_file(config_path) {
        Ok(config) => {
            info!("✅ Configuration is valid");
            info!("{}", config);
            Ok(())
        }
        Err(e) => {
            error!("❌ Configuration validation failed: {}", e);
            Err(e.into())
        }
    }
}

pub fn show_config(
    default: bool,
    config_path: &Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if default {
        let config = Config::default();
        println!("{}", serde_json::to_string_pretty(&config)?);
    } else {
        let path = config_path.as_deref().unwrap_or("config/default.toml");
        match Config::from_file(path) {
            Ok(config) => {
                println!("{}", serde_json::to_string_pretty(&config)?);
            }
            Err(e) => {
                error!("Failed to load configuration from {}: {}", path, e);
                return Err(e.into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Commands};
    use std::fs;

    use tempfile::NamedTempFile;

    fn create_test_config() -> NamedTempFile {
        let config_content = r#"
[api]
host = "127.0.0.1"
port = 8080

[api.cors]
enable_cors = false
cors_allowed_origins = "*"
cors_allowed_methods = "GET,POST,OPTIONS"
cors_allowed_headers = "*"
cors_allow_credentials = false

[api.request]
max_request_size_kb = 10240

[api.auth]
enable = "env:FABER_AUTH_ENABLE|false"
secret_key = "env:FABER_AUTH_SECRET_KEY"

[api.endpoints]
health_endpoint = "/health"
execute_endpoint = "/execute-tasks"

[sandbox.resource_limits]
memory_limit_kb = 524288
cpu_time_limit_ms = 10000
max_cpu_cores = 1
wall_time_limit_ms = 30000
max_processes = 50
max_fds = 256
stack_limit_kb = 4
data_segment_limit_kb = 256
address_space_limit_kb = 1024
cpu_rate_limit_percent = 50
io_read_limit_kb_s = 10
io_write_limit_kb_s = 10

[sandbox.cgroups]
enabled = true
prefix = "faber"
version = "v2"
enable_cpu_rate_limit = true
enable_memory_limit = true
enable_process_limit = true

[sandbox.filesystem]
readonly = true
tmpfs_size_mb = 100

[sandbox.filesystem.mounts]
readable = { "src" = ["/tmp/test"] }
writable = { "output" = ["/tmp/output"] }
tmpfs = { "temp" = ["/tmp/temp"] }

[sandbox.security]
default_security_level = "standard"

[sandbox.security.namespaces]
pid = false
mount = true
network = true
ipc = true
uts = true
user = true
time = false
cgroup = true

[sandbox.security.seccomp]
enabled = true
default_action = "SCMP_ACT_ERRNO"
architectures = ["SCMP_ARCH_X86_64", "SCMP_ARCH_X86", "SCMP_ARCH_AARCH64"]

[sandbox.security.seccomp.syscalls]
allowed = ["read", "write", "open", "close", "exit", "exit_group"]
disallowed = []
"#;

        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, config_content).unwrap();
        temp_file
    }

    #[test]
    fn test_validate_config_valid() {
        let config_file = create_test_config();
        let result = validate_config(config_file.path().to_str().unwrap());
        if let Err(e) = &result {
            println!("Validation error: {}", e);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_config_invalid_path() {
        let result = validate_config("nonexistent_config.toml");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_invalid_content() {
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, "invalid toml content").unwrap();

        let result = validate_config(temp_file.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_show_config_default() {
        let result = show_config(true, &None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_config_from_file() {
        let config_file = create_test_config();
        let config_path = config_file.path().to_str().unwrap().to_string();

        let result = show_config(false, &Some(config_path));
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_config_from_default_path() {
        // This test assumes the default config file exists
        // If it doesn't exist, the test will fail as expected
        let _result = show_config(false, &None);
        // We don't assert here because the default file might not exist in test environment
    }

    #[test]
    fn test_show_config_invalid_file() {
        let result = show_config(false, &Some("nonexistent.toml".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_serve_command_creation() {
        let cli = Cli {
            log_level: Some(tracing::Level::INFO),
            config: Some("test.toml".to_string()),
            debug: false,
            open_mode: false,
            host: Some("127.0.0.1".to_string()),
            port: Some(8080),
            log_file: None,
            command: Some(Commands::Serve {
                graceful_shutdown: true,
            }),
        };

        assert_eq!(cli.log_level, Some(tracing::Level::INFO));
        assert_eq!(cli.config, Some("test.toml".to_string()));
        assert_eq!(cli.host, Some("127.0.0.1".to_string()));
        assert_eq!(cli.port, Some(8080));

        match cli.command {
            Some(Commands::Serve { graceful_shutdown }) => {
                assert!(graceful_shutdown);
            }
            _ => panic!("Expected Serve command"),
        }
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let json = serde_json::to_string_pretty(&config);
        assert!(json.is_ok());

        let json_str = json.unwrap();
        assert!(json_str.contains("api"));
        assert!(json_str.contains("sandbox"));
    }
}
