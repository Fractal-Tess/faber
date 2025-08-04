//! Linux namespace management for container isolation
//!
//! This module provides functionality to create and manage Linux namespaces
//! for secure process isolation.

use std::os::unix::process::CommandExt;
use std::process::Command;
use tracing::{debug, info};

use super::error::SandboxError;

/// Linux namespace flags from libc
const CLONE_NEWPID: i32 = 0x20000000; // PID namespace
const CLONE_NEWNS: i32 = 0x00020000; // Mount namespace  
const CLONE_NEWNET: i32 = 0x40000000; // Network namespace
const CLONE_NEWIPC: i32 = 0x08000000; // IPC namespace
const CLONE_NEWUTS: i32 = 0x04000000; // UTS namespace (hostname)
const CLONE_NEWUSER: i32 = 0x10000000; // User namespace

/// Configuration for which namespaces to enable
#[derive(Debug, Clone)]
pub struct NamespaceConfig {
    /// Enable PID namespace isolation
    pub pid: bool,
    /// Enable mount namespace isolation
    pub mount: bool,
    /// Enable network namespace isolation  
    pub network: bool,
    /// Enable IPC namespace isolation
    pub ipc: bool,
    /// Enable UTS namespace isolation (hostname)
    pub uts: bool,
    /// Enable user namespace isolation
    pub user: bool,
}

impl Default for NamespaceConfig {
    fn default() -> Self {
        Self {
            pid: true,
            mount: true,
            network: true,
            ipc: true,
            uts: true,
            user: false, // User namespace can be complex, start disabled
        }
    }
}

impl NamespaceConfig {
    /// Create a configuration with all namespaces enabled
    pub fn all_enabled() -> Self {
        Self {
            pid: true,
            mount: true,
            network: true,
            ipc: true,
            uts: true,
            user: true,
        }
    }

    /// Create a configuration with minimal namespaces (PID + Mount only)
    pub fn minimal() -> Self {
        Self {
            pid: true,
            mount: true,
            network: false,
            ipc: false,
            uts: false,
            user: false,
        }
    }

    /// Get the clone flags for this configuration
    pub fn clone_flags(&self) -> i32 {
        let mut flags = 0;

        if self.pid {
            flags |= CLONE_NEWPID;
        }
        if self.mount {
            flags |= CLONE_NEWNS;
        }
        if self.network {
            flags |= CLONE_NEWNET;
        }
        if self.ipc {
            flags |= CLONE_NEWIPC;
        }
        if self.uts {
            flags |= CLONE_NEWUTS;
        }
        if self.user {
            flags |= CLONE_NEWUSER;
        }

        flags
    }
}

/// Namespace manager for handling namespace operations
pub struct NamespaceManager {
    config: NamespaceConfig,
}

impl NamespaceManager {
    /// Create a new namespace manager
    pub fn new(config: NamespaceConfig) -> Self {
        Self { config }
    }

    /// Apply namespace configuration to a command
    /// This uses the `unshare` system call to create new namespaces
    pub fn apply_namespaces(&self, cmd: &mut Command) -> Result<(), SandboxError> {
        let flags = self.config.clone_flags();

        if flags == 0 {
            return Ok(());
        }

        // Use pre_exec to call unshare before executing the command
        unsafe {
            cmd.pre_exec(move || {
                // Call unshare to create new namespaces
                let result = libc::unshare(flags);
                if result != 0 {
                    let error = std::io::Error::last_os_error();
                    return Err(error);
                }

                // If we have a mount namespace, set up filesystem isolation
                if flags & CLONE_NEWNS != 0 {
                    // TODO: Set up mounts within the namespace here
                    // This is where we would call mount_manager.setup_namespace_mounts()
                    // For now, we'll rely on the container setup done earlier
                }

                // If we have a network namespace, set up loopback interface
                if flags & CLONE_NEWNET != 0 {
                    // Bring up loopback interface
                    let _ = std::process::Command::new("ip")
                        .args(["link", "set", "dev", "lo", "up"])
                        .output();
                }

                // If we have a PID namespace, we're now PID 1 in the new namespace
                if flags & CLONE_NEWPID != 0 {
                    // TODO: Handle PID 1 responsibilities if needed
                }

                Ok(())
            });
        }

        debug!("Namespace configuration applied to command");
        Ok(())
    }

    /// Check if namespaces are supported on this system
    pub fn check_namespace_support() -> Result<(), SandboxError> {
        // Try to check if namespace files exist in /proc
        let namespace_files = [
            "/proc/self/ns/pid",
            "/proc/self/ns/mnt",
            "/proc/self/ns/net",
            "/proc/self/ns/ipc",
            "/proc/self/ns/uts",
            "/proc/self/ns/user",
        ];

        for ns_file in &namespace_files {
            if !std::path::Path::new(ns_file).exists() {
                return Err(SandboxError::NamespaceSetup(format!(
                    "Namespace support not available: {ns_file} not found"
                )));
            }
        }

        info!("Namespace support verified");
        Ok(())
    }
}
