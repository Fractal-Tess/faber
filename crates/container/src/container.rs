use faber_config::GlobalConfig;
use faber_core::{FaberError, Result, Task, TaskResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::cgroups::CgroupManager;
use super::error::SandboxError;
use super::mounts::{MountConfig, MountManager};
use super::namespaces::{NamespaceConfig, NamespaceManager};
use super::privileges::PrivilegeManager;
use super::seccomp::SeccompLevel;

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

// Security level is now determined by configuration
// The config system provides the security settings directly

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceLimits {
    pub memory_limit: u64,
    pub cpu_time_limit: u64,
    pub wall_time_limit: u64,
    pub max_processes: u32,
    pub max_fds: u64,
    pub stack_limit: u64,
    pub data_segment_limit: u64,
    pub address_space_limit: u64,
    pub cpu_rate_limit: Option<u32>,
    pub io_read_limit: Option<u64>,
    pub io_write_limit: Option<u64>,
}

impl ResourceLimits {
    pub fn from_config(config: &GlobalConfig) -> Self {
        let limits = &config.sandbox.resource_limits;
        Self {
            memory_limit: limits.memory_limit_kb as u64 * 1024, // Convert KB to bytes
            cpu_time_limit: limits.cpu_time_limit_ms as u64 * 1_000_000, // Convert ms to nanoseconds
            wall_time_limit: limits.wall_time_limit_ms as u64 * 1_000_000, // Convert ms to nanoseconds
            max_processes: limits.max_processes,
            max_fds: limits.max_fds as u64,
            stack_limit: limits.stack_limit_kb as u64 * 1024, // Convert KB to bytes
            data_segment_limit: limits.data_segment_limit_kb as u64 * 1024, // Convert KB to bytes
            address_space_limit: limits.address_space_limit_kb as u64 * 1024, // Convert KB to bytes
            cpu_rate_limit: if limits.cpu_rate_limit_percent > 0 {
                Some(limits.cpu_rate_limit_percent)
            } else {
                None
            },
            io_read_limit: if limits.io_read_limit_kb_s > 0 {
                Some(limits.io_read_limit_kb_s as u64 * 1024) // Convert KB/s to bytes/s
            } else {
                None
            },
            io_write_limit: if limits.io_write_limit_kb_s > 0 {
                Some(limits.io_write_limit_kb_s as u64 * 1024) // Convert KB/s to bytes/s
            } else {
                None
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NamespaceSettings {
    pub pid: bool,
    pub mount: bool,
    pub network: bool,
    pub ipc: bool,
    pub uts: bool,
    pub user: bool,
    pub time: bool,
    pub cgroup: bool,
}

impl NamespaceSettings {
    pub fn from_config(config: &GlobalConfig) -> Self {
        let namespaces = &config.sandbox.security.namespaces;
        Self {
            pid: namespaces.pid,
            mount: namespaces.mount,
            network: namespaces.network,
            ipc: namespaces.ipc,
            uts: namespaces.uts,
            user: namespaces.user,
            time: namespaces.time,
            cgroup: namespaces.cgroup,
        }
    }
}

/// Configuration for container security settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerConfig {
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
    /// Create a new configuration from the global config
    pub fn from_config(config: &GlobalConfig) -> Self {
        Self {
            resource_limits: ResourceLimits::from_config(config),
            namespace_settings: NamespaceSettings::from_config(config),
            uid: 65534, // nobody user - will work with user namespace
            gid: 65534, // nobody group - will work with user namespace
            enable_mount_operations: true,
            work_dir_size_mb: 64,
            mount_config: MountConfig::from_config(config),
            seccomp_level: if config.sandbox.security.seccomp.enabled {
                SeccompLevel::Basic
            } else {
                SeccompLevel::None
            },
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
        // This should not be used directly - use from_config instead
        // This is kept for compatibility but will use default values
        Self {
            resource_limits: ResourceLimits {
                memory_limit: 512 * 1024 * 1024, // 512MB
                cpu_time_limit: 10_000_000_000,  // 10 seconds
                wall_time_limit: 30_000_000_000, // 30 seconds
                max_processes: 50,
                max_fds: 256,
                stack_limit: 4 * 1024 * 1024,                // 4MB
                data_segment_limit: 256 * 1024 * 1024,       // 256MB
                address_space_limit: 1 * 1024 * 1024 * 1024, // 1GB
                cpu_rate_limit: Some(50),                    // 50% CPU
                io_read_limit: Some(10 * 1024 * 1024),       // 10MB/s
                io_write_limit: Some(10 * 1024 * 1024),      // 10MB/s
            },
            namespace_settings: NamespaceSettings {
                pid: false,
                mount: true,
                network: true,
                ipc: true,
                uts: true,
                user: true,
                time: false,
                cgroup: true,
            },
            uid: 65534,
            gid: 65534,
            enable_mount_operations: true,
            work_dir_size_mb: 64,
            mount_config: MountConfig::default(),
            seccomp_level: SeccompLevel::None,
        }
    }
}

/// Secure container sandbox for process execution
pub struct Container {
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
    // TODO:
    // Seccomp filter for system call filtering
    // seccomp_filter: SeccompFilter,
}

impl Container {
    /// Create a new container sandbox from global config
    pub fn from_config(global_config: &GlobalConfig) -> Result<Self> {
        let config = ContainerConfig::from_config(global_config);
        Self::new_with_config(config, global_config)
    }

    /// Create a new container sandbox with specific config
    pub fn new(config: ContainerConfig) -> Result<Self> {
        // Load default config for cgroups and other settings
        let global_config = GlobalConfig::default();
        Self::new_with_config(config, &global_config)
    }

    /// Create a new container sandbox with both configs
    fn new_with_config(config: ContainerConfig, global_config: &GlobalConfig) -> Result<Self> {
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
            return Err(FaberError::Sandbox(error_context));
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
            user: config.namespace_settings.user, // Use config setting instead of forcing true
        };

        let namespace_manager = NamespaceManager::new(namespace_config);

        // Initialize mount manager with container-specific configuration
        let mount_config = config.mount_config.clone();
        info!(
            "Using mount configuration with {} mounts",
            mount_config.mounts.len()
        );
        let mount_manager = MountManager::new(&mount_config, &container_root);

        // Create directory structure first (outside namespace)
        if let Err(e) = mount_manager.apply_mounts() {
            let debug_info = format!(
                "Container root: {}, Mount config: {:?}, Error: {}",
                container_root.display(),
                mount_config,
                e
            );
            let error_context =
                format!("Failed to apply mounts for container {container_id}. {debug_info}");
            return Err(FaberError::Sandbox(error_context));
        }

        // Log successful mount operations for debugging
        info!(
            "Successfully applied mounts for container {} with config: {:?}",
            container_id, mount_config
        );

        // Apply path masking for additional security
        if let Err(e) = mount_manager.apply_path_masking() {
            warn!(
                "Failed to apply path masking for container {}: {}",
                container_id, e
            );
        }

        // Initialize cgroup manager for resource limits
        let cgroup_manager = match CgroupManager::new(&container_id, global_config) {
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

        // TODO: ADD seccomp filtering

        info!(
            "Successfully created container sandbox: {container_id} with root at {container_root:?}"
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
            // seccomp_filter,
        })
    }

    /// Get the container ID
    pub fn container_id(&self) -> &str {
        &self.container_id
    }

    /// Get the working directory
    pub fn work_dir(&self) -> &Path {
        &self.work_dir
    }

    /// Check if container is active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Copy files into the container
    pub fn copy_files_in(&mut self, files: &HashMap<String, String>) -> Result<()> {
        if !self.is_active {
            return Err(FaberError::Sandbox(
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
                    return Err(FaberError::Sandbox(error_context));
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
                return Err(FaberError::Sandbox(error_context));
            }
        }

        info!(
            "Successfully copied {} files into container {}",
            files.len(),
            self.container_id
        );
        Ok(())
    }

    fn is_active_or_err(&self) -> Result<()> {
        if !self.is_active {
            error!("❌ Container is not active");
            return Err(FaberError::Sandbox("Container is not active".to_string()));
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
    ) -> Result<TaskResult> {
        use faber_core::{ResourceLimitViolations, ResourceUsage};
        use std::time::Instant;

        // Check if container is active
        self.is_active_or_err()?;

        // Record start time for wall clock measurement
        let start_time = Instant::now();

        // Resolve command path within the sandbox environment
        let resolved_command = if command.starts_with('/') {
            // For absolute paths, try to find the command in mounted directories
            let command_name = std::path::Path::new(command)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| command.to_string());

            let path_dirs = [
                self.container_root.join("usr/local/bin"),
                self.container_root.join("usr/bin"),
                self.container_root.join("bin"),
            ];

            let mut found_path = None;
            for dir in &path_dirs {
                let test_path = dir.join(&command_name);
                if test_path.exists() {
                    found_path = Some(test_path);
                    break;
                }
            }

            match found_path {
                Some(path) => {
                    info!(
                        "Resolved absolute command '{}' to '{}'",
                        command,
                        path.display()
                    );
                    path.to_string_lossy().to_string()
                }
                None => {
                    // If not found in mounted dirs, try the original path (might work in some cases)
                    warn!(
                        "Could not resolve absolute command '{}' in mounted directories, trying original path",
                        command
                    );
                    command.to_string()
                }
            }
        } else {
            // For relative commands, use the command as-is and let PATH resolve it
            command.to_string()
        };

        debug!("Command resolved to: {}", resolved_command);

        // Construct command with resolved path
        let mut cmd = Command::new(&resolved_command);
        cmd.args(args);
        cmd.current_dir(&self.work_dir);

        // IMPORTANT: Set up stdout/stderr capture BEFORE applying namespaces
        // This ensures the pipes are established before any namespace changes
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // Apply seccomp filter for system call restriction
        // if let Err(e) = self.seccomp_filter.apply_to_command(&mut cmd) {
        //     warn!("Failed to apply seccomp filter: {}", e);
        // }

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
            "Executing command in container {}: '{}' (resolved from '{}') with args {:?} in dir {}",
            self.container_id,
            resolved_command,
            command,
            args,
            self.work_dir.display()
        );

        // Check if resolved command file exists
        if !std::path::Path::new(&resolved_command).exists() {
            warn!("Resolved command file does not exist: {}", resolved_command);
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
            resource_violations.output_limit_exceeded = true;
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
            if resource_violations.output_limit_exceeded {
                faber_core::TaskStatus::MemoryLimitExceeded
            } else if resource_violations.cpu_time_limit_exceeded {
                faber_core::TaskStatus::CpuLimitExceeded
            } else if resource_violations.wall_time_limit_exceeded {
                faber_core::TaskStatus::Timeout
            } else {
                faber_core::TaskStatus::ResourceLimitExceeded
            }
        } else if exit_code != 0 {
            faber_core::TaskStatus::Failure
        } else {
            faber_core::TaskStatus::Success
        };

        if exit_code != 0 && status == faber_core::TaskStatus::Failure {
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
            return Err(FaberError::Sandbox(error_context));
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
    pub fn cleanup(&mut self) -> Result<()> {
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
            warn!("Failed to unmount filesystems during cleanup: {}", e);
        }

        // Try to force unmount any remaining mounts in the container root
        if self.container_root.exists() {
            self.force_unmount_container_root();

            // Try to remove container root directory with retries
            let mut attempts = 0;
            const MAX_ATTEMPTS: usize = 3;

            while attempts < MAX_ATTEMPTS {
                match std::fs::remove_dir_all(&self.container_root) {
                    Ok(()) => break,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::Other
                            && e.raw_os_error() == Some(16) =>
                    {
                        // Resource busy - retry after a short delay
                        attempts += 1;
                        if attempts < MAX_ATTEMPTS {
                            warn!(
                                "Container root busy, retrying cleanup in 100ms (attempt {})",
                                attempts
                            );
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        } else {
                            warn!(
                                "Failed to remove container root after {} attempts (resource busy): {}",
                                MAX_ATTEMPTS,
                                self.container_root.display()
                            );
                            // Don't fail the cleanup - just leave the directory for manual cleanup
                        }
                    }
                    Err(e) => {
                        return Err(FaberError::Sandbox(format!(
                            "Failed to remove container root {}: {}",
                            self.container_root.display(),
                            e
                        )));
                    }
                }
            }
        }

        self.is_active = false;
        Ok(())
    }

    /// Force unmount any remaining mounted filesystems in the container root
    fn force_unmount_container_root(&self) {
        use nix::mount::{MntFlags, umount2};

        // Try to force unmount the container root itself and any subdirectories
        let paths_to_unmount = [
            self.container_root.join("proc"),
            self.container_root.join("tmp"),
            self.container_root.join("work"),
            self.container_root.clone(),
        ];

        for path in &paths_to_unmount {
            if path.exists() {
                // Try lazy unmount first
                if let Err(_) = umount2(path, MntFlags::MNT_DETACH) {
                    // If lazy unmount fails, try force unmount
                    let _ = umount2(path, MntFlags::MNT_FORCE | MntFlags::MNT_DETACH);
                }
            }
        }
    }

    /// Get process resource usage using wait4
    fn get_process_usage(&self, pid: u32) -> Result<ProcessUsage> {
        use libc::{WNOHANG, rusage, wait4};
        use std::mem;

        let mut rusage: rusage = unsafe { mem::zeroed() };

        // Use wait4 to get resource usage for the process
        let result = unsafe { wait4(pid as i32, std::ptr::null_mut(), WNOHANG, &mut rusage) };

        if result < 0 {
            return Err(FaberError::Sandbox(format!(
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

        // Check if common binaries exist in mounted locations
        let common_bins = [
            ("sh", ["bin/sh", "usr/bin/sh"]),
            ("ls", ["bin/ls", "usr/bin/ls"]),
            ("echo", ["bin/echo", "usr/bin/echo"]),
            ("gcc", ["usr/bin/gcc", "usr/local/bin/gcc"]),
            ("python3", ["usr/bin/python3", "usr/local/bin/python3"]),
            ("which", ["usr/bin/which", "bin/which"]),
        ];

        let mut available_bins = Vec::new();
        let mut missing_bins = Vec::new();

        for (bin_name, possible_paths) in &common_bins {
            let mut found = false;
            for path in possible_paths {
                let bin_path = self.container_root.join(path);
                if bin_path.exists() {
                    available_bins.push(format!("{} ({})", bin_name, bin_path.display()));
                    found = true;
                    break;
                }
            }
            if !found {
                let attempted_paths: Vec<String> = possible_paths
                    .iter()
                    .map(|p| self.container_root.join(p).display().to_string())
                    .collect();
                missing_bins.push(format!("{} (tried: {:?})", bin_name, attempted_paths));
            }
        }

        info.push(format!("Available binaries: {:?}", available_bins));
        info.push(format!("Missing binaries: {:?}", missing_bins));

        // Check PATH directories and their contents
        let path_dirs = [
            self.container_root.join("usr/local/bin"),
            self.container_root.join("usr/bin"),
            self.container_root.join("bin"),
        ];

        let mut path_info = Vec::new();
        for dir in &path_dirs {
            if dir.exists() {
                // Count files in directory
                if let Ok(entries) = std::fs::read_dir(dir) {
                    let count = entries.count();
                    path_info.push(format!("{} ({} files)", dir.display(), count));
                } else {
                    path_info.push(format!("{} (unreadable)", dir.display()));
                }
            } else {
                path_info.push(format!("{} (missing)", dir.display()));
            }
        }
        info.push(format!("PATH directories: {:?}", path_info));

        // Check working directory
        if self.work_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&self.work_dir) {
                let files: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();
                info.push(format!(
                    "Work directory accessible with {} items: {:?}",
                    files.len(),
                    files
                ));
            } else {
                info.push("Work directory exists but not readable".to_string());
            }
        } else {
            info.push("Work directory does not exist".to_string());
        }

        // Check mount points
        let mut mount_info = Vec::new();
        for mount in &self.config.mount_config.mounts {
            let target_path = self.container_root.join(&mount.target);
            if target_path.exists() {
                mount_info.push(format!(
                    "{:?} -> {} (OK)",
                    mount.source,
                    target_path.display()
                ));
            } else {
                mount_info.push(format!(
                    "{:?} -> {} (MISSING)",
                    mount.source,
                    target_path.display()
                ));
            }
        }
        info.push(format!("Mount status: {:?}", mount_info));

        info.join(", ")
    }
}

impl Drop for Container {
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
