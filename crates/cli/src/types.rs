use tracing::Level;

pub struct ServeOptions {
    pub auth_enabled: bool,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub workers: Option<usize>,
    pub log_dir: String,
    pub log_level: Level,
    pub config_path: String,
}

impl ServeOptions {
    pub fn new(
        auth_enabled: bool,
        host: Option<String>,
        port: Option<u16>,
        workers: Option<usize>,
        log_dir: String,
        log_level: Level,
        config_path: String,
    ) -> Self {
        Self {
            auth_enabled,
            host,
            port,
            workers,
            log_dir,
            log_level,
            config_path,
        }
    }
}
