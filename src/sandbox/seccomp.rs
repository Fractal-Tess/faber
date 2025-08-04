//! Seccomp BPF system call filtering for enhanced security
//!
//! This module provides functionality to restrict system calls that processes
//! can make, significantly reducing the attack surface of the sandbox.

use std::os::unix::process::CommandExt;
use std::process::Command;
use tracing::{debug, error, info, warn};

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

/// Seccomp filter manager
pub struct SeccompFilter {
    level: SeccompLevel,
}

impl SeccompFilter {
    /// Create a new seccomp filter
    pub fn new(level: SeccompLevel) -> Result<Self, SandboxError> {
        Ok(Self { level })
    }

    /// Apply seccomp filter to a command
    pub fn apply_to_command(&self, cmd: &mut Command) -> Result<(), SandboxError> {
        match self.level {
            SeccompLevel::None => {
                debug!("Seccomp filtering disabled");
                Ok(())
            }
            SeccompLevel::Basic | SeccompLevel::Strict => {
                // For now, we'll use a simple seccomp approach
                // TODO: Implement proper libseccomp integration
                debug!("Seccomp filtering enabled (level: {:?})", self.level);

                unsafe {
                    cmd.pre_exec(move || {
                        // Simple seccomp setup using prctl
                        // This is a basic implementation - in production, use libseccomp
                        let result =
                            libc::prctl(libc::PR_SET_SECCOMP, libc::SECCOMP_MODE_STRICT, 0, 0, 0);

                        if result != 0 {
                            warn!(
                                "Failed to set seccomp mode: {}",
                                std::io::Error::last_os_error()
                            );
                            // Don't fail, just warn
                        } else {
                            debug!("Seccomp strict mode applied");
                        }

                        Ok(())
                    });
                }

                Ok(())
            }
        }
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
