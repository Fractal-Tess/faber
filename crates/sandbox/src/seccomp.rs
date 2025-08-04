use faber_core::Result;
use tracing::info;

pub struct SeccompManager {
    pub enabled: bool,
    pub level: String,
    pub config_file: String,
}

impl SeccompManager {
    pub fn new(enabled: bool, level: String, config_file: String) -> Self {
        Self {
            enabled,
            level,
            config_file,
        }
    }

    pub async fn setup_seccomp(&self) -> Result<()> {
        if !self.enabled {
            info!("Seccomp disabled");
            return Ok(());
        }

        info!(
            "Would setup seccomp with level: {}, config: {}",
            self.level, self.config_file
        );
        // TODO: Implement seccomp setup
        Ok(())
    }
}
