use nix::sched::CloneFlags;
use tracing::{debug, info, warn};

use crate::config::NamespaceConfig;

#[derive(Debug, thiserror::Error)]
pub enum NamespaceError {
    #[error("Failed to create namespace: {0}")]
    CreationError(String),
    #[error("Failed to cleanup namespace: {0}")]
    CleanupError(String),
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

        // Note: Namespaces are typically created when the container process starts
        // via unshare(2) or clone(2) with namespace flags. This method is mainly
        // for validation and preparation.

        // Validate namespace configuration
        self.validate_namespace_config()?;

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

    /// Get unshare flags for the configured namespaces
    pub fn get_unshare_flags(&self) -> Vec<&'static str> {
        let mut flags = Vec::new();

        if self.config.mount {
            flags.push("--mount");
        }
        if self.config.uts {
            flags.push("--uts");
        }
        if self.config.ipc {
            flags.push("--ipc");
        }
        if self.config.network {
            flags.push("--net");
        }
        if self.config.pid {
            flags.push("--pid");
        }
        if self.config.user {
            flags.push("--user");
        }
        if self.config.time {
            flags.push("--time");
        }
        if self.config.cgroup {
            flags.push("--cgroup");
        }

        flags
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

    /// Validate namespace configuration
    fn validate_namespace_config(&self) -> Result<(), NamespaceError> {
        // Check for potential conflicts or unsupported combinations
        if self.config.user && !self.config.pid {
            warn!("User namespace enabled but PID namespace disabled - this may cause issues");
        }

        if self.config.network && !self.config.mount {
            warn!(
                "Network namespace enabled but mount namespace disabled - this may limit functionality"
            );
        }

        Ok(())
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

    /// Check if a specific namespace is enabled
    pub fn is_namespace_enabled(&self, namespace: &str) -> bool {
        match namespace {
            "mount" => self.config.mount,
            "uts" => self.config.uts,
            "ipc" => self.config.ipc,
            "network" => self.config.network,
            "pid" => self.config.pid,
            "user" => self.config.user,
            "time" => self.config.time,
            "cgroup" => self.config.cgroup,
            _ => false,
        }
    }

    /// Get the namespace configuration
    pub fn get_config(&self) -> &NamespaceConfig {
        &self.config
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

    /// Check if any namespaces are enabled
    pub fn has_enabled_namespaces(&self) -> bool {
        self.config.mount
            || self.config.uts
            || self.config.ipc
            || self.config.network
            || self.config.pid
            || self.config.user
            || self.config.time
            || self.config.cgroup
    }
}
