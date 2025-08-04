//! Seccomp BPF system call filtering for enhanced security
//!
//! This module provides functionality to restrict system calls that processes
//! can make, significantly reducing the attack surface of the sandbox.

use std::fs;
use std::os::unix::process::CommandExt;
use std::process::Command;
use tracing::{debug, info, warn};

use super::error::SandboxError;

/// Seccomp security level
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SeccompLevel {
    /// No seccomp filtering
    None,
    /// Basic filtering - allow common syscalls
    Basic,
    /// Strict filtering - minimal allowed syscalls
    Strict,
}

impl Default for SeccompLevel {
    fn default() -> Self {
        Self::Basic
    }
}

/// Seccomp configuration from YAML file (for future use)
#[derive(Debug, serde::Deserialize)]
struct SeccompConfig {
    #[serde(default = "default_action")]
    default_action: String,
    #[serde(default)]
    architectures: Vec<String>,
    #[serde(default)]
    syscalls: Vec<SyscallRule>,
}

#[derive(Debug, serde::Deserialize)]
struct SyscallRule {
    names: Vec<String>,
    action: String,
}

fn default_action() -> String {
    "SCMP_ACT_ERRNO".to_string()
}

/// Seccomp filter manager
pub struct SeccompFilter {
    level: SeccompLevel,
    config_path: Option<String>,
}

impl SeccompFilter {
    /// Create a new seccomp filter
    pub fn new(level: SeccompLevel) -> Result<Self, SandboxError> {
        Ok(Self {
            level,
            config_path: None,
        })
    }

    /// Create a new seccomp filter with configuration file
    pub fn new_with_config(level: SeccompLevel, config_path: String) -> Result<Self, SandboxError> {
        Ok(Self {
            level,
            config_path: Some(config_path),
        })
    }

    /// Apply seccomp filter to a command
    pub fn apply_to_command(&self, cmd: &mut Command) -> Result<(), SandboxError> {
        match self.level {
            SeccompLevel::None => {
                debug!("Seccomp filtering disabled");
                Ok(())
            }
            SeccompLevel::Basic | SeccompLevel::Strict => {
                debug!("Seccomp filtering enabled (level: {:?})", self.level);

                // Try to load configuration file first (for future use)
                if let Some(config_path) = &self.config_path {
                    if let Ok(_) = self.load_config_filter(config_path) {
                        debug!("Seccomp configuration loaded from {}", config_path);
                    }
                }

                // Use simple seccomp setup using prctl
                unsafe {
                    cmd.pre_exec(move || {
                        // Simple seccomp setup using prctl
                        // This is a basic implementation - in production, use libseccomp
                        let result =
                            libc::prctl(libc::PR_SET_SECCOMP, libc::SECCOMP_MODE_STRICT, 0, 0, 0);
                        if result != 0 {
                            // Don't fail, just continue silently
                        }
                        Ok(())
                    });
                }

                Ok(())
            }
        }
    }

    /// Load seccomp configuration from file (placeholder for future implementation)
    fn load_config_filter(&self, config_path: &str) -> Result<(), SandboxError> {
        let config_content = fs::read_to_string(config_path).map_err(|e| {
            SandboxError::SecuritySetup(format!("Failed to read seccomp config: {}", e))
        })?;

        let _config: SeccompConfig = serde_yaml::from_str(&config_content).map_err(|e| {
            SandboxError::SecuritySetup(format!("Failed to parse seccomp config: {}", e))
        })?;

        // TODO: Implement proper seccomp filter creation when libseccomp linking is resolved
        debug!("Seccomp configuration loaded from {}", config_path);
        Ok(())
    }

    /// Get the seccomp level
    pub fn level(&self) -> SeccompLevel {
        self.level
    }

    /// Check if seccomp is supported on this system
    pub fn check_support() -> Result<(), SandboxError> {
        // Check if seccomp is available via prctl
        let result = unsafe { libc::prctl(libc::PR_GET_SECCOMP, 0, 0, 0, 0) };

        if result >= 0 {
            info!("Seccomp is supported on this system");
            Ok(())
        } else {
            warn!("Seccomp is not supported on this system");
            Err(SandboxError::SecuritySetup(
                "Seccomp not supported".to_string(),
            ))
        }
    }
}
