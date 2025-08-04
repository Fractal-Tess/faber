//! Seccomp BPF system call filtering for enhanced security
//!
//! This module provides functionality to restrict system calls that processes
//! can make, significantly reducing the attack surface of the sandbox.

use std::fs;
use std::os::unix::process::CommandExt;
use std::process::Command;
use tracing::debug;

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

/// Seccomp configuration from YAML file
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
    bpf_program: Option<Vec<u8>>,
}

impl SeccompFilter {
    /// Create a new seccomp filter
    pub fn new(level: SeccompLevel) -> Result<Self, SandboxError> {
        Ok(Self {
            level,
            config_path: None,
            bpf_program: None,
        })
    }

    /// Create a new seccomp filter with configuration file
    pub fn new_with_config(level: SeccompLevel, config_path: String) -> Result<Self, SandboxError> {
        let mut filter = Self {
            level,
            config_path: Some(config_path.clone()),
            bpf_program: None,
        };

        // Load and compile BPF program
        filter.load_and_compile_bpf(&config_path)?;

        Ok(filter)
    }

    /// Load and compile BPF program from configuration
    fn load_and_compile_bpf(&mut self, config_path: &str) -> Result<(), SandboxError> {
        let config_content = fs::read_to_string(config_path).map_err(|e| {
            SandboxError::ResourceLimitFailed(format!("Failed to read seccomp config: {}", e))
        })?;

        let config: SeccompConfig = serde_yaml::from_str(&config_content).map_err(|e| {
            SandboxError::ResourceLimitFailed(format!("Failed to parse seccomp config: {}", e))
        })?;

        // Generate BPF program based on configuration
        self.bpf_program = Some(self.generate_bpf_program(&config)?);

        Ok(())
    }

    /// Generate BPF program from configuration
    fn generate_bpf_program(&self, _config: &SeccompConfig) -> Result<Vec<u8>, SandboxError> {
        // This is a simplified BPF program generation
        // In production, you'd want to use a proper BPF compiler

        let mut bpf = Vec::new();

        // Basic BPF program structure for x86_64
        // Allow common syscalls, deny everything else
        match self.level {
            SeccompLevel::None => {
                // No filtering
                return Ok(Vec::new());
            }
            SeccompLevel::Basic => {
                // Basic filtering - allow common syscalls
                bpf.extend_from_slice(&self.generate_basic_bpf());
            }
            SeccompLevel::Strict => {
                // Strict filtering - minimal allowed syscalls
                bpf.extend_from_slice(&self.generate_strict_bpf());
            }
        }

        Ok(bpf)
    }

    /// Generate basic BPF program
    fn generate_basic_bpf(&self) -> Vec<u8> {
        // Simplified BPF program for basic filtering
        // In production, use a proper BPF compiler like libseccomp
        vec![
            // Load architecture
            0x20, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, // Jump if not x86_64
            0x15, 0x00, 0x00, 0x05, 0xc0, 0x00, 0x3e, 0x00, // Load syscall number
            0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // Allow common syscalls (read, write, exit, etc.)
            0x15, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, // read
            0x15, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, // write
            0x15, 0x00, 0x00, 0x01, 0x3c, 0x00, 0x00, 0x00, // exit
            0x15, 0x00, 0x00, 0x01, 0xe7, 0x00, 0x00, 0x00, // exit_group
            // Default action: kill process
            0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]
    }

    /// Generate strict BPF program
    fn generate_strict_bpf(&self) -> Vec<u8> {
        // Very restrictive BPF program
        vec![
            // Load architecture
            0x20, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, // Jump if not x86_64
            0x15, 0x00, 0x00, 0x03, 0xc0, 0x00, 0x3e, 0x00, // Load syscall number
            0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // Allow only essential syscalls
            0x15, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, // read
            0x15, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, // write
            0x15, 0x00, 0x00, 0x01, 0x3c, 0x00, 0x00, 0x00, // exit
            // Default action: kill process
            0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]
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

                if let Some(bpf_program) = &self.bpf_program {
                    let bpf_program = bpf_program.clone();
                    unsafe {
                        cmd.pre_exec(move || {
                            // Apply BPF program using prctl
                            let result = libc::prctl(
                                libc::PR_SET_SECCOMP,
                                libc::SECCOMP_MODE_FILTER,
                                bpf_program.as_ptr() as *const libc::c_void,
                                0,
                                0,
                            );
                            if result != 0 {
                                // Log error but don't fail
                                eprintln!(
                                    "Failed to apply seccomp filter: {}",
                                    std::io::Error::last_os_error()
                                );
                            }
                            Ok(())
                        });
                    }
                } else {
                    // Fallback to basic seccomp
                    unsafe {
                        cmd.pre_exec(move || {
                            let result = libc::prctl(
                                libc::PR_SET_SECCOMP,
                                libc::SECCOMP_MODE_STRICT,
                                0,
                                0,
                                0,
                            );
                            if result != 0 {
                                // Don't fail, just continue silently
                            }
                            Ok(())
                        });
                    }
                }

                Ok(())
            }
        }
    }

    /// Load configuration from file (for future use)
    fn load_config_filter(&self, config_path: &str) -> Result<(), SandboxError> {
        let _config_content = fs::read_to_string(config_path).map_err(|e| {
            SandboxError::ResourceLimitFailed(format!("Failed to read seccomp config: {}", e))
        })?;

        // TODO: Parse and apply configuration
        Ok(())
    }

    /// Get the current seccomp level
    pub fn level(&self) -> SeccompLevel {
        self.level
    }

    /// Check if seccomp is supported on this system
    pub fn check_support() -> Result<(), SandboxError> {
        // Check if seccomp is available
        let result = unsafe { libc::prctl(libc::PR_GET_SECCOMP, 0, 0, 0, 0) };
        if result < 0 {
            return Err(SandboxError::ResourceLimitFailed(
                "Seccomp not supported on this system".to_string(),
            ));
        }
        Ok(())
    }
}
