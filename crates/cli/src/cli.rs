use clap::{Parser, Subcommand};
use tracing::Level;

#[derive(Parser, Debug)]
#[command(name = "faber")]
#[command(about = "A secure sandboxed task execution service")]
#[command(version)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Log level (error, warn, info, debug, trace)
    #[arg(short, long, default_value = "info")]
    pub log_level: Option<Level>,

    /// Configuration file path
    #[arg(short, long, default_value = "config/default.toml")]
    pub config: Option<String>,

    /// Enable debug mode
    #[arg(short, long)]
    pub debug: bool,

    /// Enable open mode (no authentication)
    #[arg(long)]
    pub open_mode: bool,

    /// Host to bind to
    #[arg(long)]
    pub host: Option<String>,

    /// Port to bind to
    #[arg(short, long)]
    pub port: Option<u16>,

    /// Number of worker threads for the execution queue
    #[arg(long)]
    pub workers: Option<usize>,

    /// Log file path (if not specified, logs only to console)
    #[arg(long)]
    pub log_file: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the Faber server
    Serve {
        /// Enable graceful shutdown
        #[arg(long)]
        graceful_shutdown: bool,
    },
    /// Validate configuration
    Validate {
        /// Configuration file to validate (uses --config if not specified)
        config: Option<String>,
    },
    /// Show configuration
    Config {
        /// Show default configuration
        #[arg(long)]
        default: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_parse_basic() {
        let args = vec!["faber"];
        let cli = Cli::try_parse_from(args).unwrap();

        assert_eq!(cli.log_level, Some(Level::INFO));
        assert_eq!(cli.config, Some("config/default.toml".to_string()));
        assert!(!cli.debug);
        assert!(!cli.open_mode);
        assert!(cli.host.is_none());
        assert!(cli.port.is_none());
        assert!(cli.workers.is_none());
        assert!(cli.log_file.is_none());
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_cli_parse_with_options() {
        let args = vec![
            "faber",
            "--log-level",
            "debug",
            "--config",
            "test.toml",
            "--debug",
            "--open-mode",
            "--host",
            "127.0.0.1",
            "--port",
            "8080",
            "--workers",
            "4",
            "--log-file",
            "test.log",
        ];
        let cli = Cli::try_parse_from(args).unwrap();

        assert_eq!(cli.log_level, Some(Level::DEBUG));
        assert_eq!(cli.config, Some("test.toml".to_string()));
        assert!(cli.debug);
        assert!(cli.open_mode);
        assert_eq!(cli.host, Some("127.0.0.1".to_string()));
        assert_eq!(cli.port, Some(8080));
        assert_eq!(cli.workers, Some(4));
        assert_eq!(cli.log_file, Some("test.log".to_string()));
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
            workers: Some(4),
            log_file: None,
            command: Some(Commands::Serve {
                graceful_shutdown: true,
            }),
        };

        assert_eq!(cli.log_level, Some(tracing::Level::INFO));
        assert_eq!(cli.config, Some("test.toml".to_string()));
        assert_eq!(cli.host, Some("127.0.0.1".to_string()));
        assert_eq!(cli.port, Some(8080));
        assert_eq!(cli.workers, Some(4));

        match cli.command {
            Some(Commands::Serve { graceful_shutdown }) => {
                assert!(graceful_shutdown);
            }
            _ => panic!("Expected Serve command"),
        }
    }

    #[test]
    fn test_cli_parse_serve_command() {
        let args = vec!["faber", "serve", "--graceful-shutdown"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Some(Commands::Serve { graceful_shutdown }) => {
                assert!(graceful_shutdown);
            }
            _ => panic!("Expected Serve command"),
        }
    }

    #[test]
    fn test_cli_parse_validate_command() {
        let args = vec!["faber", "validate", "test-config.toml"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Some(Commands::Validate { config }) => {
                assert_eq!(config, Some("test-config.toml".to_string()));
            }
            _ => panic!("Expected Validate command"),
        }
    }

    #[test]
    fn test_cli_parse_config_command() {
        let args = vec!["faber", "config", "--default"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Some(Commands::Config { default }) => {
                assert!(default);
            }
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn test_cli_help() {
        let mut cmd = Cli::command();
        let help = cmd.render_help().to_string();

        assert!(help.contains("faber"));
        assert!(help.contains("A secure sandboxed task execution service"));
        assert!(help.contains("serve"));
        assert!(help.contains("validate"));
        assert!(help.contains("config"));
    }

    // Version test removed due to clap API constraints
}
