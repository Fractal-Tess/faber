use nix::sched::{CloneFlags, unshare};
use tracing::{debug, info, warn};

use crate::config::NamespaceConfig;

#[derive(Debug, thiserror::Error)]
pub enum NamespaceError {
    #[error("Failed to create namespace: {0}")]
    CreationError(String),
    #[error("Failed to cleanup namespace: {0}")]
    CleanupError(String),
    #[error("Failed to setup environment: {0}")]
    EnvironmentError(String),
}

/// Container namespace manager
pub struct ContainerNamespaces {
    config: NamespaceConfig,
    active_namespaces: Vec<String>,
}

impl ContainerNamespaces {
    pub fn new(config: NamespaceConfig) -> Self {
        Self {
            config,
            active_namespaces: Vec::new(),
        }
    }

    /// Initialize namespaces based on configuration
    pub async fn initialize(&mut self) -> Result<(), NamespaceError> {
        info!("Initializing container namespaces");

        // Log which namespaces will be used
        self.log_namespace_config();

        info!("Container namespaces initialized successfully");
        Ok(())
    }

    /// Cleanup namespaces
    pub async fn cleanup(&mut self) -> Result<(), NamespaceError> {
        info!("Cleaning up container namespaces");

        // Note: Namespaces are automatically cleaned up when the last process
        // in the namespace exits. This method is mainly for logging.

        self.active_namespaces.clear();
        info!("Container namespaces cleanup completed");
        Ok(())
    }

    /// Set up the container environment for command execution
    /// This creates namespaces based on the configuration
    pub fn setup_environment(&self) -> Result<(), NamespaceError> {
        debug!("Setting up container namespaces");

        // Get namespace flags
        let clone_flags = self.get_clone_flags();

        // Create namespaces
        unshare(clone_flags).map_err(|e| {
            NamespaceError::EnvironmentError(format!("Failed to create namespaces: {e}"))
        })?;

        debug!("Container namespaces setup completed");
        Ok(())
    }

    /// Get nix CloneFlags for the configured namespaces
    pub fn get_clone_flags(&self) -> CloneFlags {
        let mut flags = CloneFlags::empty();

        if self.config.mount {
            flags.insert(CloneFlags::CLONE_NEWNS);
        }
        if self.config.uts {
            flags.insert(CloneFlags::CLONE_NEWUTS);
        }
        if self.config.ipc {
            flags.insert(CloneFlags::CLONE_NEWIPC);
        }
        if self.config.network {
            flags.insert(CloneFlags::CLONE_NEWNET);
        }
        if self.config.pid {
            flags.insert(CloneFlags::CLONE_NEWPID);
        }
        if self.config.user {
            flags.insert(CloneFlags::CLONE_NEWUSER);
        }

        if self.config.cgroup {
            flags.insert(CloneFlags::CLONE_NEWCGROUP);
        }

        flags
    }

    /// Log namespace configuration
    fn log_namespace_config(&self) {
        debug!("Namespace configuration:");
        debug!("  Mount namespace: {}", self.config.mount);
        debug!("  UTS namespace: {}", self.config.uts);
        debug!("  IPC namespace: {}", self.config.ipc);
        debug!("  Network namespace: {}", self.config.network);
        debug!("  PID namespace: {}", self.config.pid);
        debug!("  User namespace: {}", self.config.user);
        debug!("  Time namespace: {}", self.config.time);
        debug!("  Cgroup namespace: {}", self.config.cgroup);
    }

    /// Get a summary of enabled namespaces
    pub fn get_enabled_namespaces(&self) -> Vec<&'static str> {
        let mut enabled = Vec::new();

        if self.config.mount {
            enabled.push("mount");
        }
        if self.config.uts {
            enabled.push("uts");
        }
        if self.config.ipc {
            enabled.push("ipc");
        }
        if self.config.network {
            enabled.push("network");
        }
        if self.config.pid {
            enabled.push("pid");
        }
        if self.config.user {
            enabled.push("user");
        }
        if self.config.time {
            enabled.push("time");
        }
        if self.config.cgroup {
            enabled.push("cgroup");
        }

        enabled
    }
}
