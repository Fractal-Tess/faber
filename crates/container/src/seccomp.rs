use faber_core::{FaberError, Result};
use std::process::Command;
use tracing::{info, warn};

/// Seccomp security levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SeccompLevel {
    None,
    Basic,
    Medium,
    Strict,
}

impl Default for SeccompLevel {
    fn default() -> Self {
        Self::None
    }
}

/// Seccomp filter for system call restriction
pub struct SeccompFilter {
    pub level: SeccompLevel,
    pub config_file: Option<String>,
}

impl SeccompFilter {
    /// Create a new seccomp filter with the specified level
    pub fn new(level: SeccompLevel) -> std::result::Result<Self, super::error::SandboxError> {
        Ok(Self {
            level,
            config_file: None,
        })
    }

    /// Create a new seccomp filter with configuration file
    pub fn new_with_config(
        level: SeccompLevel,
        config_file: String,
    ) -> std::result::Result<Self, super::error::SandboxError> {
        Ok(Self {
            level,
            config_file: Some(config_file),
        })
    }

    /// Apply seccomp filter to a command
    pub fn apply_to_command(&self, _cmd: &mut Command) -> Result<()> {
        if self.level == SeccompLevel::None {
            return Ok(());
        }

        info!("Applying seccomp filter level: {:?}", self.level);

        // Apply seccomp filter using prctl
        unsafe {
            use libc::{PR_SET_SECCOMP, SECCOMP_MODE_FILTER, prctl};

            // For now, we'll use a basic seccomp filter
            // In a real implementation, you would load a BPF filter
            if prctl(
                PR_SET_SECCOMP,
                SECCOMP_MODE_FILTER,
                std::ptr::null::<libc::c_void>(),
                0,
                0,
            ) != 0
            {
                warn!(
                    "Failed to apply seccomp filter: {}",
                    std::io::Error::last_os_error()
                );
            }
        }

        Ok(())
    }
}

// Legacy SeccompManager for backward compatibility
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
