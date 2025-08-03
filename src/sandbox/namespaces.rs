//! Namespace configuration for container isolation
//!
//! This module handles Linux namespace creation and management
//! for secure container isolation.

use crate::sandbox::{Result, SandboxError};
use nix::sched::{CloneFlags, unshare};
use nix::unistd::{getpid, setpgid, setsid};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Configuration for Linux namespaces
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Enable cgroup namespace isolation
    pub cgroup: bool,
}

impl Default for NamespaceConfig {
    fn default() -> Self {
        Self {
            pid: true,
            mount: true,
            network: true,
            ipc: true,
            uts: true,
            user: false, // Disabled by default due to complexity
            cgroup: true,
        }
    }
}

impl NamespaceConfig {
    /// Create a new namespace configuration with all isolation enabled
    pub fn full_isolation() -> Self {
        Self {
            pid: true,
            mount: true,
            network: true,
            ipc: true,
            uts: true,
            user: true,
            cgroup: true,
        }
    }

    /// Create a new namespace configuration with minimal isolation
    pub fn minimal_isolation() -> Self {
        Self {
            pid: true,
            mount: true,
            network: false,
            ipc: true,
            uts: false,
            user: false,
            cgroup: false,
        }
    }

    /// Create a new namespace configuration for testing (less isolation)
    pub fn testing() -> Self {
        Self {
            pid: false,
            mount: true,
            network: false,
            ipc: false,
            uts: false,
            user: false,
            cgroup: false,
        }
    }

    /// Convert to CloneFlags for namespace creation
    pub fn to_clone_flags(&self) -> CloneFlags {
        let mut flags = CloneFlags::empty();

        if self.pid {
            flags |= CloneFlags::CLONE_NEWPID;
        }
        if self.mount {
            flags |= CloneFlags::CLONE_NEWNS;
        }
        if self.network {
            flags |= CloneFlags::CLONE_NEWNET;
        }
        if self.ipc {
            flags |= CloneFlags::CLONE_NEWIPC;
        }
        if self.uts {
            flags |= CloneFlags::CLONE_NEWUTS;
        }
        if self.user {
            flags |= CloneFlags::CLONE_NEWUSER;
        }
        if self.cgroup {
            flags |= CloneFlags::CLONE_NEWCGROUP;
        }

        flags
    }

    /// Create the namespaces using unshare
    pub fn create_namespaces(&self) -> Result<()> {
        let flags = self.to_clone_flags();

        if flags.is_empty() {
            debug!("No namespaces to create");
            return Ok(());
        }

        info!("Creating namespaces with flags: {:?}", flags);

        unshare(flags).map_err(|e| {
            SandboxError::NamespaceSetup(format!("Failed to create namespaces: {}", e))
        })?;

        // If we created a PID namespace, we need to handle session/process group
        if self.pid {
            self.setup_pid_namespace()?;
        }

        // If we created a UTS namespace, set a container hostname
        if self.uts {
            self.setup_uts_namespace()?;
        }

        info!("Successfully created namespaces");
        Ok(())
    }

    /// Setup PID namespace specifics
    fn setup_pid_namespace(&self) -> Result<()> {
        debug!("Setting up PID namespace");

        // Create new session and process group
        setsid().map_err(|e| {
            SandboxError::NamespaceSetup(format!("Failed to create new session: {}", e))
        })?;

        let pid = getpid();
        setpgid(pid, pid).map_err(|e| {
            SandboxError::NamespaceSetup(format!("Failed to set process group: {}", e))
        })?;

        debug!("PID namespace setup complete");
        Ok(())
    }

    /// Setup UTS namespace specifics
    fn setup_uts_namespace(&self) -> Result<()> {
        debug!("Setting up UTS namespace");

        // Set container hostname
        let hostname = "faber-container";

        unsafe {
            let hostname_cstr = std::ffi::CString::new(hostname)
                .map_err(|e| SandboxError::NamespaceSetup(format!("Invalid hostname: {}", e)))?;

            if libc::sethostname(hostname_cstr.as_ptr(), hostname.len()) != 0 {
                return Err(SandboxError::NamespaceSetup(
                    "Failed to set container hostname".to_string(),
                ));
            }
        }

        debug!(
            "UTS namespace setup complete, hostname set to: {}",
            hostname
        );
        Ok(())
    }

    /// Get a summary of enabled namespaces
    pub fn summary(&self) -> Vec<&'static str> {
        let mut enabled = Vec::new();

        if self.pid {
            enabled.push("PID");
        }
        if self.mount {
            enabled.push("Mount");
        }
        if self.network {
            enabled.push("Network");
        }
        if self.ipc {
            enabled.push("IPC");
        }
        if self.uts {
            enabled.push("UTS");
        }
        if self.user {
            enabled.push("User");
        }
        if self.cgroup {
            enabled.push("Cgroup");
        }

        enabled
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

    /// Enter the configured namespaces
    pub fn enter_namespaces(&self) -> Result<()> {
        info!("Entering namespaces: {:?}", self.config.summary());
        self.config.create_namespaces()
    }

    /// Get the namespace configuration
    pub fn config(&self) -> &NamespaceConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_config_defaults() {
        let config = NamespaceConfig::default();
        assert!(config.pid);
        assert!(config.mount);
        assert!(config.network);
        assert!(config.ipc);
        assert!(config.uts);
        assert!(!config.user); // Should be disabled by default
        assert!(config.cgroup);
    }

    #[test]
    fn test_namespace_config_full_isolation() {
        let config = NamespaceConfig::full_isolation();
        assert!(config.pid);
        assert!(config.mount);
        assert!(config.network);
        assert!(config.ipc);
        assert!(config.uts);
        assert!(config.user);
        assert!(config.cgroup);
    }

    #[test]
    fn test_namespace_config_minimal_isolation() {
        let config = NamespaceConfig::minimal_isolation();
        assert!(config.pid);
        assert!(config.mount);
        assert!(!config.network);
        assert!(config.ipc);
        assert!(!config.uts);
        assert!(!config.user);
        assert!(!config.cgroup);
    }

    #[test]
    fn test_clone_flags_conversion() {
        let config = NamespaceConfig::default();
        let flags = config.to_clone_flags();

        // Should contain the default enabled flags
        assert!(flags.contains(CloneFlags::CLONE_NEWPID));
        assert!(flags.contains(CloneFlags::CLONE_NEWNS));
        assert!(flags.contains(CloneFlags::CLONE_NEWNET));
        assert!(flags.contains(CloneFlags::CLONE_NEWIPC));
        assert!(flags.contains(CloneFlags::CLONE_NEWUTS));
        assert!(flags.contains(CloneFlags::CLONE_NEWCGROUP));

        // Should not contain user namespace (disabled by default)
        assert!(!flags.contains(CloneFlags::CLONE_NEWUSER));
    }

    #[test]
    fn test_namespace_summary() {
        let config = NamespaceConfig::default();
        let summary = config.summary();

        assert!(summary.contains(&"PID"));
        assert!(summary.contains(&"Mount"));
        assert!(summary.contains(&"Network"));
        assert!(summary.contains(&"IPC"));
        assert!(summary.contains(&"UTS"));
        assert!(summary.contains(&"Cgroup"));
        assert!(!summary.contains(&"User"));
    }
}
