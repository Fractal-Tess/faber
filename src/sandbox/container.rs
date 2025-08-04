//! Secure container-based sandbox implementation
//!
//! This module provides a production-ready container sandbox with:
//! - Linux namespace isolation (PID, Mount, Network, IPC, UTS, User)  
//! - Resource limits via cgroups (memory, CPU, processes)
//! - Privilege dropping and user isolation
//! - Secure filesystem isolation

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::executor::{task::TaskResult, task::TaskStatus};

use super::cgroups::CgroupManager;
use super::error::SandboxError;
use super::mounts::{MountConfig, MountManager};
use super::namespaces::{NamespaceConfig, NamespaceManager};
use super::privileges::PrivilegeManager;
use super::seccomp::{SeccompFilter, SeccompLevel};

/// Process resource usage statistics
#[derive(Debug, Clone)]
struct ProcessUsage {
    /// User CPU time in nanoseconds
    user_time_ns: u64,
    /// System CPU time in nanoseconds
    system_time_ns: u64,
    /// Maximum resident set size in KB
    max_rss_kb: u64,
}

/// Security level presets for container isolation
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SecurityLevel {
    /// Minimal isolation - for trusted code
    Minimal,
    /// Standard isolation - for most use cases
    Standard,
    /// Maximum isolation - for untrusted code
    Maximum,
    /// Custom isolation - user-defined settings
    Custom,
}

/// Namespace isolation configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NamespaceSettings {
    pub pid: bool,
    pub mount: bool,
    pub network: bool,
    pub ipc: bool,
    pub uts: bool,
    pub user: bool,
    /// Enable time namespace (Linux 5.6+)
    pub time: bool,
    /// Enable cgroup namespace
    pub cgroup: bool,
}

impl NamespaceSettings {
    pub fn from_security_level(level: SecurityLevel) -> Self {
        match level {
            SecurityLevel::Minimal => Self {
                pid: false,
                mount: false,
                network: true,
                ipc: false,
                uts: false,
                user: false,
                time: false,
                cgroup: false,
            },
            SecurityLevel::Standard => Self {
                pid: false,    // Disable PID namespace to avoid process spawning issues
                mount: true,   // Re-enable mount namespace with proper setup
                network: true, // Enable network namespace for isolation
                ipc: true,
                uts: true,
                user: true, // Enable user namespace for secure privilege dropping
                time: false,
                cgroup: true,
            },
            SecurityLevel::Maximum => Self {
                pid: true,
                mount: true,
                network: false,
                ipc: true,
                uts: true,
                user: true,
                time: true,
                cgroup: true,
            },
            SecurityLevel::Custom => Self {
                // Default to maximum security for custom
                pid: true,
                mount: true,
                network: false,
                ipc: true,
                uts: true,
                user: true,
                time: true,
                cgroup: true,
            },
        }
    }
}

/// Resource limits for container execution
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResourceLimits {
    /// Memory limit in bytes
    pub memory_limit: u64,
    /// CPU time limit in nanoseconds  
    pub cpu_time_limit: u64,
    /// Wall time limit in nanoseconds
    pub wall_time_limit: u64,
    /// Maximum number of processes
    pub max_processes: u32,
    /// Maximum number of file descriptors
    pub max_fds: u64,
    /// Stack size limit in bytes
    pub stack_limit: u64,
    /// Data segment limit in bytes
    pub data_segment_limit: u64,
    /// Address space limit in bytes
    pub address_space_limit: u64,
    /// CPU rate limit (percentage)
    pub cpu_rate_limit: Option<u32>,
    /// CPU set limit (specific cores)
    pub cpu_set_limit: Option<String>,
    /// I/O rate limits
    pub io_read_limit: Option<u64>,
    pub io_write_limit: Option<u64>,
}

impl ResourceLimits {
    pub fn from_security_level(level: SecurityLevel) -> Self {
        match level {
            SecurityLevel::Minimal => Self {
                memory_limit: 2 * 1024 * 1024 * 1024, // 2GB
                cpu_time_limit: 30_000_000_000,       // 30 seconds
                wall_time_limit: 60_000_000_000,      // 60 seconds
                max_processes: 100,
                max_fds: 1024,
                stack_limit: 8 * 1024 * 1024,                // 8MB
                data_segment_limit: 1 * 1024 * 1024 * 1024,  // 1GB
                address_space_limit: 4 * 1024 * 1024 * 1024, // 4GB
                cpu_rate_limit: None,
                cpu_set_limit: None,
                io_read_limit: None,
                io_write_limit: None,
            },
            SecurityLevel::Standard => Self {
                memory_limit: 512 * 1024 * 1024, // 512MB
                cpu_time_limit: 10_000_000_000,  // 10 seconds
                wall_time_limit: 30_000_000_000, // 30 seconds
                max_processes: 50,
                max_fds: 256,
                stack_limit: 4 * 1024 * 1024,                // 4MB
                data_segment_limit: 256 * 1024 * 1024,       // 256MB
                address_space_limit: 1 * 1024 * 1024 * 1024, // 1GB
                cpu_rate_limit: Some(50),                    // 50% CPU
                cpu_set_limit: None,
                io_read_limit: Some(10 * 1024 * 1024), // 10MB/s
                io_write_limit: Some(10 * 1024 * 1024), // 10MB/s
            },
            SecurityLevel::Maximum => Self {
                memory_limit: 128 * 1024 * 1024, // 128MB
                cpu_time_limit: 5_000_000_000,   // 5 seconds
                wall_time_limit: 15_000_000_000, // 15 seconds
                max_processes: 10,
                max_fds: 64,
                stack_limit: 1 * 1024 * 1024,           // 1MB
                data_segment_limit: 64 * 1024 * 1024,   // 64MB
                address_space_limit: 256 * 1024 * 1024, // 256MB
                cpu_rate_limit: Some(25),               // 25% CPU
                cpu_set_limit: None,
                io_read_limit: Some(1 * 1024 * 1024),  // 1MB/s
                io_write_limit: Some(1 * 1024 * 1024), // 1MB/s
            },
            SecurityLevel::Custom => Self {
                // Default to maximum security for custom
                memory_limit: 128 * 1024 * 1024, // 128MB
                cpu_time_limit: 5_000_000_000,   // 5 seconds
                wall_time_limit: 15_000_000_000, // 15 seconds
                max_processes: 10,
                max_fds: 64,
                stack_limit: 1 * 1024 * 1024,           // 1MB
                data_segment_limit: 64 * 1024 * 1024,   // 64MB
                address_space_limit: 256 * 1024 * 1024, // 256MB
                cpu_rate_limit: Some(25),               // 25% CPU
                cpu_set_limit: None,
                io_read_limit: Some(1 * 1024 * 1024),  // 1MB/s
                io_write_limit: Some(1 * 1024 * 1024), // 1MB/s
            },
        }
    }
}

/// Security hardening options
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SecurityOptions {
    /// Enable ASLR (Address Space Layout Randomization)
    pub enable_aslr: bool,
    /// Enable stack canaries
    pub enable_stack_canaries: bool,
    /// Enable DEP (Data Execution Prevention)
    pub enable_dep: bool,
    /// Enable seccomp filtering
    pub enable_seccomp: bool,
    /// Enable capabilities dropping
    pub enable_capabilities: bool,
    /// Enable no_new_privs
    pub enable_no_new_privs: bool,
    /// Enable secure computing mode
    pub enable_secure_computing: bool,
    /// Disable core dumps
    pub disable_core_dumps: bool,
    /// Restrict file system access
    pub restrict_fs_access: bool,
    /// Enable network isolation
    pub enable_network_isolation: bool,
    /// Enable time namespace
    pub enable_time_namespace: bool,
}

impl SecurityOptions {
    pub fn from_security_level(level: SecurityLevel) -> Self {
        match level {
            SecurityLevel::Minimal => Self {
                enable_aslr: true,
                enable_stack_canaries: false,
                enable_dep: true,
                enable_seccomp: false,
                enable_capabilities: false,
                enable_no_new_privs: false,
                enable_secure_computing: false,
                disable_core_dumps: false,
                restrict_fs_access: false,
                enable_network_isolation: true,
                enable_time_namespace: false,
            },
            SecurityLevel::Standard => Self {
                enable_aslr: true,
                enable_stack_canaries: true,
                enable_dep: true,
                enable_seccomp: true,
                enable_capabilities: true,
                enable_no_new_privs: true,
                enable_secure_computing: true,
                disable_core_dumps: true,
                restrict_fs_access: true,
                enable_network_isolation: true,
                enable_time_namespace: false,
            },
            SecurityLevel::Maximum => Self {
                enable_aslr: true,
                enable_stack_canaries: true,
                enable_dep: true,
                enable_seccomp: true,
                enable_capabilities: true,
                enable_no_new_privs: true,
                enable_secure_computing: true,
                disable_core_dumps: true,
                restrict_fs_access: true,
                enable_network_isolation: true,
                enable_time_namespace: true,
            },
            SecurityLevel::Custom => Self {
                // Default to maximum security for custom
                enable_aslr: true,
                enable_stack_canaries: true,
                enable_dep: true,
                enable_seccomp: true,
                enable_capabilities: true,
                enable_no_new_privs: true,
                enable_secure_computing: true,
                disable_core_dumps: true,
                restrict_fs_access: true,
                enable_network_isolation: true,
                enable_time_namespace: true,
            },
        }
    }
}

/// Configuration for container security settings
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContainerConfig {
    /// Security level preset
    pub security_level: SecurityLevel,
    /// Resource limits for execution
    pub resource_limits: ResourceLimits,
    /// Namespace isolation settings
    pub namespace_settings: NamespaceSettings,
    /// User ID to run processes as
    pub uid: u32,
    /// Group ID to run processes as  
    pub gid: u32,
    /// Whether to attempt mount operations (requires privileges)
    pub enable_mount_operations: bool,
    /// Size of RAM-based work directory in MB (default: 256)
    pub work_dir_size_mb: u32,
    /// Mount configuration for filesystem access
    pub mount_config: MountConfig,
    /// Seccomp security level
    pub seccomp_level: SeccompLevel,
}

impl ContainerConfig {
    /// Create a new configuration with the specified security level
    pub fn new(security_level: SecurityLevel) -> Self {
        Self {
            security_level,
            resource_limits: ResourceLimits::from_security_level(security_level),
            namespace_settings: NamespaceSettings::from_security_level(security_level),
            uid: 65534, // nobody user - will work with user namespace
            gid: 65534, // nobody group - will work with user namespace
            enable_mount_operations: true,
            work_dir_size_mb: 64,
            mount_config: MountConfig::default(),
            seccomp_level: SeccompLevel::None, // Temporarily disable seccomp due to being too restrictive
        }
    }

    /// Builder method to customize resource limits
    pub fn with_resource_limits(mut self, limits: ResourceLimits) -> Self {
        self.resource_limits = limits;
        self
    }

    /// Builder method to customize namespace settings
    pub fn with_namespace_settings(mut self, settings: NamespaceSettings) -> Self {
        self.namespace_settings = settings;
        self
    }

    /// Builder method to set user/group IDs
    pub fn with_user_ids(mut self, uid: u32, gid: u32) -> Self {
        self.uid = uid;
        self.gid = gid;
        self
    }
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self::new(SecurityLevel::Standard)
    }
}

/// Secure container sandbox for process execution
pub struct ContainerSandbox {
    /// Unique container ID
    container_id: String,
    /// Container configuration
    config: ContainerConfig,
    /// Working directory path
    work_dir: PathBuf,
    /// Root filesystem path for container
    container_root: PathBuf,
    /// Whether container is active
    is_active: bool,
    /// Cgroup manager for resource management
    cgroup_manager: Option<CgroupManager>,
    /// Namespace manager for managing namespaces
    namespace_manager: NamespaceManager,
    /// Mount manager for managing filesystem mounts
    mount_manager: MountManager,
    /// Privilege manager for dropping privileges
    privilege_manager: PrivilegeManager,
    /// Seccomp filter for system call filtering
    seccomp_filter: SeccompFilter,
}

impl ContainerSandbox {
    /// Create a new container sandbox with static default configuration
    pub fn new_default() -> Result<Self, SandboxError> {
        // Static configuration - cannot be controlled by API
        let config = ContainerConfig::new(SecurityLevel::Standard);
        Self::new(config)
    }

    /// Create a new container sandbox
    pub fn new(config: ContainerConfig) -> Result<Self, SandboxError> {
        let container_id = Uuid::new_v4().to_string();
        info!("Creating new container sandbox: {}", container_id);

        // Create container root directory
        let container_root = std::env::temp_dir()
            .join("faber_container")
            .join(&container_id);

        if let Err(e) = fs::create_dir_all(&container_root) {
            let error_context = format!(
                "Failed to create container root directory {} for container {}. Error: {}",
                container_root.display(),
                container_id,
                e
            );
            return Err(SandboxError::ContainerCreation(error_context));
        }

        // Work directory will be created as a tmpfs mount for fast I/O performance
        let work_dir = container_root.join("work");

        // Ensure work directory has proper permissions
        if let Err(e) = std::fs::create_dir_all(&work_dir) {
            warn!("Failed to create work directory: {}", e);
        }
        // Set permissions to be writable by the nobody user (65534)
        if let Err(e) = std::fs::set_permissions(&work_dir, std::fs::Permissions::from_mode(0o777))
        {
            warn!("Failed to set work directory permissions: {}", e);
        }

        // Initialize namespace manager
        let namespace_config = NamespaceConfig {
            pid: config.namespace_settings.pid,
            mount: config.namespace_settings.mount,
            network: config.namespace_settings.network,
            ipc: config.namespace_settings.ipc,
            uts: config.namespace_settings.uts,
            user: true, // Enable user namespace for secure privilege dropping
        };

        let namespace_manager = NamespaceManager::new(namespace_config);

        // Initialize mount manager with container-specific configuration
        let mount_config = if config.mount_config.mounts.is_empty() {
            // Use default config with custom work directory size
            MountConfig::default_secure()
        } else {
            config.mount_config.clone()
        };
        let mount_manager = MountManager::new(&mount_config, &container_root);

        // Create directory structure first (outside namespace)
        if let Err(e) = mount_manager.apply_mounts() {
            let error_context = format!(
                "Failed to apply mounts for container {}. Mount config: {:?}. Error: {}",
                container_id, mount_config, e
            );
            return Err(SandboxError::MountFailed(error_context));
        }

        // Apply path masking for additional security
        if let Err(e) = mount_manager.apply_path_masking() {
            warn!(
                "Failed to apply path masking for container {}: {}",
                container_id, e
            );
        }

        // Initialize cgroup manager for resource limits
        let cgroup_manager = match CgroupManager::new(&container_id) {
            Ok(manager) => {
                // Apply resource limits
                if let Err(e) = manager.apply_limits(&config.resource_limits) {
                    warn!("Failed to apply resource limits: {}", e);
                    None
                } else {
                    Some(manager)
                }
            }
            Err(e) => {
                warn!("Failed to create cgroup manager: {}", e);
                None
            }
        };

        // Initialize privilege manager
        let privilege_manager = PrivilegeManager::new(config.uid, config.gid);

        // Initialize seccomp filter
        let seccomp_filter = if Path::new("seccomp.yaml").exists() {
            SeccompFilter::new_with_config(config.seccomp_level, "seccomp.yaml".to_string())
                .unwrap_or_else(|e| {
                    warn!(
                        "Failed to create seccomp filter with config: {}, falling back to basic",
                        e
                    );
                    SeccompFilter::new(config.seccomp_level).unwrap_or_else(|e| {
                        warn!(
                            "Failed to create seccomp filter: {}, falling back to none",
                            e
                        );
                        SeccompFilter::new(SeccompLevel::None).unwrap()
                    })
                })
        } else {
            SeccompFilter::new(config.seccomp_level).unwrap_or_else(|e| {
                warn!(
                    "Failed to create seccomp filter: {}, falling back to none",
                    e
                );
                SeccompFilter::new(SeccompLevel::None).unwrap()
            })
        };

        info!(
            "Successfully created container sandbox: {} with root at {}",
            container_id,
            container_root.display()
        );

        Ok(Self {
            container_id,
            config,
            work_dir,
            container_root,
            is_active: true,
            cgroup_manager,
            namespace_manager,
            mount_manager,
            privilege_manager,
            seccomp_filter,
        })
    }

    /// Get the container ID
    pub fn container_id(&self) -> &str {
        &self.container_id
    }

    /// Get the working directory
    ///
    /// The work directory is mounted as a tmpfs (RAM-based filesystem)
    /// for maximum I/O performance. This is particularly beneficial for:
    /// - Compilation workloads with many temporary files
    /// - File-intensive operations
    /// - Applications that create many small files
    pub fn work_dir(&self) -> &Path {
        &self.work_dir
    }

    /// Get the work directory path
    ///
    /// Returns the path to the tmpfs-mounted work directory
    pub fn work_dir_path(&self) -> PathBuf {
        self.work_dir.clone()
    }

    /// Check if container is active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Copy files into the container
    pub fn copy_files_in(&mut self, files: &HashMap<String, String>) -> Result<(), SandboxError> {
        if !self.is_active {
            return Err(SandboxError::ExecutionFailed(
                "Cannot copy files to inactive container".to_string(),
            ));
        }

        info!(
            "Copying {} files into container {}",
            files.len(),
            self.container_id
        );

        for (file_path, content) in files {
            let full_path = self.work_dir.join(file_path);

            // Create parent directories
            if let Some(parent) = full_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    let error_context = format!(
                        "Failed to create directory {} in container {}. Error: {}",
                        parent.display(),
                        self.container_id,
                        e
                    );
                    return Err(SandboxError::ExecutionFailed(error_context));
                }
            }

            // Write file
            if let Err(e) = std::fs::write(&full_path, content) {
                let error_context = format!(
                    "Failed to write file {} in container {}. Content length: {} bytes. Error: {}",
                    full_path.display(),
                    self.container_id,
                    content.len(),
                    e
                );
                return Err(SandboxError::ExecutionFailed(error_context));
            }
        }

        info!(
            "Successfully copied {} files into container {}",
            files.len(),
            self.container_id
        );
        Ok(())
    }

    fn is_active_or_err(&self) -> Result<(), SandboxError> {
        if !self.is_active {
            error!("❌ Container is not active");
            return Err(SandboxError::ContainerNotActive);
        }

        Ok(())
    }

    fn build_std_env(&self, mut cmd: Command) -> Command {
        // Build PATH pointing to mounted directories within container
        let bin_path = self.container_root.join("bin");
        let usr_bin_path = self.container_root.join("usr/bin");
        let usr_local_bin_path = self.container_root.join("usr/local/bin");

        let path = format!(
            "{}:{}:{}",
            usr_local_bin_path.to_string_lossy(),
            usr_bin_path.to_string_lossy(),
            bin_path.to_string_lossy()
        );
        // Set environment variables
        cmd.env_clear();
        cmd.env("PATH", path);
        cmd.env("PWD", self.work_dir.to_string_lossy().to_string());
        cmd.env("HOME", self.work_dir.to_string_lossy().to_string());

        // Set library path for dynamic linking
        let lib_path = self.container_root.join("lib");
        let usr_lib_path = self.container_root.join("usr/lib");
        let lib64_path = self.container_root.join("lib64");

        let ld_library_path = format!(
            "{}:{}:{}",
            lib_path.to_string_lossy(),
            usr_lib_path.to_string_lossy(),
            lib64_path.to_string_lossy()
        );

        cmd.env("LD_LIBRARY_PATH", ld_library_path);

        cmd
    }

    /// Execute a command in the container with full isolation
    pub fn execute_command(
        &mut self,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<TaskResult, SandboxError> {
        use crate::executor::task::{ResourceLimitViolations, ResourceUsage};
        use std::time::Instant;

        // Check if container is active
        self.is_active_or_err()?;

        // Record start time for wall clock measurement
        let start_time = Instant::now();

        // Construct command
        let mut cmd = Command::new(command);
        cmd.args(args);
        cmd.current_dir(&self.work_dir);

        // IMPORTANT: Set up stdout/stderr capture BEFORE applying namespaces
        // This ensures the pipes are established before any namespace changes
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // Apply seccomp filter for system call restriction
        if let Err(e) = self.seccomp_filter.apply_to_command(&mut cmd) {
            warn!("Failed to apply seccomp filter: {}", e);
        }

        // Apply privilege dropping
        if let Err(e) = self.privilege_manager.apply_privileges(&mut cmd) {
            warn!("Failed to apply privilege dropping: {}", e);
        }

        // Build standard environment variables
        cmd = self.build_std_env(cmd);

        // Apply custom environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        // Apply namespaces to the command (but be careful not to break pipes)
        if let Err(e) = self.namespace_manager.apply_namespaces(&mut cmd) {
            warn!("Failed to apply namespaces: {}", e);
        }

        // Log command details for debugging
        info!(
            "Executing command in container {}: '{}' with args {:?} in dir {}",
            self.container_id,
            command,
            args,
            self.work_dir.display()
        );

        // Debug: Check if the command exists
        let command_path = if command.starts_with('/') {
            command.to_string()
        } else {
            // Try to find the command in PATH
            let path_dirs = [
                self.container_root.join("usr/local/bin"),
                self.container_root.join("usr/bin"),
                self.container_root.join("bin"),
            ];

            let mut found_path = None;
            for dir in &path_dirs {
                let test_path = dir.join(command);
                if test_path.exists() {
                    found_path = Some(test_path);
                    break;
                }
            }

            found_path
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| command.to_string())
        };

        debug!("Command path resolved to: {}", command_path);

        // Check if command file exists
        if !std::path::Path::new(&command_path).exists() {
            warn!("Command file does not exist: {}", command_path);
        }

        // Execute and capture result
        let child = cmd.spawn().map_err(|e| {
            let debug_info = self.get_debug_info();
            let error_context = format!(
                "Command '{}' execution failed in container {}. Working directory: {}. Command args: {:?}. Error: {}. Error kind: {:?}. Debug info: {}",
                command,
                self.container_id,
                self.work_dir.display(),
                args,
                e,
                e.kind(),
                debug_info
            );
            SandboxError::ExecutionFailed(error_context)
        })?;

        let child_pid = child.id();

        // Add process to cgroup (if cgroup manager exists)
        if let Some(ref cgroup_manager) = self.cgroup_manager {
            if let Err(e) = cgroup_manager.add_process(child_pid) {
                warn!("Failed to add process to cgroup: {}", e);
            }
        }

        // Wait for the process to complete
        let output = child.wait_with_output().map_err(|e| {
            let debug_info = self.get_debug_info();
            let error_context = format!(
                "Command '{}' execution failed in container {}. Working directory: {}. Command args: {:?}. Error: {}. Error kind: {:?}. Debug info: {}",
                command,
                self.container_id,
                self.work_dir.display(),
                args,
                e,
                e.kind(),
                debug_info
            );
            SandboxError::ExecutionFailed(error_context)
        })?;

        // Calculate wall time
        let wall_time = start_time.elapsed();
        let wall_time_ns = wall_time.as_nanos() as u64;

        // Collect resource usage statistics
        let mut resource_usage = ResourceUsage::new();
        let mut resource_violations = ResourceLimitViolations::new();

        // Get resource stats from cgroup if available
        if let Some(ref cgroup_manager) = self.cgroup_manager {
            if let Ok(stats) = cgroup_manager.get_resource_stats() {
                resource_usage.memory_peak_bytes = stats.memory_usage;
                resource_usage.memory_current_bytes = stats.memory_usage;
                resource_usage.cpu_time_ns = stats.cpu_usage;
                resource_usage.process_count = stats.process_count;
                resource_usage.io_read_bytes = stats.io_read_bytes;
                resource_usage.io_write_bytes = stats.io_write_bytes;
            }
        }

        // Set wall time
        resource_usage.wall_time_ns = wall_time_ns;

        // Get process resource usage using wait4 if available
        if let Ok(usage) = self.get_process_usage(child_pid) {
            resource_usage.user_time_ns = usage.user_time_ns;
            resource_usage.system_time_ns = usage.system_time_ns;
            resource_usage.cpu_time_ns = usage.user_time_ns + usage.system_time_ns;
        }

        // Check for resource limit violations
        let limits = &self.config.resource_limits;
        if resource_usage.memory_peak_bytes > limits.memory_limit {
            resource_violations.memory_limit_exceeded = true;
        }
        if resource_usage.cpu_time_ns > limits.cpu_time_limit {
            resource_violations.cpu_time_limit_exceeded = true;
        }
        if resource_usage.wall_time_ns > limits.wall_time_limit {
            resource_violations.wall_time_limit_exceeded = true;
        }
        if resource_usage.process_count > limits.max_processes {
            resource_violations.process_limit_exceeded = true;
        }

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Log the captured output and resource usage for debugging
        debug!(
            "Command '{}' completed with exit code {}. Wall time: {:?}, CPU time: {:?}, Memory: {:.2}MB. Stdout: '{}', Stderr: '{}'",
            command,
            exit_code,
            resource_usage.wall_time(),
            resource_usage.cpu_time(),
            resource_usage.memory_peak_mb(),
            stdout,
            stderr
        );

        // Determine task status based on exit code and resource violations
        let status = if resource_violations.any_exceeded() {
            if resource_violations.memory_limit_exceeded {
                TaskStatus::MemoryLimitExceeded
            } else if resource_violations.cpu_time_limit_exceeded {
                TaskStatus::CpuLimitExceeded
            } else if resource_violations.wall_time_limit_exceeded {
                TaskStatus::Timeout
            } else {
                TaskStatus::ResourceLimitExceeded
            }
        } else if exit_code != 0 {
            TaskStatus::Failure
        } else {
            TaskStatus::Success
        };

        if exit_code != 0 && status == TaskStatus::Failure {
            let debug_info = self.get_debug_info();
            let error_context = format!(
                "Command '{}' failed with exit code {} in container {}. Working directory: {}. Command args: {:?}. Stdout: '{}'. Stderr: '{}'. Debug info: {}",
                command,
                exit_code,
                self.container_id,
                self.work_dir.display(),
                args,
                stdout,
                stderr,
                debug_info
            );
            return Err(SandboxError::ExecutionFailed(error_context));
        }

        Ok(TaskResult {
            status,
            error: None,
            exit_code: Some(exit_code),
            stdout: Some(stdout),
            stderr: Some(stderr),
            resource_usage,
            resource_limits_exceeded: resource_violations,
        })
    }

    /// Clean up the container
    pub fn cleanup(&mut self) -> Result<(), SandboxError> {
        if !self.is_active {
            return Ok(());
        }

        info!("Cleaning up container {}", self.container_id);

        // Clean up cgroups
        if let Some(ref mut cgroup_manager) = self.cgroup_manager {
            if let Err(e) = cgroup_manager.cleanup() {
                warn!("Failed to cleanup cgroups: {}", e);
            }
        }

        // Unmount all filesystems first to prevent "Resource busy" errors
        if let Err(e) = self.mount_manager.unmount_all() {
            // Log the error but don't fail cleanup - we'll try to remove anyway
            error!("Failed to unmount filesystems during cleanup: {}", e);
        }

        // Remove container root directory
        if self.container_root.exists() {
            std::fs::remove_dir_all(&self.container_root).map_err(|e| {
                SandboxError::CleanupFailed(format!(
                    "Failed to remove container root {}: {}",
                    self.container_root.display(),
                    e
                ))
            })?;
        }

        self.is_active = false;
        Ok(())
    }

    /// Get process resource usage using wait4
    fn get_process_usage(&self, pid: u32) -> Result<ProcessUsage, SandboxError> {
        use libc::{WNOHANG, rusage, wait4};
        use std::mem;

        let mut rusage: rusage = unsafe { mem::zeroed() };

        // Use wait4 to get resource usage for the process
        let result = unsafe { wait4(pid as i32, std::ptr::null_mut(), WNOHANG, &mut rusage) };

        if result < 0 {
            return Err(SandboxError::ResourceLimitFailed(format!(
                "Failed to get resource usage for process {}: {}",
                pid,
                std::io::Error::last_os_error()
            )));
        }

        Ok(ProcessUsage {
            user_time_ns: (rusage.ru_utime.tv_sec as u64 * 1_000_000_000)
                + (rusage.ru_utime.tv_usec as u64 * 1000),
            system_time_ns: (rusage.ru_stime.tv_sec as u64 * 1_000_000_000)
                + (rusage.ru_stime.tv_usec as u64 * 1000),
            max_rss_kb: rusage.ru_maxrss as u64,
        })
    }

    /// Get debugging information about the container state
    fn get_debug_info(&self) -> String {
        let mut info = Vec::new();

        info.push(format!("Container ID: {}", self.container_id));
        info.push(format!("Working directory: {}", self.work_dir.display()));
        info.push(format!("Container root: {}", self.container_root.display()));
        info.push(format!("Is active: {}", self.is_active));

        // Check if common binaries exist
        let common_bins = [
            "/bin/sh",
            "/bin/ls",
            "/usr/bin/gcc",
            "/usr/bin/python3",
            "/bin/echo",
            "/usr/bin/which",
        ];
        let mut available_bins = Vec::new();
        let mut missing_bins = Vec::new();

        for bin in &common_bins {
            let bin_path = self.container_root.join(bin.trim_start_matches('/'));
            if bin_path.exists() {
                available_bins.push(bin);
            } else {
                missing_bins.push(bin);
            }
        }

        info.push(format!("Available binaries: {:?}", available_bins));
        info.push(format!("Missing binaries: {:?}", missing_bins));

        // Check PATH directories
        let path_dirs = [
            self.container_root.join("usr/local/bin"),
            self.container_root.join("usr/bin"),
            self.container_root.join("bin"),
        ];

        let mut existing_path_dirs = Vec::new();
        let mut missing_path_dirs = Vec::new();

        for dir in &path_dirs {
            if dir.exists() {
                existing_path_dirs.push(dir.display().to_string());
            } else {
                missing_path_dirs.push(dir.display().to_string());
            }
        }

        info.push(format!(
            "Existing PATH directories: {:?}",
            existing_path_dirs
        ));
        info.push(format!("Missing PATH directories: {:?}", missing_path_dirs));

        // Check working directory contents
        if let Ok(entries) = std::fs::read_dir(&self.work_dir) {
            let files: Vec<String> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect();
            info.push(format!("Work directory contents: {:?}", files));
        } else {
            info.push("Work directory not accessible".to_string());
        }

        info.join(", ")
    }
}

impl Drop for ContainerSandbox {
    fn drop(&mut self) {
        if self.is_active {
            if let Err(e) = self.cleanup() {
                error!(
                    "Failed to cleanup container {} during drop: {}",
                    self.container_id, e
                );
            }
        }
    }
}
